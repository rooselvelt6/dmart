use anyhow::Result;
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2, Params,
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, TokenData, Validation};
use serde::{Deserialize, Serialize};
use surrealdb::engine::local::Db;
use surrealdb::Surreal;
use uuid::Uuid;

use dmart_shared::models::*;

const JWT_SECRET: &[u8] = b"dmart-uci-jwt-secret-key-2024-secure";

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

pub struct AuthService {
    db: Surreal<Db>,
    argon2: Argon2<'static>,
}

impl AuthService {
    pub fn new(db: Surreal<Db>) -> Self {
        let params = Params::new(65536, 3, 4, Some(32)).unwrap();
        let argon2 = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
        Self { db, argon2 }
    }

    pub async fn register(&self, req: RegisterRequest) -> Result<User, String> {
        let role = match req.rol.as_str() {
            "admin" => UserRole::Admin,
            "medico" => UserRole::Medico,
            "enfermero" => UserRole::Enfermero,
            _ => UserRole::Viewer,
        };

        let salt = SaltString::generate(&mut rand::thread_rng());
        let password_hash = self
            .argon2
            .hash_password(req.password.as_bytes(), &salt)
            .map_err(|e| format!("Hash error: {}", e))?
            .to_string();

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

    pub async fn authenticate(&self, username: &str, password: &str) -> Result<LoginResponse, String> {
        let user: Option<User> = self
            .db
            .select(("users", username))
            .await
            .map_err(|e| e.to_string())?;

        let user = user.ok_or_else(|| "Usuario no encontrado".to_string())?;

        if !user.activo {
            return Err("Usuario inactivo".to_string());
        }

        let parsed_hash =
            PasswordHash::new(&user.password_hash).map_err(|_| "Invalid password hash".to_string())?;

        self.argon2
            .verify_password(password.as_bytes(), &parsed_hash)
            .map_err(|_| "Contraseña incorrecta".to_string())?;

        let permissions = match user.rol {
            UserRole::Admin => vec!["*".to_string()],
            UserRole::Medico => vec!["patients:read".to_string(), "patients:create".to_string(), "measurements:*".to_string()],
            UserRole::Enfermero => vec!["patients:read".to_string(), "measurements:*".to_string()],
            UserRole::Viewer => vec!["patients:read".to_string()],
        };
        
        let exp = chrono::Utc::now().timestamp() + 3600;
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
            &EncodingKey::from_secret(JWT_SECRET),
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
            decode(token, &DecodingKey::from_secret(JWT_SECRET), &validation)
                .map_err(|e| e.to_string())?;

        let claims = token_data.claims;

        if claims.exp < chrono::Utc::now().timestamp() {
            return Err("Token expirado".to_string());
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
            .select("users")
            .await
            .map_err(|e| e.to_string())?;

        Ok(users.into_iter().find(|u| u.user_id == user_id))
    }

    pub async fn list_users(&self) -> Result<Vec<UserInfo>, String> {
        let users: Vec<User> = self
            .db
            .select("users")
            .await
            .map_err(|e| e.to_string())?;

        Ok(users.iter().map(UserInfo::from).collect())
    }
}

pub fn extract_token_from_header(header: &str) -> Option<&str> {
    if header.starts_with("Bearer ") {
        Some(&header[7..])
    } else {
        None
    }
}