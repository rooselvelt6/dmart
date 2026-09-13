use crate::auth::{
    AuthService, Claims, LoginRequest, LoginResponse, RefreshRequest, RegisterRequest,
    extract_token_from_header, parse_role,
};
use crate::db::Database;
use axum::{
    Json, Router,
    extract::{ConnectInfo, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use dmart_shared::models::*;
use std::net::SocketAddr;

pub fn router() -> Router<Database> {
    Router::new()
        .route("/login", post(login))
        .route("/logout", post(logout))
        .route("/me", get(me))
        .route("/users", get(list_users))
        .route("/register", post(register))
        .route("/refresh", post(refresh))
        .route("/revoke-all", post(revoke_all))
        .route("/mfa/setup", post(crate::mfa::setup))
        .route("/mfa/confirm", post(crate::mfa::confirm))
        .route("/mfa/verify", post(crate::mfa::verify))
        .route("/mfa/disable", post(crate::mfa::disable))
}

/// Adjunta la cookie httpOnly del refresh token a una respuesta, si el login lo
/// provee (no ocurre en el reto MFA, que aún no ha emitido sesión).
fn apply_refresh_cookie(resp: &mut Response, refresh_token: &str) {
    if !refresh_token.is_empty()
        && let Ok(value) = HeaderValue::from_str(&crate::auth::refresh_cookie(refresh_token))
    {
        resp.headers_mut().insert(header::SET_COOKIE, value);
    }
}

async fn login(
    State(db): State<Database>,
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(req): Json<LoginRequest>,
) -> Response {
    let auth_service = AuthService::new((*db).clone());
    let user_agent = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let ip_address = Some(addr.ip().to_string());

    match auth_service
        .authenticate(&req.username, &req.password, user_agent, ip_address)
        .await
    {
        Ok(response) => {
            if let Some(audit) = crate::audit::audit() {
                let _ = audit
                    .log_login_success(&response.user.user_id, &response.user.username, None)
                    .await;
            }
            let mut resp =
                (StatusCode::OK, Json(ApiResponse::ok(response.clone()))).into_response();
            apply_refresh_cookie(&mut resp, &response.refresh_token);
            resp
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

async fn logout(State(db): State<Database>, headers: HeaderMap, claims: Claims) -> Response {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(extract_token_from_header)
        .unwrap_or("");
    crate::auth::revoke_token(token, claims.exp);
    crate::auth::persist_revoked_jti(&claims.jti, claims.exp).await;

    // Revoca también el refresh token (sesión completa) si viene por cookie.
    if let Some(rt) = crate::auth::refresh_token_from_cookie(&headers) {
        let auth_service = AuthService::new((*db).clone());
        let _ = auth_service.revoke_refresh_token(&rt).await;
    }

    let mut resp = (StatusCode::OK, Json(ApiResponse::ok(()))).into_response();
    if let Ok(value) = HeaderValue::from_str(&crate::auth::clear_refresh_cookie()) {
        resp.headers_mut().insert(header::SET_COOKIE, value);
    }
    resp
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

async fn list_users(State(db): State<Database>) -> Response {
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

async fn register(State(db): State<Database>, Json(req): Json<RegisterRequest>) -> Response {
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

/// Rota el refresh token (single-use). El token se lee de la cookie httpOnly o,
/// para clientes no-navegador, del cuerpo JSON. Nunca del access token.
async fn refresh(
    State(db): State<Database>,
    headers: HeaderMap,
    body: Option<Json<RefreshRequest>>,
) -> Response {
    let auth_service = AuthService::new((*db).clone());
    let user_agent = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);

    let token = crate::auth::refresh_token_from_cookie(&headers)
        .or_else(|| body.and_then(|Json(b)| b.refresh_token));

    let Some(token) = token else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::<LoginResponse>::err(
                "Refresh token requerido".to_string(),
            )),
        )
            .into_response();
    };

    match auth_service
        .rotate_refresh_token(&token, user_agent, None)
        .await
    {
        Ok(response) => {
            let mut resp =
                (StatusCode::OK, Json(ApiResponse::ok(response.clone()))).into_response();
            apply_refresh_cookie(&mut resp, &response.refresh_token);
            resp
        }
        Err(e) => (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::<LoginResponse>::err(e)),
        )
            .into_response(),
    }
}

/// Revoca todas las sesiones (refresh tokens) del usuario actual, incluido el
/// acceso en curso.
async fn revoke_all(State(db): State<Database>, headers: HeaderMap, claims: Claims) -> Response {
    let auth_service = AuthService::new((*db).clone());
    auth_service.revoke_all_refresh_tokens(&claims.sub).await;

    // Invalida también todos los access tokens pendientes (epoch de usuario).
    let cutoff = chrono::Utc::now().timestamp();
    crate::auth::revoke_user_access_before(&claims.sub, cutoff);
    crate::auth::persist_user_revocation(&claims.sub, cutoff).await;

    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(extract_token_from_header)
        .unwrap_or("");
    crate::auth::revoke_token(token, claims.exp);
    crate::auth::persist_revoked_jti(&claims.jti, claims.exp).await;

    (StatusCode::OK, Json(ApiResponse::ok(()))).into_response()
}
