use axum::{
    Router,
    routing::{post, get},
    extract::State,
    http::StatusCode,
    Json,
};
use dmart_shared::models::*;
use crate::db::Database;
use crate::auth;

pub fn router() -> Router<Database> {
    Router::new()
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        .route("/auth/me", get(me))
        .route("/auth/users", get(list_users))
}

async fn login(
    State(db): State<Database>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<ApiResponse<LoginResponse>>, StatusCode> {
    match auth::authenticate(&db, &req.username, &req.password).await {
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
    match auth::list_users(&db).await {
        Ok(users) => Ok(Json(ApiResponse::ok(users))),
        Err(e) => Ok(Json(ApiResponse::err(e.to_string()))),
    }
}