//! SPEC-017: handlers HTTP Device Registry.
//! Endpoints: `GET /devices`, `POST /devices`, `GET /devices/{id}`,
//! `POST /devices/{id}/heartbeat`, `GET /devices/status`.

use crate::db::Database;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use dmart_shared::models::ApiResponse;
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub struct DeviceQuery {
    pub estado: Option<String>,
}

/// Normaliza un cuerpo JSON de alta a `RegisterDeviceInput`, rechazando
/// payloads malformados o estados fuera del enum con un 400.
fn parse_register_body(
    body: &Value,
) -> Result<crate::device_registry::RegisterDeviceInput, String> {
    let input: crate::device_registry::RegisterDeviceInput =
        serde_json::from_value(body.clone()).map_err(|e| format!("cuerpo inválido: {}", e))?;

    let required = [
        (input.device_type.is_empty(), "device_type"),
        (input.fabricante.is_empty(), "fabricante"),
        (input.modelo.is_empty(), "modelo"),
        (input.serial.is_empty(), "serial"),
    ];
    if let Some((_, campo)) = required.iter().find(|(vacio, _)| *vacio) {
        return Err(format!("campo requerido faltante: {campo}"));
    }

    if let Some(estado) = &input.estado
        && !estado.is_empty()
        && estado
            .parse::<crate::device_registry::DeviceState>()
            .is_err()
    {
        return Err(format!(
            "estado inválido '{estado}' (esperado: online/offline/mantenimiento)"
        ));
    }

    Ok(input)
}

pub async fn list_devices(State(db): State<Database>, query: Query<DeviceQuery>) -> Response {
    let estado = query.estado.as_deref();
    match crate::device_registry::list(&db, estado).await {
        Ok(devices) => (StatusCode::OK, Json(ApiResponse::ok(devices))).into_response(),
        Err(e) => {
            let msg = crate::security::sanitize_internal_error(&e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<Vec<crate::device_registry::ClinicalDevice>>::err(msg)),
            )
                .into_response()
        }
    }
}

pub async fn register_device(State(db): State<Database>, body: Json<Value>) -> Response {
    let input = match parse_register_body(&body) {
        Ok(input) => input,
        Err(msg) => {
            return (StatusCode::BAD_REQUEST, Json(ApiResponse::<()>::err(msg))).into_response();
        }
    };

    match crate::device_registry::register(&db, input).await {
        Ok(device) => (StatusCode::CREATED, Json(ApiResponse::ok(device))).into_response(),
        Err(e) => {
            let msg = crate::security::sanitize_internal_error(&e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<crate::device_registry::ClinicalDevice>::err(msg)),
            )
                .into_response()
        }
    }
}

pub async fn get_device(State(db): State<Database>, id: Path<String>) -> Response {
    match crate::device_registry::get(&db, &id).await {
        Ok(Some(device)) => (StatusCode::OK, Json(ApiResponse::ok(device))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<crate::device_registry::ClinicalDevice>::err(
                "Dispositivo no encontrado",
            )),
        )
            .into_response(),
        Err(e) => {
            let msg = crate::security::sanitize_internal_error(&e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<crate::device_registry::ClinicalDevice>::err(msg)),
            )
                .into_response()
        }
    }
}

pub async fn heartbeat_device(State(db): State<Database>, id: Path<String>) -> Response {
    match crate::device_registry::heartbeat(&db, &id).await {
        Ok(Some(device)) => (StatusCode::OK, Json(ApiResponse::ok(device))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<crate::device_registry::ClinicalDevice>::err(
                "Dispositivo no encontrado",
            )),
        )
            .into_response(),
        Err(e) => {
            let msg = crate::security::sanitize_internal_error(&e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<crate::device_registry::ClinicalDevice>::err(msg)),
            )
                .into_response()
        }
    }
}

pub async fn device_status_summary(State(db): State<Database>) -> Response {
    match crate::device_registry::status_summary(&db).await {
        Ok(summary) => (StatusCode::OK, Json(ApiResponse::ok(summary))).into_response(),
        Err(e) => {
            let msg = crate::security::sanitize_internal_error(&e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<Value>::err(msg)),
            )
                .into_response()
        }
    }
}
