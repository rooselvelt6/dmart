use anyhow::Result;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use jsonwebtoken::{encode, decode, EncodingKey, DecodingKey, Header, TokenData, Validation};
use serde::{Serialize, Deserialize};
use surrealdb::engine::local::Db;
use surrealdb::Surreal;
use dmart_shared::models::*;

const JWT_SECRET: &[u8] = b"dmart-uci-secure-key-2024-v1";
const TOKEN_EXPIRES_HOURS: i64 = 24;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub username: String,
    pub rol: UserRole,
    pub exp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthError {
    pub message: String,
}

impl From<&str> for AuthError {
    fn from(s: &str) -> Self {
        Self { message: s.to_string() }
    }
}

pub async fn create_user(db: &Surreal<Db>, username: &str, password: &str, nombre: &str, rol: UserRole) -> Result<User> {
    let argon2 = Argon2::default();
    let password_hash = argon2.hash_password(password.as_bytes(), &Default::default())
        .map_err(|e| anyhow::anyhow!("Hash error: {}", e))?
        .to_string();
    
    let user = User {
        user_id: Uuid::new_v4().to_string(),
        username: username.to_string(),
        password_hash,
        rol,
        nombre: nombre.to_string(),
        activo: true,
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    
    let created: Option<User> = db
        .create(("users", user.username.clone()))
        .content(user.clone())
        .await?;
    
    created.ok_or_else(|| anyhow::anyhow!("Failed to create user"))
}

pub async fn authenticate(db: &Surreal<Db>, username: &str, password: &str) -> Result<LoginResponse, String> {
    let user: Option<User> = db.select(("users", username)).await
        .map_err(|e| e.to_string())?;
    
    let user = user.ok_or_else(|| "Usuario no encontrado".to_string())?;
    
    if !user.activo {
        return Err("Usuario inactivo".to_string());
    }
    
    let parsed_hash = PasswordHash::new(&user.password_hash)
        .map_err(|e| e.to_string())?;
    
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .map_err(|_| "Contraseña incorrecta".to_string())?;
    
    let claims = Claims {
        sub: user.user_id.clone(),
        username: user.username.clone(),
        rol: user.rol.clone(),
        exp: chrono::Utc::now().timestamp() + (TOKEN_EXPIRES_HOURS * 3600),
    };
    
    let token = encode(&Header::default(), &claims, EncodingKey::from_secret(JWT_SECRET))
        .map_err(|e| e.to_string())?;
    
    Ok(LoginResponse {
        token,
        user: UserInfo::from(&user),
    })
}

pub fn verify_token(token: &str) -> Result<Claims, String> {
    let validation = Validation::default();
    let token_data: TokenData<Claims> = decode(token, DecodingKey::from_secret(JWT_SECRET), &validation)
        .map_err(|e| e.to_string())?;
    
    Ok(token_data.claims)
}

pub async fn get_user_by_id(db: &Surreal<Db>, user_id: &str) -> Result<Option<User>> {
    let users: Vec<User> = db.select("users").await?;
    Ok(users.into_iter().find(|u| u.user_id == user_id))
}

pub async fn list_users(db: &Surreal<Db>) -> Result<Vec<UserInfo>> {
    let users: Vec<User> = db.select("users").await?;
    Ok(users.iter().map(UserInfo::from).collect())
}