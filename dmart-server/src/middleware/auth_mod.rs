use axum::{
    Json,
    body::Body,
    extract::{Request, State},
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::auth::{AuthService, extract_token_from_header};
use dmart_shared::models::ApiResponse;

#[derive(Clone)]
pub struct AuthMiddlewareConfig {
    pub auth_service: AuthService,
    pub open_paths: Vec<String>,
}

impl AuthMiddlewareConfig {
    pub fn new(auth_service: AuthService) -> Self {
        Self {
            auth_service,
            open_paths: vec![
                "/health".to_string(),
                "/auth/login".to_string(),
                "/auth/mfa/verify".to_string(),
            ],
        }
    }

    pub fn is_path_open(&self, path: &str) -> bool {
        self.open_paths.iter().any(|p| path.starts_with(p))
    }
}

/// Returns true for paths that require the Admin role (permission `*`).
///
/// Staff management, user creation and the sandbox/generator are restricted,
/// while the patient-care helpers (`check-camas`, `disponibles`, equipo de
/// cama) remain available to any authenticated clinician.
pub fn is_admin_only_path(path: &str) -> bool {
    const CARE_PATHS: [&str; 4] = [
        "/admin/check-camas",
        "/admin/camas/disponibles",
        "/admin/equipos/disponibles",
        "/admin/equipos/cama/",
    ];
    let is_care_path = CARE_PATHS.iter().any(|p| path.starts_with(p));
    let is_admin_area = path.starts_with("/admin")
        || path.starts_with("/auth/register")
        || path.starts_with("/auth/users")
        || path.starts_with("/sandbox");
    is_admin_area && !is_care_path
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
            let response = Json(ApiResponse::<String>::err(
                "Authentication required".to_string(),
            ));
            return Response::builder()
                .status(401)
                .body(response.into_response().into_body())
                .unwrap();
        }
    };

    match state.auth_service.verify_token(token) {
        Ok(claims) => {
            if crate::auth::is_token_revoked_in_cache(token).await {
                let response = Json(ApiResponse::<String>::err("Token revocado".to_string()));
                return Response::builder()
                    .status(401)
                    .body(response.into_response().into_body())
                    .unwrap();
            }

            if claims.scope != "session" {
                let response = Json(ApiResponse::<String>::err(
                    "Complete la verificación MFA".to_string(),
                ));
                return Response::builder()
                    .status(401)
                    .body(response.into_response().into_body())
                    .unwrap();
            }

            if is_admin_only_path(&path) && !claims.has_permission("*") {
                let response = Json(ApiResponse::<String>::err(
                    "Forbidden: se requiere rol Admin".to_string(),
                ));
                return Response::builder()
                    .status(403)
                    .body(response.into_response().into_body())
                    .unwrap();
            }

            let method = request.method().to_string();
            if let Some(required) = crate::rbac::permission_for(&method, &path)
                && !claims.has_permission(required)
            {
                let response = Json(ApiResponse::<String>::err(format!(
                    "Forbidden: se requiere permiso {}",
                    required
                )));
                return Response::builder()
                    .status(403)
                    .body(response.into_response().into_body())
                    .unwrap();
            }

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

#[cfg(test)]
mod tests {
    use super::is_admin_only_path;

    #[test]
    fn admin_only_detection() {
        assert!(is_admin_only_path("/admin/staff"));
        assert!(is_admin_only_path("/admin/stats"));
        assert!(is_admin_only_path("/admin/equipos"));
        assert!(is_admin_only_path("/admin/camas"));
        assert!(is_admin_only_path("/admin/equipos/asignar"));
        assert!(is_admin_only_path("/auth/register"));
        assert!(is_admin_only_path("/auth/users"));
        assert!(is_admin_only_path("/sandbox/generate"));

        assert!(!is_admin_only_path("/admin/check-camas"));
        assert!(!is_admin_only_path("/admin/camas/disponibles"));
        assert!(!is_admin_only_path("/admin/equipos/disponibles"));
        assert!(!is_admin_only_path("/admin/equipos/cama/abc"));
        assert!(!is_admin_only_path("/patients"));
        assert!(!is_admin_only_path("/auth/login"));
        assert!(!is_admin_only_path("/health"));
    }
}
