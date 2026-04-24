use axum::{
    Router,
    routing::{post, get},
    extract::State,
    http::StatusCode,
    Json,
};
use dmart_shared::models::*;
use crate::db::Database;
use crate::auth::{AuthService, LoginRequest, LoginResponse, RegisterRequest};

pub fn router() -> Router<Database> {
    Router::new()
        .route("/login", post(login))
        .route("/logout", post(logout))
        .route("/me", get(me))
        .route("/users", get(list_users))
        .route("/register", post(register))
}

async fn login(
    State(db): State<Database>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<ApiResponse<LoginResponse>>, StatusCode> {
    let auth_service = AuthService::new((*db).clone());
    match auth_service.authenticate(&req.username, &req.password).await {
        Ok(response) => Ok(Json(ApiResponse::ok(response))),
        Err(e) => Ok(Json(ApiResponse::err(e))),
    }
}

async fn logout() -> Result<Json<ApiResponse<()>>, StatusCode> {
    Ok(Json(ApiResponse::ok(())))
}

async fn me() -> Result<Json<ApiResponse<UserInfo>>, StatusCode> {
    Ok(Json(ApiResponse::ok(UserInfo {
        user_id: "demo".to_string(),
        username: "demo".to_string(),
        rol: UserRole::Admin,
        nombre: "Usuario Demo".to_string(),
    })))
}

async fn list_users(
    State(db): State<Database>,
) -> Result<Json<ApiResponse<Vec<UserInfo>>>, StatusCode> {
    let auth_service = AuthService::new((*db).clone());
    match auth_service.list_users().await {
        Ok(users) => Ok(Json(ApiResponse::ok(users))),
        Err(e) => Ok(Json(ApiResponse::err(e.to_string()))),
    }
}

async fn register(
    State(db): State<Database>,
    Json(req): Json<RegisterRequest>,
) -> Result<Json<ApiResponse<User>>, StatusCode> {
    let auth_service = AuthService::new((*db).clone());
    match auth_service.register(req).await {
        Ok(user) => Ok(Json(ApiResponse::ok(user))),
        Err(e) => Ok(Json(ApiResponse::err(e))),
    }
}