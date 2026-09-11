use anyhow::Result;
use argon2::{
    Argon2, Params,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
};
use axum::{
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, TokenData, Validation, decode, encode};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;
use surrealdb::Surreal;
use surrealdb::engine::local::Db;
use tracing;
use uuid::Uuid;

use dmart_shared::models::*;

impl<S: Send + Sync> FromRequestParts<S> for Claims {
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Claims>()
            .cloned()
            .ok_or((StatusCode::UNAUTHORIZED, "Not authenticated"))
    }
}

fn jwt_expiry_hours() -> i64 {
    static JWT_EXPIRY: OnceLock<i64> = OnceLock::new();
    *JWT_EXPIRY.get_or_init(|| {
        std::env::var("JWT_EXPIRY_HOURS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1)
            .clamp(1, 24)
    })
}

fn jwt_secret() -> &'static [u8] {
    static JWT_SECRET: OnceLock<Vec<u8>> = OnceLock::new();
    JWT_SECRET.get_or_init(|| {
        std::env::var("JWT_SECRET")
            .map(|s| s.into_bytes())
            .unwrap_or_else(|_| {
                let mut key = vec![0u8; 32];
                rand::thread_rng().fill_bytes(&mut key);
                tracing::warn!("⚠️ JWT_SECRET not set! Using auto-generated 32-byte key. Tokens will be invalid after server restart. Set JWT_SECRET in .env");
                key
            })
    })
    .as_slice()
}

static REVOKED_TOKENS: OnceLock<std::sync::Mutex<HashMap<String, i64>>> = OnceLock::new();

/// Adds a JWT to the revocation list until its natural expiration (used on logout).
pub fn revoke_token(token: &str, exp: i64) {
    if token.is_empty() {
        return;
    }
    REVOKED_TOKENS
        .get_or_init(Default::default)
        .lock()
        .unwrap()
        .insert(token.to_owned(), exp);
}

/// Returns true if the token has been explicitly revoked, pruning expired entries.
fn is_token_revoked(token: &str) -> bool {
    let now = chrono::Utc::now().timestamp();
    let mut map = REVOKED_TOKENS.get_or_init(Default::default).lock().unwrap();
    map.retain(|_, exp| *exp > now);
    map.contains_key(token)
}

/// Persists a JWT revocation to Valkey when available so it survives server
/// restarts and is shared across instances. The entry expires with the token.
pub async fn persist_revoked_token(token: &str, exp: i64) {
    if crate::cache::cache_available() {
        let ttl = (exp - chrono::Utc::now().timestamp()).max(1) as u64;
        let _ = crate::cache::cache_set(&format!("jwt:revoked:{}", token), "1", ttl).await;
    }
}

/// Checks the distributed (Valkey) revocation list for a token.
pub async fn is_token_revoked_in_cache(token: &str) -> bool {
    crate::cache::cache_available()
        && crate::cache::cache_get(&format!("jwt:revoked:{}", token))
            .await
            .is_some()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub username: String,
    pub rol: String,
    pub permissions: Vec<String>,
    pub exp: i64,
    pub iat: i64,
}

impl Claims {
    pub fn has_permission(&self, permission: &str) -> bool {
        self.permissions.iter().any(|p| p == permission || p == "*")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    pub token: String,
    pub user: UserInfo,
    pub mfa_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MfaSetupResponse {
    pub secret: String,
    pub qr_code: String,
    pub backup_codes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MfaVerifyRequest {
    pub code: String,
    pub backup_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
    pub nombre: String,
    pub rol: String,
}

#[derive(Clone)]
pub struct AuthService {
    db: Surreal<Db>,
    argon2: Argon2<'static>,
}

impl AuthService {
    pub fn new(db: Surreal<Db>) -> Self {
        let params = argon2_params();
        let argon2 = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
        Self { db, argon2 }
    }

    pub async fn register(&self, req: RegisterRequest) -> Result<User, String> {
        let role = parse_role(&req.rol);
        let password_hash = hash_password(&req.password)?;

        let user = User {
            user_id: Uuid::new_v4().to_string(),
            username: req.username.clone(),
            password_hash,
            rol: role,
            nombre: req.nombre,
            activo: true,
            created_at: chrono::Utc::now().to_rfc3339(),
        };

        let created: Option<User> = self
            .db
            .create(("users", user.username.clone()))
            .content(user.clone())
            .await
            .map_err(|e| e.to_string())?;

        created.ok_or_else(|| "Failed to create user".to_string())
    }

    pub async fn authenticate(
        &self,
        username: &str,
        password: &str,
    ) -> Result<LoginResponse, String> {
        // Prefer the canonical keyed lookup (users registered with the username
        // as record id) and fall back to a field search for records created
        // under other ids (e.g. users created via the admin staff API, which
        // store the record id as a UUID). Without this fallback those users
        // could never log in.
        let user: User = match self
            .db
            .select(("users", username))
            .await
            .map_err(|e| e.to_string())?
        {
            Some(u) => u,
            None => {
                let users: Vec<User> = self.db.select("users").await.map_err(|e| e.to_string())?;
                users
                    .into_iter()
                    .find(|u| u.username == username)
                    .ok_or_else(|| "Usuario no encontrado".to_string())?
            }
        };

        if !user.activo {
            return Err("Usuario inactivo".to_string());
        }

        let parsed_hash = PasswordHash::new(&user.password_hash)
            .map_err(|_| "Invalid password hash".to_string())?;

        self.argon2
            .verify_password(password.as_bytes(), &parsed_hash)
            .map_err(|_| "Contraseña incorrecta".to_string())?;

        let permissions = crate::rbac::Role::from(user.rol.clone()).permissions();

        let exp_hours = jwt_expiry_hours();
        let exp = chrono::Utc::now().timestamp() + exp_hours * 3600;
        let iat = chrono::Utc::now().timestamp();

        let claims = Claims {
            sub: user.user_id.clone(),
            username: user.username.clone(),
            rol: user.rol.to_string(),
            permissions,
            exp,
            iat,
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(jwt_secret()),
        )
        .map_err(|e| e.to_string())?;

        Ok(LoginResponse {
            token,
            user: UserInfo::from(&user),
            mfa_required: false,
        })
    }

    pub fn verify_token(&self, token: &str) -> Result<Claims, String> {
        let validation = Validation::default();
        let token_data: TokenData<Claims> =
            decode(token, &DecodingKey::from_secret(jwt_secret()), &validation)
                .map_err(|e| e.to_string())?;

        let claims = token_data.claims;

        if claims.exp < chrono::Utc::now().timestamp() {
            return Err("Token expirado".to_string());
        }

        if is_token_revoked(token) {
            return Err("Token revocado".to_string());
        }

        Ok(claims)
    }

    pub fn verify_token_optional(&self, token: Option<&str>) -> Option<Claims> {
        match token {
            Some(t) => self.verify_token(t).ok(),
            None => None,
        }
    }

    pub async fn get_user(&self, user_id: &str) -> Result<Option<User>, String> {
        let users: Vec<User> = self.db.select("users").await.map_err(|e| e.to_string())?;

        Ok(users.into_iter().find(|u| u.user_id == user_id))
    }

    pub async fn refresh_token(&self, token: &str) -> Result<LoginResponse, String> {
        let claims = self.verify_token(token)?;
        let user: Option<User> = self
            .db
            .select(("users", &claims.username))
            .await
            .map_err(|e| e.to_string())?;
        let user = user.ok_or_else(|| "Usuario no encontrado".to_string())?;

        if !user.activo {
            return Err("Usuario inactivo".to_string());
        }

        let permissions = crate::rbac::Role::from(user.rol.clone()).permissions();

        let exp_hours = jwt_expiry_hours();
        let exp = chrono::Utc::now().timestamp() + exp_hours * 3600;
        let iat = chrono::Utc::now().timestamp();

        let new_claims = Claims {
            sub: user.user_id.clone(),
            username: user.username.clone(),
            rol: user.rol.to_string(),
            permissions,
            exp,
            iat,
        };

        let token = encode(
            &Header::default(),
            &new_claims,
            &EncodingKey::from_secret(jwt_secret()),
        )
        .map_err(|e| e.to_string())?;

        Ok(LoginResponse {
            token,
            user: UserInfo::from(&user),
            mfa_required: false,
        })
    }

    pub async fn list_users(&self) -> Result<Vec<UserInfo>, String> {
        let users: Vec<User> = self.db.select("users").await.map_err(|e| e.to_string())?;

        Ok(users.iter().map(UserInfo::from).collect())
    }
}

pub fn extract_token_from_header(header: &str) -> Option<&str> {
    header.strip_prefix("Bearer ")
}

const ARGON2_M_COST_DEFAULT: u32 = 19456;
const ARGON2_T_COST_DEFAULT: u32 = 3;
const ARGON2_P_COST_DEFAULT: u32 = 1;

fn env_u32(name: &str, default: u32) -> u32 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Argon2id parameters, configurable via DMART_ARGON2_M_COST / DMART_ARGON2_T_COST /
/// DMART_ARGON2_P_COST. Defaults follow OWASP recommendations for interactive logins
/// (~19 MiB, 3 iterations, 1 lane) instead of the previous fixed 64 MiB.
fn argon2_params() -> Params {
    let m = env_u32("DMART_ARGON2_M_COST", ARGON2_M_COST_DEFAULT);
    let t = env_u32("DMART_ARGON2_T_COST", ARGON2_T_COST_DEFAULT);
    let p = env_u32("DMART_ARGON2_P_COST", ARGON2_P_COST_DEFAULT);
    match Params::new(m, t, p, Some(32)) {
        Ok(params) => params,
        Err(e) => {
            tracing::warn!("DMART_ARGON2_* inválidos ({e}); usando 19456/3/1");
            Params::new(
                ARGON2_M_COST_DEFAULT,
                ARGON2_T_COST_DEFAULT,
                ARGON2_P_COST_DEFAULT,
                Some(32),
            )
            .expect("default argon2 params are valid")
        }
    }
}

/// Hashes a plaintext password with the configured Argon2id parameters.
pub fn hash_password(password: &str) -> Result<String, String> {
    let argon2 = Argon2::new(
        argon2::Algorithm::Argon2id,
        argon2::Version::V0x13,
        argon2_params(),
    );
    let salt = SaltString::generate(&mut rand::thread_rng());
    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| format!("Hash error: {}", e))
}

/// Parses a role string (case-insensitive) into a `UserRole`.
pub fn parse_role(s: &str) -> UserRole {
    match s.to_lowercase().as_str() {
        "admin" => UserRole::Admin,
        "medico" | "médico" | "doctor" => UserRole::Medico,
        "enfermero" | "enfermera" | "nurse" => UserRole::Enfermero,
        _ => UserRole::Viewer,
    }
}

/// Seed default admin user if no users exist
pub async fn seed_default_admin(db: &Surreal<Db>) -> Result<bool> {
    let users: Vec<User> = db
        .select("users")
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    if !users.is_empty() {
        tracing::info!("👥 Found {} users, skipping seed", users.len());
        return Ok(false);
    }

    tracing::info!("🌱 Seeding default admin user...");

    let admin_password = std::env::var("DMART_ADMIN_PASSWORD")
        .ok()
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| {
            let generated = Uuid::new_v4().to_string();
            tracing::warn!(
                "⚠️ DMART_ADMIN_PASSWORD no configurada. Contraseña temporal del admin (se muestra una sola vez): {generated}"
            );
            generated
        });

    let password_hash = hash_password(&admin_password).map_err(anyhow::Error::msg)?;

    let user = User {
        user_id: Uuid::new_v4().to_string(),
        username: "admin".to_string(),
        password_hash: password_hash.to_string(),
        rol: UserRole::Admin,
        nombre: "Administrador".to_string(),
        activo: true,
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    let created: Option<User> = db
        .create(("users", "admin"))
        .content(user)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    match created {
        Some(_) => {
            tracing::info!("✅ Admin user created with password from DMART_ADMIN_PASSWORD env var");
            Ok(true)
        }
        None => Err(anyhow::anyhow!("Failed to create admin user")),
    }
}
