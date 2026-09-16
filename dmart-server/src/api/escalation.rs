//! SPEC-019: handlers HTTP Alert Escalation.
//! Rutas: `GET/POST /escalation/policies`, `GET /escalation/active`,
//! `POST /escalation/{id}/ack`, `POST /escalation/{id}/escalate`.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use dmart_shared::models::ApiResponse;
use serde::Deserialize;
use serde_json::Value;

use crate::db::Database;
use crate::escalation as escalation_ops;
use crate::escalation::{Escalation, EscalationPolicy, EscalationStatus, Severity};

/// Cuerpo de `set_policy`: solo `severity` es obligatorio; el resto tiene
/// defaults razonables por severidad (`default_policy`).
#[derive(Debug, Deserialize)]
pub struct SetPolicyInput {
    pub severity: Severity,
    #[serde(default)]
    pub max_response_minutes: Option<u32>,
    #[serde(default)]
    pub timeout_minutes: Option<u32>,
    #[serde(default)]
    pub target_role: Option<String>,
    #[serde(default)]
    pub enabled: Option<bool>,
}

pub async fn list_policies(State(db): State<Database>) -> Response {
    match escalation_ops::list_policies(&db).await {
        Ok(policies) => (StatusCode::OK, Json(ApiResponse::ok(policies))).into_response(),
        Err(e) => {
            let msg = crate::security::sanitize_internal_error(&e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<Vec<EscalationPolicy>>::err(msg)),
            )
                .into_response()
        }
    }
}

pub async fn set_policy(State(db): State<Database>, body: Json<Value>) -> Response {
    let input = match serde_json::from_value::<SetPolicyInput>(body.0) {
        Ok(input) => input,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<EscalationPolicy>::err(format!(
                    "cuerpo de política inválido: {e}"
                ))),
            )
                .into_response();
        }
    };
    // Upsert parcial: los campos ausentes heredan la política vigente y, en su
    // defecto, los defaults razonables de la severidad.
    let base = match escalation_ops::get_policy(&db, input.severity).await {
        Ok(Some(policy)) => policy,
        Ok(None) => escalation_ops::default_policy(input.severity),
        Err(_) => escalation_ops::default_policy(input.severity),
    };
    let policy = EscalationPolicy {
        severity: input.severity,
        max_response_minutes: input
            .max_response_minutes
            .unwrap_or(base.max_response_minutes),
        timeout_minutes: input.timeout_minutes.unwrap_or(base.timeout_minutes),
        target_role: input.target_role.unwrap_or(base.target_role),
        enabled: input.enabled.unwrap_or(base.enabled),
        ..base
    };
    match escalation_ops::upsert_policy(&db, policy).await {
        Ok(saved) => (StatusCode::OK, Json(ApiResponse::ok(saved))).into_response(),
        Err(e) => {
            let msg = crate::security::sanitize_internal_error(&e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<EscalationPolicy>::err(msg)),
            )
                .into_response()
    }
}
}

pub async fn active_escalations(State(db): State<Database>) -> Response {
    match escalation_ops::active_escalations(&db).await {
        Ok(escalations) => (StatusCode::OK, Json(ApiResponse::ok(escalations))).into_response(),
        Err(e) => {
            let msg = crate::security::sanitize_internal_error(&e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<Vec<Escalation>>::err(msg)),
            )
                .into_response()
        }
    }
}

pub async fn acknowledge_alert(State(db): State<Database>, Path(id): Path<String>) -> Response {
    match escalation_ops::acknowledge_escalation(&db, &id).await {
        Ok(Some(esc)) if esc.status == EscalationStatus::Resolved => (
            StatusCode::CONFLICT,
            Json(ApiResponse::<Escalation>::err("escalación ya resuelta")),
        )
            .into_response(),
        Ok(Some(esc)) => (StatusCode::OK, Json(ApiResponse::ok(esc))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<Escalation>::err("escalación no encontrada")),
        )
            .into_response(),
        Err(e) => {
            let msg = crate::security::sanitize_internal_error(&e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<Escalation>::err(msg)),
            )
                .into_response()
        }
    }
}

pub async fn escalate_alert(State(db): State<Database>, Path(id): Path<String>) -> Response {
    match escalation_ops::escalate_escalation(&db, &id, None).await {
        Ok(Some(esc)) if esc.status == EscalationStatus::Resolved => (
            StatusCode::CONFLICT,
            Json(ApiResponse::<Escalation>::err("escalación ya resuelta")),
        )
            .into_response(),
        Ok(Some(esc)) => (StatusCode::OK, Json(ApiResponse::ok(esc))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<Escalation>::err("escalación no encontrada")),
        )
            .into_response(),
        Err(e) => {
            let msg = crate::security::sanitize_internal_error(&e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<Escalation>::err(msg)),
            )
                .into_response()
        }
    }
}
