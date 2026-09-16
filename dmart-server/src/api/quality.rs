//! SPEC-018: handlers HTTP Data Quality.
//! Validación de `VitalsMessage` con persistencia de issues en `quality_events`,
//! reporte de los últimos issues y agregación por severidad/tipo.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use dmart_shared::models::ApiResponse;
use serde_json::Value;

use crate::data_quality::{
    QualityIssue, QualitySummary, ReportQuery, quality_report as report_issues,
    quality_summary as summarize, validate_and_store,
};
use crate::db::Database;

fn err_response<T: serde::Serialize>(status: StatusCode, msg: String) -> Response {
    let resp: ApiResponse<T> = ApiResponse::err(msg);
    (status, Json(resp)).into_response()
}

/// POST /api/data-quality/validate — valida un `VitalsMessage`, persiste los
/// issues detectados y responde con la lista (o `err` si el body no es parseable).
/// No ingesta measurements ni altera patients.
pub async fn validate_message(State(db): State<Database>, body: Json<Value>) -> Response {
    let msg: crate::hl7::parser::VitalsMessage = match serde_json::from_value(body.0) {
        Ok(msg) => msg,
        Err(e) => {
            return err_response::<Vec<QualityIssue>>(
                StatusCode::BAD_REQUEST,
                format!("body no parseable a VitalsMessage: {e}"),
            );
        }
    };
    match validate_and_store(&db, &msg).await {
        Ok(issues) => (StatusCode::OK, Json(ApiResponse::ok(issues))).into_response(),
        Err(e) => {
            let msg = crate::security::sanitize_internal_error(&e);
            err_response::<Vec<QualityIssue>>(StatusCode::INTERNAL_SERVER_ERROR, msg)
        }
    }
}

/// GET /api/data-quality/report — últimos issues (orden `timestamp` desc,
/// por defecto 100).
pub async fn quality_report(State(db): State<Database>) -> Response {
    match report_issues(&db, ReportQuery::default()).await {
        Ok(issues) => (StatusCode::OK, Json(ApiResponse::ok(issues))).into_response(),
        Err(e) => {
            let msg = crate::security::sanitize_internal_error(&e);
            err_response::<Vec<QualityIssue>>(StatusCode::INTERNAL_SERVER_ERROR, msg)
        }
    }
}

/// GET /api/data-quality/summary — agregado por severidad y por code.
pub async fn quality_summary(State(db): State<Database>) -> Response {
    match summarize(&db).await {
        Ok(summary) => (StatusCode::OK, Json(ApiResponse::ok(summary))).into_response(),
        Err(e) => {
            let msg = crate::security::sanitize_internal_error(&e);
            err_response::<QualitySummary>(StatusCode::INTERNAL_SERVER_ERROR, msg)
        }
    }
}
