use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde_json::json;

use dmart_shared::models::{
    ApiResponse, CreateFeatureFlagRequest, FeatureFlag, FeatureFlagListResponse, UpdateFeatureFlagRequest,
    UserRole,
};
use crate::auth::Claims;
use crate::db::Database;

/// GET /api/v1/admin/flags — Lista feature flags (admin)
pub async fn list_flags(
    State(db): State<Database>,
    claims: Claims,
) -> impl IntoResponse {
    if claims.rol != "Admin" {
        return (StatusCode::FORBIDDEN, Json(ApiResponse::<()>::err("admin role required"))).into_response();
    }

    match crate::db::list_feature_flags(db.as_ref(), None).await {
        Ok(flags) => (StatusCode::OK, Json(ApiResponse::ok(FeatureFlagListResponse { flags }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<()>::err(e.to_string()))).into_response(),
    }
}

/// POST /api/v1/admin/flags — Crea feature flag (admin)
pub async fn create_flag(
    State(db): State<Database>,
    claims: Claims,
    Json(req): Json<CreateFeatureFlagRequest>,
) -> impl IntoResponse {
    if claims.rol != "Admin" {
        return (StatusCode::FORBIDDEN, Json(ApiResponse::<()>::err("admin role required"))).into_response();
    }

    // Validar key format
    if !req.key.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()>::err("key must be lowercase alphanumeric + underscore")),
        )
            .into_response();
    }

    match crate::db::create_feature_flag(db.as_ref(), req).await {
        Ok(flag) => (StatusCode::CREATED, Json(ApiResponse::ok(flag))).into_response(),
        Err(e) => {
            if e.to_string().contains("already exists") || e.to_string().contains("duplicate") {
                (StatusCode::CONFLICT, Json(ApiResponse::<()>::err("flag key already exists"))).into_response()
            } else {
                (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<()>::err(e.to_string()))).into_response()
            }
        }
    }
}

/// GET /api/v1/admin/flags/{key} — Obtiene feature flag (admin)
pub async fn get_flag(
    State(db): State<Database>,
    claims: Claims,
    Path(key): Path<String>,
) -> impl IntoResponse {
    if claims.rol != "Admin" {
        return (StatusCode::FORBIDDEN, Json(ApiResponse::<()>::err("admin role required"))).into_response();
    }

    match crate::db::get_feature_flag(db.as_ref(), &key, None).await {
        Ok(Some(f)) => (StatusCode::OK, Json(ApiResponse::ok(f))).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(ApiResponse::<()>::err("flag not found"))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<()>::err(e.to_string()))).into_response(),
    }
}

/// PUT /api/v1/admin/flags/{key} — Actualiza feature flag (admin)
pub async fn update_flag(
    State(db): State<Database>,
    claims: Claims,
    Path(key): Path<String>,
    Json(req): Json<UpdateFeatureFlagRequest>,
) -> impl IntoResponse {
    if claims.rol != "Admin" {
        return (StatusCode::FORBIDDEN, Json(ApiResponse::<()>::err("admin role required"))).into_response();
    }

    match crate::db::update_feature_flag(db.as_ref(), &key, req).await {
        Ok(flag) => (StatusCode::OK, Json(ApiResponse::ok(flag))).into_response(),
        Err(e) => {
            if e.to_string().contains("not found") {
                (StatusCode::NOT_FOUND, Json(ApiResponse::<()>::err("flag not found"))).into_response()
            } else {
                (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<()>::err(e.to_string()))).into_response()
            }
        }
    }
}

/// DELETE /api/v1/admin/flags/{key} — Elimina feature flag (admin)
pub async fn delete_flag(
    State(db): State<Database>,
    claims: Claims,
    Path(key): Path<String>,
) -> impl IntoResponse {
    if claims.rol != "Admin" {
        return (StatusCode::FORBIDDEN, Json(ApiResponse::<()>::err("admin role required"))).into_response();
    }

    match crate::db::delete_feature_flag(db.as_ref(), &key).await {
        Ok(_) => (StatusCode::NO_CONTENT, Json(ApiResponse::<()>::ok(()))).into_response(),
        Err(e) => {
            if e.to_string().contains("not found") {
                (StatusCode::NOT_FOUND, Json(ApiResponse::<()>::err("flag not found"))).into_response()
            } else {
                (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<()>::err(e.to_string()))).into_response()
            }
        }
    }
}

/// Rutas para feature flags
pub fn routes() -> axum::Router<Database> {
    axum::Router::new()
        .route("/admin/flags", axum::routing::get(list_flags).post(create_flag))
        .route(
            "/admin/flags/{key}",
            axum::routing::get(get_flag)
                .put(update_flag)
                .delete(delete_flag),
        )
}