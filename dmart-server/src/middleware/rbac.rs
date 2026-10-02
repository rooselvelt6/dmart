//! Middleware de autorización por tenant (SPEC-025 / F2.1-rbac).
//!
//! Valida que el JWT contiene `tenant_id` y que coincide con el recurso
//  accedido (cuando aplica). Rechaza con 403 si no hay tenant o no coincide.

use crate::auth::Claims;
use crate::db::Database;
use crate::rbac::ResourceOwner;
use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use tracing::warn;

/// Estado compartido para el middleware (DB pool).
#[derive(Clone)]
pub struct RbacState {
    pub db: Database,
}

/// Extrae el tenant_id de los Claims ya validados en request extensions.
fn extract_tenant_id(request: &Request) -> Option<String> {
    let claims = request.extensions().get::<Claims>()?;
    Some(claims.tenant_id.clone())
}

/// Middleware que exige `tenant_id` en el JWT (ya validado por auth_middleware)
/// y lo inyecta en extensions para fácil acceso.
///
/// Uso:
/// ```rust
/// let app = Router::new()
///     .route("/patients", get(list_patients))
///     .layer(axum::middleware::from_fn_with_state(rbac_state, require_tenant));
/// ```
/// Nota: Este middleware debe ir DESPUÉS de `auth_middleware` que ya valida el token
/// e inserta `Claims` en extensions.
pub async fn require_tenant(
    State(_state): State<RbacState>,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let tenant_id = extract_tenant_id(&request).ok_or_else(|| {
        warn!("require_tenant: Claims sin tenant_id en extensions");
        StatusCode::FORBIDDEN
    })?;

    // Inyecta tenant_id en extensions para que handlers lo usen
    request.extensions_mut().insert(tenant_id);

    Ok(next.run(request).await)
}

/// Middleware que valida ownership del recurso por tenant_id.
///
/// Para rutas como `/patients/{id}`, verifica que el patient pertenece al tenant del JWT.
pub async fn require_tenant_ownership<T>(
    State(state): State<RbacState>,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode>
where
    T: crate::rbac::ResourceOwner,
{
    let tenant_id = extract_tenant_id(&request).ok_or_else(|| {
        warn!("require_tenant_ownership: Claims sin tenant_id en extensions");
        StatusCode::FORBIDDEN
    })?;

    // Extrae resource_id de la ruta (último segmento)
    let path = request.uri().path();
    let resource_id = path.split('/').next_back().unwrap_or("");

    // Verifica ownership via trait ResourceOwner
    let db = state.db.clone();
    let owns = T::check_ownership(&db, &tenant_id, resource_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !owns {
        warn!(
            "require_tenant_ownership: tenant {} no posee resource {}",
            tenant_id, resource_id
        );
        return Err(StatusCode::FORBIDDEN);
    }

    request.extensions_mut().insert(tenant_id);
    Ok(next.run(request).await)
}

/// Helper para obtener tenant_id del request (inyectado por require_tenant).
pub fn get_tenant_id(request: &Request) -> Option<String> {
    request.extensions().get::<String>().cloned()
}

/// Helper para obtener claims completos del request (ya validados por auth_middleware).
pub fn get_claims(request: &Request) -> Option<Claims> {
    request.extensions().get::<Claims>().cloned()
}
