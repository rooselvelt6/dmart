use axum::{
    body::Body,
    extract::{Request, State},
    middleware::Next,
    response::Response,
    Json,
};
use serde_json::json;

use crate::auth::{extract_token_from_header, AuthService, Claims};
use dmart_shared::models::ApiResponse;

pub struct AuthMiddlewareConfig {
    pub auth_service: AuthService,
    pub required_paths: Vec<String>,
    pub open_paths: Vec<String>,
}

impl AuthMiddlewareConfig {
    pub fn new(auth_service: AuthService) -> Self {
        Self {
            auth_service,
            required_paths: vec![
                "/api/patients".to_string(),
                "/api/measurements".to_string(),
                "/api/stats".to_string(),
            ],
            open_paths: vec![
                "/api/health".to_string(),
                "/api/auth/login".to_string(),
                "/api/auth/register".to_string(),
            ],
        }
    }

    pub fn is_path_open(&self, path: &str) -> bool {
        self.open_paths.iter().any(|p| path.starts_with(p))
    }

    pub fn is_auth_required(&self, path: &str) -> bool {
        !self.is_path_open(path)
    }
}

pub async fn auth_middleware(
    State(state): State<AuthMiddlewareConfig>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let path = request.uri().path().to_string();

    if state.is_path_open(&path) {
        return next.run(request).await;
    }

    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok());

    let token = match auth_header.and_then(extract_token_from_header) {
        Some(t) => t,
        None => {
            let response = Json(ApiResponse::<String>::err("Authentication required".to_string()));
            return Response::builder()
                .status(401)
                .body(response.into_response().into_body())
                .unwrap();
        }
    };

    match state.auth_service.verify_token(token) {
        Ok(claims) => {
            let mut request = request;
            request.extensions_mut().insert(claims);
            next.run(request).await
        }
        Err(e) => {
            let response = Json(ApiResponse::<String>::err(e));
            Response::builder()
                .status(401)
                .body(response.into_response().into_body())
                .unwrap()
        }
    }
}

pub fn require_auth<T: std::fmt::Display>(claims: &Claims, permission: &str) -> Result<(), String> {
    if claims.has_permission(permission) || claims.has_permission("*") {
        Ok(())
    } else {
        Err(format!("Permission denied: {}", permission))
    }
}

pub fn require_role(claims: &Claims, roles: &[&str]) -> Result<(), String> {
    if roles.iter().any(|r| *r == claims.rol || claims.has_permission("*")) {
        Ok(())
    } else {
        Err(format!("Role not authorized. Required: {}", roles.join(" or ")))
    }
}

#[derive(axum::extract::FromRef)]
pub struct AuthUser(pub Claims);

pub async fn get_auth_user(
    request: &Request<Body>,
) -> Option<&Claims> {
    request.extensions().get::<Claims>()
}