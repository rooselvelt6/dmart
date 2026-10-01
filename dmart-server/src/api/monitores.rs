//! Integración de monitores vía HTTP: ingesta de mensajes HL7 v2.
//!
//! Endpoint único usado por gateways (edge) o para pruebas: recibe el mensaje
//! HL7 crudo en el campo `message` y lo ingresa a través del mismo pipeline
//! que MLLP.
//!
//! ⚠️ SPEC-025: la ingesta por HTTP está **acotada al tenant del JWT**. La
//! referencia `PID-3` del mensaje la elige el emisor, así que sin este filtro
//! cualquier rol con `measurements:create` (Enfermero/Médico/Admin) podría
//! escribir signos vitales en la historia de un paciente de otro hospital.

use crate::auth::Claims;
use crate::db::Database;
use crate::hl7::ingest::ingest_vitals_for_tenant;
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

async fn ingest(db: &Database, tenant_id: &str, msg: VitalsMessage) -> Response {
    match ingest_vitals_for_tenant(db, &msg, Some(tenant_id)).await {
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
                // Un `patient not found` acá significa "no existe en TU tenant":
                // 404, no 422, para no confirmar la existencia de la referencia
                // en otro hospital (enumeración de pacientes).
                if e.to_string().contains("no encontrado") {
                    StatusCode::NOT_FOUND
                } else {
                    StatusCode::UNPROCESSABLE_ENTITY
                },
                Json(ApiResponse::<serde_json::Value>::err(msg)),
            )
                .into_response()
        }
    }
}

/// POST /api/monitores/hl7 — ingerir un mensaje HL7 v2 (ORU^R01).
///
/// Requiere `measurements:create` (aplicado por el middleware RBAC) y se
/// ejecuta acotado al `tenant_id` de los claims.
pub async fn hl7_ingest(
    State(db): State<Database>,
    claims: Claims,
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
        Ok(msg) => ingest(&db, &claims.tenant_id, msg).await,
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
