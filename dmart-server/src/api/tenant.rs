//! SPEC-025: Tenant Management API (solo super_admin / rol Admin).
//! Endpoints: GET/POST /admin/tenants y POST /admin/tenants/{slug}/impersonate.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;

use crate::auth::Claims;
use crate::db::Database;
use crate::tenant;

use dmart_shared::models::ApiResponse;

#[derive(Debug, Deserialize)]
pub struct CreateTenantRequest {
    pub name: String,
    pub slug: String,
}

#[derive(serde::Serialize)]
pub struct TenantListItem {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub active: bool,
    pub patient_count: u64,
}

async fn patient_count(db: &Database, slug: &str) -> u64 {
    crate::db::count_patients_for_tenant(db, slug).await.unwrap_or(0)
}

/// GET /api/admin/tenants — lista todos los tenants (super_admin).
pub async fn list_tenants_api(
    State(db): State<Database>,
) -> impl IntoResponse {
    match tenant::list_tenants(&db).await {
        Ok(list) => {
            let items: Vec<TenantListItem> = {
                let mut v = Vec::new();
                for t in list {
                    let slug = t.slug;
                    v.push(TenantListItem {
                        id: slug.clone(),
                        name: t.name,
                        slug: slug.clone(),
                        active: t.active,
                        patient_count: patient_count(&db, &slug).await,
                    });
                }
                v
            };
            (StatusCode::OK, Json(ApiResponse::ok(items))).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<Vec<TenantListItem>>::err(e.to_string())),
        )
            .into_response(),
    }
}

/// POST /api/admin/tenants — crea un tenant (super_admin).
pub async fn create_tenant_api(
    State(db): State<Database>,
    Json(req): Json<CreateTenantRequest>,
) -> impl IntoResponse {
    let slug = req.slug.trim().to_lowercase();
    if slug.len() < 3 || slug.len() > 50 || !slug.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<tenant::Tenant>::err(
                "slug debe ser [a-z0-9-] (3-50 chars)".to_string(),
            )),
        )
            .into_response();
    }
    if req.name.trim().len() < 3 || req.name.len() > 100 {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<tenant::Tenant>::err(
                "name debe tener 3-100 chars".to_string(),
            )),
        )
            .into_response();
    }

    match tenant::create_tenant(&db, &slug, req.name.trim()).await {
        Ok(t) => (StatusCode::CREATED, Json(ApiResponse::ok(t))).into_response(),
        Err(e) => (
            StatusCode::CONFLICT,
            Json(ApiResponse::<tenant::Tenant>::err(format!("No se pudo crear el tenant: {e}"))),
        )
            .into_response(),
    }
}

/// POST /api/admin/tenants/{slug}/impersonate — emite claims con el tenant
/// objetivo para soporte (super_admin). Audit log registrado.
pub async fn impersonate_tenant_api(
    State(db): State<Database>,
    Path(slug): Path<String>,
    claims: Claims,
) -> impl IntoResponse {
    if slug == "default" || crate::tenant::get_tenant(&db, &slug).await.map(|t| t.is_none()).unwrap_or(true) {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<serde_json::Value>::err("Tenant no encontrado".to_string())),
        )
            .into_response();
    }

    if let Some(audit) = crate::audit::audit() {
        let _ = audit
            .log_system_event(
                &claims.sub,
                &claims.username,
                "tenant.impersonate",
                &format!("impersonation target: {}", slug),
            )
            .await;
    }

    let expires_at = chrono::Utc::now().timestamp() + 60 * 30; // 30 min
    (StatusCode::OK, Json(ApiResponse::ok(serde_json::json!({
        "tenant_id": slug,
        "expires_at": expires_at,
    })))).into_response()
}

/// GET /api/admin/tenants/audit — auditoría de aislamiento de datos.
/// Escanea pacientes, mediciones, usuarios y embeddings en busca de registros
/// sin `tenant_id` o con `tenant_id` no registrado (huérfanos).
pub async fn audit_tenancy_api(
    State(db): State<Database>,
) -> impl IntoResponse {
    match crate::db::audit_tenancy(&db).await {
        Ok(report) => (
            if report.healthy {
                StatusCode::OK
            } else {
                StatusCode::CONFLICT
            },
            Json(ApiResponse::ok(report)),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<crate::db::TenancyAuditReport>::err(e.to_string())),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_validation() {
        assert!(slug_is_valid("hosp-a"));
        assert!(slug_is_valid("hosp2026"));
        assert!(!slug_is_valid("Hosp A"));
        assert!(!slug_is_valid("ab"));
        assert!(!slug_is_valid("hosp_a"));
    }

    fn slug_is_valid(slug: &str) -> bool {
        let len_ok = (3..=50).contains(&slug.len());
        let chars_ok = slug.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        len_ok && chars_ok
    }
}