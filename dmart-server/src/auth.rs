use anyhow::Result;
use argon2::{
    Argon2, Params,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
};
use axum::{
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use jsonwebtoken::{DecodingKey, EncodingKey, Header, TokenData, Validation, decode, encode};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::OnceLock;
use surrealdb::Surreal;
use surrealdb::engine::local::Db;
use tracing;
use uuid::Uuid;
use zeroize::{Zeroize, ZeroizeOnDrop};

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

/// TTL of the access token in seconds.
///
/// Prefers `JWT_EXPIRY_MINUTES` (default 15 min, clamped 1..=480); keeps the
/// legacy `JWT_EXPIRY_HOURS` as an override for existing deployments.
fn jwt_expiry_seconds() -> i64 {
    static JWT_EXPIRY: OnceLock<i64> = OnceLock::new();
    *JWT_EXPIRY.get_or_init(|| {
        if let Ok(minutes) = std::env::var("JWT_EXPIRY_MINUTES")
            && let Ok(v) = minutes.parse::<i64>()
        {
            return v.clamp(1, 480) * 60;
        }
        if let Ok(hours) = std::env::var("JWT_EXPIRY_HOURS")
            && let Ok(v) = hours.parse::<i64>()
        {
            return v.clamp(1, 24) * 3600;
        }
        15 * 60
    })
}

/// Validez (segundos) del token de reto emitido tras el primer factor MFA.
const MFA_CHALLENGE_SECONDS: i64 = 5 * 60;

/// Validez (días) del refresh token.
const REFRESH_TOKEN_TTL_DAYS: i64 = 7;

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
///
/// Keyed by the `jti` claim (stable across rotations) instead of the raw token
/// string, so a single canonical revocation id is used per issued token.
pub async fn persist_revoked_jti(jti: &str, exp: i64) {
    if jti.is_empty() {
        return;
    }
    if crate::cache::cache_available() {
        let ttl = (exp - chrono::Utc::now().timestamp()).max(1) as u64;
        let _ = crate::cache::cache_set(&format!("blacklist:access:{}", jti), "1", ttl).await;
    }
}

/// Checks the distributed (Valkey) revocation list for an access token's `jti`.
pub async fn is_jti_revoked_in_cache(jti: &str) -> bool {
    !jti.is_empty()
        && crate::cache::cache_available()
        && crate::cache::cache_get(&format!("blacklist:access:{}", jti))
            .await
            .is_some()
}

/// Revocación por usuario con epoch: todos los access tokens emitidos con
/// `iat` anterior al corte quedan invalidados de forma inmediata (respuesta a
/// un posible robo / revocación de todas las sesiones). Se conserva el máximo
/// valor de corte en memoria y en Valkey (`blacklist:user:{user_id}`).
static REVOKED_USER_BEFORE: OnceLock<std::sync::Mutex<HashMap<String, i64>>> = OnceLock::new();

/// Registra el corte de revocación en memoria. Afecta a tokens con `iat` menor
/// al valor ya registrado (se toma el máximo).
pub fn revoke_user_access_before(user_id: &str, cutoff: i64) {
    let mut map = REVOKED_USER_BEFORE
        .get_or_init(Default::default)
        .lock()
        .unwrap();
    let entry = map.entry(user_id.to_string()).or_insert(cutoff);
    *entry = (*entry).max(cutoff);
}

fn is_user_access_revoked(user_id: &str, iat: i64) -> bool {
    let map = REVOKED_USER_BEFORE
        .get_or_init(Default::default)
        .lock()
        .unwrap();
    // `<=` porque iat y el corte tienen resolución de 1 s: un token emitido
    // en el mismo segundo que la detección de robo también debe caer.
    map.get(user_id)
        .map(|cutoff| iat <= *cutoff)
        .unwrap_or(false)
}

/// Propaga la revocación por usuario a Valkey (TTL cubre la vida máxima de un
/// refresh, para que el corte siga afectando a tokens alineados con la sesión).
pub async fn persist_user_revocation(user_id: &str, cutoff: i64) {
    if crate::cache::cache_available() {
        let _ = crate::cache::cache_set(
            &format!("blacklist:user:{}", user_id),
            &cutoff.to_string(),
            (REFRESH_TOKEN_TTL_DAYS * 86_400) as u64,
        )
        .await;
    }
}

/// Comprueba la revocación por usuario en Valkey.
pub async fn is_user_access_revoked_in_cache(user_id: &str, iat: i64) -> bool {
    if crate::cache::cache_available()
        && let Some(v) = crate::cache::cache_get(&format!("blacklist:user:{}", user_id)).await
        && let Ok(cutoff) = v.parse::<i64>()
    {
        return iat <= cutoff;
    }
    false
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub username: String,
    pub rol: String,
    pub permissions: Vec<String>,
    pub exp: i64,
    pub iat: i64,
    #[serde(default)]
    pub jti: String,
    #[serde(default = "default_token_scope")]
    pub scope: String,
    /// Slug del tenant/hospital (SPEC-025). Default: "default" (single-tenant).
    #[serde(default = "dmart_shared::models::default_tenant_id")]
    pub tenant_id: String,
}

fn default_token_scope() -> String {
    "session".to_string()
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

#[derive(Debug, Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct LoginResponse {
    /// Access token (nombre histórico, mantiene compatibilidad API/frontend).
    pub token: String,
    pub access_token: String,
    /// Segundos de validez del access token.
    pub expires_in: i64,
    /// Refresh token rotativo single-use. Vacio en el reto MFA.
    pub refresh_token: String,
    #[zeroize(skip)]
    pub user: UserInfo,
    pub mfa_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct RefreshRequest {
    pub refresh_token: Option<String>,
}

/// Registro persistido de un refresh token. `token_hash` es un SHA-256 del
/// token (entropia: 256 bits aleatorios => lookup indexado O(1); Argon2id se
/// reserva para passwords de baja entropia). Los registros revocados se
/// conservan para poder detectar reuso (posible robo) y revocar la familia.
#[derive(Debug, Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
struct RefreshTokenRecord {
    token_hash: String,
    user_id: String,
    expires_at: i64,
    revoked: bool,
    created_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ip_address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct MfaSetupResponse {
    pub secret: String,
    pub qr_code: String,
    pub backup_codes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
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
    /// Slug del tenant (SPEC-025). Default: "default".
    #[serde(default = "dmart_shared::models::default_tenant_id")]
    pub tenant_id: String,
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
            tenant_id: req.tenant_id,
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
        user_agent: Option<String>,
        ip_address: Option<String>,
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
                let users: Vec<User> = self
                    .db
                    .query("SELECT * FROM users WHERE username = $username LIMIT 1")
                    .bind(("username", username.to_string()))
                    .await
                    .map_err(|e| e.to_string())?
                    .take(0)
                    .map_err(|e| e.to_string())?;
                users
                    .into_iter()
                    .next()
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

        let mfa_enabled = crate::db::get_mfa_settings(&self.db, &user.user_id)
            .await
            .map(|s| s.map(|s| s.enabled).unwrap_or(false))
            .unwrap_or(false);

        if mfa_enabled {
            // Primer factor correcto: emite un token de reto de corta duración.
            // La sesión solo se completa tras validar el código TOTP.
            let iat = chrono::Utc::now().timestamp();
            let exp = iat + MFA_CHALLENGE_SECONDS;
            let claims = Claims {
                sub: user.user_id.clone(),
                username: user.username.clone(),
                rol: user.rol.to_string(),
                permissions,
                exp,
                iat,
                jti: Uuid::new_v4().to_string(),
                scope: "mfa".to_string(),
                tenant_id: user.tenant_id.clone(),
            };
            let token = encode(
                &Header::default(),
                &claims,
                &EncodingKey::from_secret(jwt_secret()),
            )
            .map_err(|e| e.to_string())?;

            return Ok(LoginResponse {
                token: token.clone(),
                access_token: token,
                expires_in: MFA_CHALLENGE_SECONDS,
                refresh_token: String::new(),
                user: UserInfo::from(&user),
                mfa_required: true,
            });
        }

        self.issue_session(user, user_agent, ip_address).await
    }

    /// Construye un par access token (claims firmados + `jti`) y refresh token
    /// rotativo (persistido con hash para lookup O(1)).
    async fn issue_session(
        &self,
        user: User,
        user_agent: Option<String>,
        ip_address: Option<String>,
    ) -> Result<LoginResponse, String> {
        let permissions = crate::rbac::Role::from(user.rol.clone()).permissions();
        let refresh_token = self
            .store_refresh_token(&user.user_id, user_agent, ip_address)
            .await?;

        let access_token = self.encode_access_token(&user, permissions, "session")?;

        Ok(LoginResponse {
            token: access_token.clone(),
            access_token,
            expires_in: jwt_expiry_seconds(),
            refresh_token,
            user: UserInfo::from(&user),
            mfa_required: false,
        })
    }

    /// Emite el token de sesión completo tras una verificación MFA satisfactoria.
    pub async fn complete_mfa_login(
        &self,
        user_id: &str,
        user_agent: Option<String>,
        ip_address: Option<String>,
    ) -> Result<LoginResponse, String> {
        let user: User = self
            .get_user(user_id)
            .await?
            .ok_or_else(|| "Usuario no encontrado".to_string())?;

        if !user.activo {
            return Err("Usuario inactivo".to_string());
        }

        self.issue_session(user, user_agent, ip_address).await
    }

    fn encode_access_token(
        &self,
        user: &User,
        permissions: Vec<String>,
        scope: &str,
    ) -> Result<String, String> {
        let iat = chrono::Utc::now().timestamp();
        let exp = iat
            + if scope == "mfa" {
                MFA_CHALLENGE_SECONDS
            } else {
                jwt_expiry_seconds()
            };
        let claims = Claims {
            sub: user.user_id.clone(),
            username: user.username.clone(),
            rol: user.rol.to_string(),
            permissions,
            exp,
            iat,
            jti: Uuid::new_v4().to_string(),
            scope: scope.to_string(),
            tenant_id: user.tenant_id.clone(),
        };
        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(jwt_secret()),
        )
        .map_err(|e| e.to_string())
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

        if is_user_access_revoked(&claims.sub, claims.iat) {
            return Err("Sesión revocada".to_string());
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
        let users: Vec<User> = self
            .db
            .query("SELECT * FROM users WHERE user_id = $user_id LIMIT 1")
            .bind(("user_id", user_id.to_string()))
            .await
            .map_err(|e| e.to_string())?
            .take(0)
            .map_err(|e| e.to_string())?;
        Ok(users.into_iter().next())
    }

    pub async fn list_users(&self) -> Result<Vec<UserInfo>, String> {
        let users: Vec<User> = self.db.select("users").await.map_err(|e| e.to_string())?;

        Ok(users.iter().map(UserInfo::from).collect())
    }

    /// Valida y rota un refresh token (single-use). Si se presenta un token ya
    /// consumido (`revoked`), se presume posible robo y se revocan todas las
    /// sesiones del usuario (reuse detection).
    pub async fn rotate_refresh_token(
        &self,
        token: &str,
        user_agent: Option<String>,
        ip_address: Option<String>,
    ) -> Result<LoginResponse, String> {
        let now = chrono::Utc::now().timestamp();
        let record = self
            .find_refresh_token(token)
            .await?
            .ok_or_else(|| "Refresh token inválido".to_string())?;

        if record.revoked {
            // Reuso de un token ya consumido: posible robo. Se revoca la
            // familia completa (refresh + todos los access tokens pendientes).
            self.revoke_all_refresh_tokens(&record.user_id).await;
            let cutoff = chrono::Utc::now().timestamp();
            revoke_user_access_before(&record.user_id, cutoff);
            persist_user_revocation(&record.user_id, cutoff).await;
            return Err("Refresh token reutilizado: sesión revocada".to_string());
        }
        if record.expires_at < now {
            return Err("Refresh token expirado".to_string());
        }

        let user = self
            .get_user(&record.user_id)
            .await?
            .ok_or_else(|| "Usuario no encontrado".to_string())?;
        if !user.activo {
            return Err("Usuario inactivo".to_string());
        }

        self.revoke_refresh_token(token).await?;
        self.issue_session(user, user_agent, ip_address).await
    }

    /// Revoca un refresh token concreto (logout).
    pub async fn revoke_refresh_token(&self, token: &str) -> Result<(), String> {
        let hash = hash_refresh_token(token);
        self.db
            .query("UPDATE refresh_token SET revoked = true WHERE token_hash = $hash")
            .bind(("hash", hash))
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Revoca todas las sesiones (refresh tokens) de un usuario. No se borran
    /// los registros: se marcan para que la detección de reuso siga siendo
    /// posible sobre tokens ya presentados.
    pub async fn revoke_all_refresh_tokens(&self, user_id: &str) {
        let _ = self
            .db
            .query("UPDATE refresh_token SET revoked = true WHERE user_id = $uid")
            .bind(("uid", user_id.to_string()))
            .await;
    }

    async fn store_refresh_token(
        &self,
        user_id: &str,
        user_agent: Option<String>,
        ip_address: Option<String>,
    ) -> Result<String, String> {
        let token = generate_refresh_token();
        let record = RefreshTokenRecord {
            token_hash: hash_refresh_token(&token),
            user_id: user_id.to_string(),
            expires_at: chrono::Utc::now().timestamp() + REFRESH_TOKEN_TTL_DAYS * 86_400,
            revoked: false,
            created_at: chrono::Utc::now().timestamp(),
            user_agent,
            ip_address,
        };
        let _: Option<RefreshTokenRecord> = self
            .db
            .create("refresh_token")
            .content(record)
            .await
            .map_err(|e| e.to_string())?;
        Ok(token)
    }

    async fn find_refresh_token(&self, token: &str) -> Result<Option<RefreshTokenRecord>, String> {
        let hash = hash_refresh_token(token);
        let rows: Vec<RefreshTokenRecord> = self
            .db
            .query("SELECT * FROM refresh_token WHERE token_hash = $hash LIMIT 1")
            .bind(("hash", hash))
            .await
            .map_err(|e| e.to_string())?
            .take(0)
            .map_err(|e| e.to_string())?;
        Ok(rows.into_iter().next())
    }
}

pub fn extract_token_from_header(header: &str) -> Option<&str> {
    header.strip_prefix("Bearer ")
}

/// Genera un refresh token opaco de alta entropía (256 bits) en base64url.
fn generate_refresh_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Hash de lookup de un refresh token (SHA-256 determinista, indexado O(1)).
pub fn hash_refresh_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Construye la cookie httpOnly del refresh token.
///
/// `Secure` se habilita por defecto (solo via HTTPS) y puede desactivarse en
/// desarrollo local (`DMART_COOKIE_SECURE=false`). `Path=/api/auth` restringe
/// el envío a los endpoints de sesión.
pub fn refresh_cookie(token: &str) -> String {
    let secure = std::env::var("DMART_COOKIE_SECURE")
        .map(|v| v != "false" && v != "0")
        .unwrap_or(true);
    format!(
        "refresh_token={}; HttpOnly; SameSite=Strict; Path=/api/auth; Max-Age={};{}",
        token,
        REFRESH_TOKEN_TTL_DAYS * 86_400,
        if secure { " Secure" } else { "" }
    )
}

/// Cookie de borrado del refresh token (logout, Max-Age=0).
pub fn clear_refresh_cookie() -> String {
    let secure = std::env::var("DMART_COOKIE_SECURE")
        .map(|v| v != "false" && v != "0")
        .unwrap_or(true);
    format!(
        "refresh_token=; HttpOnly; SameSite=Strict; Path=/api/auth; Max-Age=0;{}",
        if secure { " Secure" } else { "" }
    )
}

/// Extrae el refresh token de una cookie httpOnly (si viene en la request).
pub fn refresh_token_from_cookie(headers: &axum::http::HeaderMap) -> Option<String> {
    headers
        .get(axum::http::header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|c| c.trim().strip_prefix("refresh_token=").map(str::to_string))
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

/// Verifica una contraseña en texto plano contra un hash Argon2id.
pub fn verify_password(password: &str, hash: &str) -> bool {
    let argon2 = Argon2::new(
        argon2::Algorithm::Argon2id,
        argon2::Version::V0x13,
        argon2_params(),
    );
    match PasswordHash::new(hash) {
        Ok(parsed) => argon2.verify_password(password.as_bytes(), &parsed).is_ok(),
        Err(_) => false,
    }
}

/// Parses a role string (case-insensitive) into a `UserRole`.
pub fn parse_role(s: &str) -> UserRole {
    match s.to_lowercase().as_str() {
        "admin" => UserRole::Admin,
        "medico" | "médico" | "doctor" => UserRole::Medico,
        "enfermero" | "enfermera" | "nurse" => UserRole::Enfermero,
        "soporte" | "support" | "soporte técnico" => UserRole::Soporte,
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
        tenant_id: dmart_shared::models::default_tenant_id(),
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
