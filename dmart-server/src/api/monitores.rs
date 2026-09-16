//! Integración de monitores vía HTTP: ingesta de mensajes HL7 v2.
//!
//! Endpoint único usado por gateways (edge) o para pruebas: recibe el mensaje
//! HL7 crudo en el campo `message` y lo ingresa a través del mismo pipeline
//! que MLLP.

use crate::db::Database;
use crate::hl7::ingest::ingest_vitals;
use crate::hl7::parser::{VitalsMessage, parse_oru_message};
use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use dmart_shared::models::ApiResponse;

#[derive(serde::Deserialize)]
pub struct Hl7IngestRequest {
    pub message: String,
}

async fn ingest(db: &Database, msg: VitalsMessage) -> Response {
    match ingest_vitals(db, &msg).await {
        Ok(m) => (
            StatusCode::CREATED,
            Json(ApiResponse::ok(serde_json::json!({
                "measurement_id": m.measurement_id,
                "patient_id": m.patient_id,
                "apache_score": m.apache_score,
                "gcs_score": m.gcs_score,
                "severity": m.severity.label(),
                "mortality_risk": m.mortality_risk,
                "source": msg.source.label(),
                "timestamp": m.timestamp,
            }))),
        )
            .into_response(),
        Err(e) => {
            let msg = crate::security::sanitize_internal_error(&e);
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(ApiResponse::<serde_json::Value>::err(msg)),
            )
                .into_response()
        }
    }
}

/// POST /api/monitores/hl7 — ingerir un mensaje HL7 v2 (ORU^R01).
pub async fn hl7_ingest(
    State(db): State<Database>,
    Json(body): Json<Hl7IngestRequest>,
) -> Response {
    if body.message.len() > 1024 * 1024 {
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(ApiResponse::<serde_json::Value>::err(
                "message too large (max 1 MiB)".to_string(),
            )),
        )
            .into_response();
    }
    match parse_oru_message(&body.message) {
        Ok(msg) => ingest(&db, msg).await,
        Err(e) => {
            let msg = crate::security::sanitize_internal_error(&e);
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(ApiResponse::<serde_json::Value>::err(msg)),
            )
                .into_response()
        }
    }
}

/// Respuesta de salud para el endpoint de monitores (usado por gateways).
pub async fn hl7_health() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(ApiResponse::ok(serde_json::json!({
            "protocol": "HL7 v2 (ORU^R01)",
            "transports": ["HTTP", "MLLP"],
            "status": "ready"
        }))),
    )
}
