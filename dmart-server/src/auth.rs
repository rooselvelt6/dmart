use anyhow::Result;
use bcrypt::{DEFAULT_COST, hash, verify};
use jsonwebtoken::{encode, decode, EncodingKey, DecodingKey, Header, TokenData, Validation};
use serde::{Serialize, Deserialize};
use surrealdb::engine::local::Db;
use surrealdb::Surreal;
use dmart_shared::models::*;
use uuid::Uuid;

const JWT_SECRET: &[u8] = b"dmart-uci-secure-key-2024-v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub username: String,
    pub rol: UserRole,
    pub exp: i64,
}

pub async fn create_user(db: &Surreal<Db>, username: &str, password: &str, nombre: &str, rol: UserRole) -> Result<User> {
    let password_hash = hash(password.as_bytes(), DEFAULT_COST)
        .map_err(|e| anyhow::anyhow!("Hash error: {}", e))?;
    
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
    
    verify(password.as_bytes(), &user.password_hash)
        .map_err(|_| "Contraseña incorrecta".to_string())?;
    
    let claims = Claims {
        sub: user.user_id.clone(),
        username: user.username.clone(),
        rol: user.rol.clone(),
        exp: chrono::Utc::now().timestamp() + 86400,
    };
    
    let token = encode(&Header::default(), &claims, &EncodingKey::from_secret(JWT_SECRET))
        .map_err(|e| e.to_string())?;
    
    Ok(LoginResponse {
        token,
        user: UserInfo::from(&user),
    })
}

pub fn verify_token(token: &str) -> Result<Claims, String> {
    let validation = Validation::default();
    let token_data: TokenData<Claims> = decode(token, &DecodingKey::from_secret(JWT_SECRET), &validation)
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