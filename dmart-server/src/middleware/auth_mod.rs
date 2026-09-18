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
                "/auth/refresh".to_string(),
            ],
        }
    }

    pub fn is_path_open(&self, path: &str) -> bool {
        self.open_paths.iter().any(|p| path.starts_with(p))
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
        Some(t) => Some(t.to_string()),
        None if path.starts_with("/realtime")
            || path.starts_with("/monitores")
            || path.contains("/export/") =>
        {
            request.uri().query().and_then(|q| {
                q.split('&')
                    .find_map(|kv| kv.strip_prefix("token=").map(str::to_string))
            })
        }
        None => None,
    };

    let token = match token {
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

    match state.auth_service.verify_token(&token) {
        Ok(claims) => {
            if crate::auth::is_jti_revoked_in_cache(&claims.jti).await
                || crate::auth::is_user_access_revoked_in_cache(&claims.sub, claims.iat).await
            {
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

            let method = request.method().to_string();
            if let Some(required) = crate::rbac::permission_for(&method, &path)
                && !claims.has_permission(required)
            {
                crate::metrics::rbac_denial(required);
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
    use crate::auth::Claims;

    fn test_claims(scope: &str) -> Claims {
        Claims {
            sub: "u1".into(),
            username: "tester".into(),
            rol: "Viewer".into(),
            permissions: vec!["patients:read".into()],
            exp: chrono::Utc::now().timestamp() + 3600,
            iat: chrono::Utc::now().timestamp(),
            jti: "jti-test-001".into(),
            scope: scope.into(),
            tenant_id: "default".into(),
        }
    }

    #[test]
    fn claims_scope_session_required() {
        let session = test_claims("session");
        assert_eq!(session.scope, "session");

        let mfa = test_claims("mfa");
        assert_eq!(mfa.scope, "mfa");
        assert_ne!(mfa.scope, "session");
    }

    #[test]
    fn claims_permission_check() {
        let viewer = test_claims("session");
        assert!(viewer.has_permission("patients:read"));
        assert!(!viewer.has_permission("users:create"));

        let admin = Claims {
            permissions: vec!["*".into()],
            ..test_claims("session")
        };
        assert!(admin.has_permission("users:create"));
        assert!(admin.has_permission("anything"));
    }
}
