use axum::{
    body::Body,
    extract::{Request, State},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};

use crate::auth::{extract_token_from_header, AuthService, Claims};
use dmart_shared::models::ApiResponse;

#[derive(Clone)]
pub struct AuthMiddlewareConfig {
    pub auth_service: AuthService,
    #[allow(dead_code)]
    pub required_paths: Vec<String>,
    pub open_paths: Vec<String>,
}

impl AuthMiddlewareConfig {
    pub fn new(auth_service: AuthService) -> Self {
        Self {
            auth_service,
            required_paths: vec![
                "/patients".to_string(),
                "/measurements".to_string(),
                "/stats".to_string(),
            ],
            open_paths: vec![
                "/health".to_string(),
                "/auth/login".to_string(),
                "/auth/register".to_string(),
            ],
        }
    }

    pub fn is_path_open(&self, path: &str) -> bool {
        self.open_paths.iter().any(|p| path.starts_with(p))
    }

    #[allow(dead_code)]
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

#[allow(dead_code)]
pub fn require_auth<T: std::fmt::Display>(claims: &Claims, permission: &str) -> Result<(), String> {
    if claims.has_permission(permission) || claims.has_permission("*") {
        Ok(())
    } else {
        Err(format!("Permission denied: {}", permission))
    }
}

#[allow(dead_code)]
pub fn require_role(claims: &Claims, roles: &[&str]) -> Result<(), String> {
    if roles.iter().any(|r| *r == claims.rol || claims.has_permission("*")) {
        Ok(())
    } else {
        Err(format!("Role not authorized. Required: {}", roles.join(" or ")))
    }
}

#[allow(dead_code)]
pub struct AuthUser(pub Claims);