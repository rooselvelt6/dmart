use crate::auth::{
    AuthService, Claims, LoginRequest, LoginResponse, RegisterRequest, extract_token_from_header,
    parse_role,
};
use crate::db::Database;
use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use dmart_shared::models::*;

pub fn router() -> Router<Database> {
    Router::new()
        .route("/login", post(login))
        .route("/logout", post(logout))
        .route("/me", get(me))
        .route("/users", get(list_users))
        .route("/register", post(register))
        .route("/refresh", post(refresh))
}

async fn login(State(db): State<Database>, Json(req): Json<LoginRequest>) -> Response {
    let auth_service = AuthService::new((*db).clone());
    match auth_service
        .authenticate(&req.username, &req.password)
        .await
    {
        Ok(response) => {
            if let Some(audit) = crate::audit::audit() {
                let _ = audit
                    .log_login_success(&response.user.user_id, &response.user.username, None)
                    .await;
            }
            (StatusCode::OK, Json(ApiResponse::ok(response))).into_response()
        }
        Err(e) => {
            if let Some(audit) = crate::audit::audit() {
                let _ = audit.log_login_failed(&req.username, &e, None).await;
            }
            (
                StatusCode::UNAUTHORIZED,
                Json(ApiResponse::<LoginResponse>::err(e)),
            )
                .into_response()
        }
    }
}

async fn logout(headers: HeaderMap, claims: Claims) -> Result<Json<ApiResponse<()>>, StatusCode> {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(extract_token_from_header)
        .unwrap_or("");
    crate::auth::revoke_token(token, claims.exp);
    crate::auth::persist_revoked_token(token, claims.exp).await;
    Ok(Json(ApiResponse::ok(())))
}

async fn me(
    State(db): State<Database>,
    claims: Claims,
) -> Result<Json<ApiResponse<UserInfo>>, StatusCode> {
    let auth_service = AuthService::new((*db).clone());
    let user_info = match auth_service.get_user(&claims.sub).await {
        Ok(Some(u)) => UserInfo::from(&u),
        _ => UserInfo {
            user_id: claims.sub.clone(),
            username: claims.username.clone(),
            rol: parse_role(&claims.rol),
            nombre: claims.username.clone(),
        },
    };
    Ok(Json(ApiResponse::ok(user_info)))
}

async fn list_users(State(db): State<Database>, claims: Claims) -> Response {
    if !claims.has_permission("*") {
        return (
            StatusCode::FORBIDDEN,
            Json(ApiResponse::<Vec<UserInfo>>::err("Solo administradores")),
        )
            .into_response();
    }
    let auth_service = AuthService::new((*db).clone());
    match auth_service.list_users().await {
        Ok(users) => (StatusCode::OK, Json(ApiResponse::ok(users))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<Vec<UserInfo>>::err(e)),
        )
            .into_response(),
    }
}

async fn register(
    State(db): State<Database>,
    claims: Claims,
    Json(req): Json<RegisterRequest>,
) -> Response {
    if !claims.has_permission("*") {
        return (
            StatusCode::FORBIDDEN,
            Json(ApiResponse::<UserInfo>::err(
                "Solo administradores pueden crear usuarios",
            )),
        )
            .into_response();
    }
    let auth_service = AuthService::new((*db).clone());
    match auth_service.register(req).await {
        Ok(user) => (
            StatusCode::CREATED,
            Json(ApiResponse::ok(UserInfo::from(&user))),
        )
            .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<UserInfo>::err(e)),
        )
            .into_response(),
    }
}

async fn refresh(
    State(db): State<Database>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<LoginResponse>>, StatusCode> {
    let auth_service = AuthService::new((*db).clone());
    let auth_header = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let token = match extract_token_from_header(auth_header) {
        Some(t) => t,
        None => return Ok(Json(ApiResponse::err("Token no proporcionado"))),
    };
    match auth_service.refresh_token(token).await {
        Ok(response) => Ok(Json(ApiResponse::ok(response))),
        Err(e) => Ok(Json(ApiResponse::err(e))),
    }
}
