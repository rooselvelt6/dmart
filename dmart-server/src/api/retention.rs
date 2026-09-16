//! SPEC-030: API admin de retención / downsampling de mediciones.
//!
//! Expone subrutas SIN prefijo (el wiring final las nida bajo `/api`). Rutas:
//! - `POST /admin/retention/run`    → ejecuta `run_downsample` (job bajo demanda).
//! - `GET  /admin/retention/config` → política activa (`RetentionConfig`).
//! - `GET  /admin/retention/status` → counts por tabla + última ejecución.

use crate::db::Database;
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use dmart_shared::models::ApiResponse;

pub fn routes() -> Router<Database> {
    Router::new()
        .route("/admin/retention/run", post(run_job))
        .route("/admin/retention/config", get(config))
        .route("/admin/retention/status", get(status))
}

/// `POST /admin/retention/run` — dispara el downsampling y devuelve el reporte.
async fn run_job(State(db): State<Database>) -> impl IntoResponse {
    match crate::retention::run_downsample(&db).await {
        Ok(report) => (StatusCode::OK, Json(ApiResponse::ok(report))).into_response(),
        Err(e) => {
            let msg = crate::security::sanitize_internal_error(&e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<crate::retention::RetentionRunReport>::err(msg)),
            )
                .into_response()
        }
    }
}

/// `GET /admin/retention/config` — política activa (env overrides incluidos).
async fn config(State(_db): State<Database>) -> impl IntoResponse {
    Json(ApiResponse::ok(crate::retention::config_from_env())).into_response()
}

/// `GET /admin/retention/status` — counts actuales + última ejecución.
async fn status(State(db): State<Database>) -> impl IntoResponse {
    match crate::retention::retention_status(&db).await {
        Ok(s) => (StatusCode::OK, Json(ApiResponse::ok(s))).into_response(),
        Err(e) => {
            let msg = crate::security::sanitize_internal_error(&e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<crate::retention::RetentionStatus>::err(msg)),
            )
                .into_response()
        }
    }
}