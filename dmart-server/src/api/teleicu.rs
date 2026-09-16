//! SPEC-020: handlers HTTP Tele-ICU.
//! Rutas (registradas en `api/mod.rs`):
//!   POST  /teleicu/sessions            → start_session
//!   GET   /teleicu/sessions            → list_sessions
//!   POST  /teleicu/sessions/{id}/end   → end_session
//!   GET   /teleicu/live/{id}           → live_view (id = patient_id)

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use dmart_shared::models::{ApiResponse, Measurement, Patient};
use serde::Deserialize;
use serde::Serialize;
use serde_json::{Value, json};

use crate::db::Database;
use crate::patient_timeline::{TimelineQuery, TimelineResponse};
use crate::teleicu::{self, SessionStatus, TeleIcuSession};

#[derive(Deserialize)]
pub struct StartSessionRequest {
    pub patient_id: String,
    pub specialist_id: String,
    pub channel: String,
}

#[derive(Serialize)]
pub struct EndedSessionResponse {
    pub session: TeleIcuSession,
    pub duration_minutes: Option<i64>,
}

#[derive(Serialize)]
pub struct SessionsList {
    pub active: Vec<TeleIcuSession>,
    pub recent: Vec<TeleIcuSession>,
}

#[derive(Serialize)]
pub struct LiveViewResponse {
    pub patient: Patient,
    pub nombre_completo: String,
    pub cama_id: Option<String>,
    pub cama_numero: Option<u8>,
    pub session: TeleIcuSession,
    pub last_measurement: Option<Measurement>,
    pub timeline: TimelineResponse,
}

// POST /api/teleicu/sessions
pub async fn start_session(State(db): State<Database>, body: Json<Value>) -> Response {
    let req: StartSessionRequest = match serde_json::from_value(body.0.clone()) {
        Ok(r) => r,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::err(
                    "Body inválido: se requieren patient_id, specialist_id y channel",
                )),
            )
                .into_response();
        }
    };

    if req.patient_id.trim().is_empty() || req.channel.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()>::err(
                "patient_id, specialist_id y channel son obligatorios",
            )),
        )
            .into_response();
    }

    match crate::db::get_patient(&db, &req.patient_id).await {
        Ok(Some(_)) => {}
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ApiResponse::<()>::err("Paciente no encontrado")),
            )
                .into_response();
        }
        Err(e) => {
            let msg = crate::security::sanitize_internal_error(&e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<()>::err(msg)),
            )
                .into_response();
        }
    }

    match teleicu::find_active_session_for_patient(&db, &req.patient_id).await {
        Ok(Some(_)) => (
            StatusCode::CONFLICT,
            Json(ApiResponse::<()>::err(
                "El paciente ya tiene una sesión activa de tele-ICU",
            )),
        )
            .into_response(),
        Ok(None) => {
            let session = TeleIcuSession::new(req.patient_id, req.specialist_id, req.channel);
            match teleicu::create_session(&db, &session).await {
                Ok(created) => {
                    crate::realtime::publish(
                        "teleicu",
                        json!({
                            "event": "session_started",
                            "session_id": created.session_id,
                            "patient_id": created.patient_id,
                        }),
                    );
                    (StatusCode::CREATED, Json(ApiResponse::ok(created))).into_response()
                }
                Err(e) => {
                    let msg = crate::security::sanitize_internal_error(&e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ApiResponse::<TeleIcuSession>::err(msg)),
                    )
                        .into_response()
                }
            }
        }
        Err(e) => {
            let msg = crate::security::sanitize_internal_error(&e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<()>::err(msg)),
            )
                .into_response()
        }
    }
}

// GET /api/teleicu/sessions
pub async fn list_sessions(State(db): State<Database>) -> Response {
    match teleicu::list_sessions(&db).await {
        Ok(sessions) => {
            let (active, recent): (Vec<_>, Vec<_>) = sessions
                .into_iter()
                .partition(|s| s.status == SessionStatus::Active);
            (
                StatusCode::OK,
                Json(ApiResponse::ok(SessionsList { active, recent })),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<SessionsList>::err(e.to_string())),
        )
            .into_response(),
    }
}

// POST /api/teleicu/sessions/{id}/end
pub async fn end_session(State(db): State<Database>, Path(id): Path<String>) -> Response {
    let session = match teleicu::get_session(&db, &id).await {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<EndedSessionResponse>::err(e.to_string())),
            )
                .into_response();
        }
    };

    let session = match session {
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(ApiResponse::<EndedSessionResponse>::err(
                    "Sesión no encontrada",
                )),
            )
                .into_response();
        }
        Some(s) => s,
    };

    if session.status == SessionStatus::Ended {
        return (
            StatusCode::CONFLICT,
            Json(ApiResponse::<EndedSessionResponse>::err(
                "La sesión ya fue finalizada",
            )),
        )
            .into_response();
    }

    match teleicu::close_session(&db, &id).await {
        Ok(Some(ended)) => {
            let duration_minutes = ended
                .ended_at
                .as_deref()
                .and_then(|e| teleicu::duration_minutes(&ended.started_at, e));
            crate::realtime::publish(
                "teleicu",
                json!({
                    "event": "session_ended",
                    "session_id": ended.session_id,
                    "patient_id": ended.patient_id,
                }),
            );
            (
                StatusCode::OK,
                Json(ApiResponse::ok(EndedSessionResponse {
                    session: ended,
                    duration_minutes,
                })),
            )
                .into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<EndedSessionResponse>::err(
                "Sesión no encontrada",
            )),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<EndedSessionResponse>::err(e.to_string())),
        )
            .into_response(),
    }
}

// GET /api/teleicu/live/{id}  (id = patient_id)
pub async fn live_view(State(db): State<Database>, Path(id): Path<String>) -> Response {
    let patient = match crate::db::get_patient(&db, &id).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ApiResponse::<LiveViewResponse>::err(
                    "Paciente no encontrado",
                )),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<LiveViewResponse>::err(e.to_string())),
            )
                .into_response();
        }
    };

    let session = match teleicu::find_active_session_for_patient(&db, &id).await {
        Ok(Some(s)) => s,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ApiResponse::<LiveViewResponse>::err(
                    "No hay una sesión activa de tele-ICU para este paciente",
                )),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<LiveViewResponse>::err(e.to_string())),
            )
                .into_response();
        }
    };

    let last_measurement = match crate::db::get_last_measurement(&db, &id).await {
        Ok(m) => m,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<LiveViewResponse>::err(e.to_string())),
            )
                .into_response();
        }
    };

    let timeline = match crate::patient_timeline::query_timeline(
        &db,
        &id,
        TimelineQuery {
            since: None,
            until: None,
            event_type: None,
            severity: None,
            cursor: None,
            limit: Some(10),
        },
    )
    .await
    {
        Ok(t) => t,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<LiveViewResponse>::err(e.to_string())),
            )
                .into_response();
        }
    };

    let nombre_completo = patient.nombre_completo();
    let cama_id = patient.cama_id.clone();
    let cama_numero = patient.cama_numero;
    (
        StatusCode::OK,
        Json(ApiResponse::ok(LiveViewResponse {
            patient,
            nombre_completo,
            cama_id,
            cama_numero,
            session,
            last_measurement,
            timeline,
        })),
    )
        .into_response()
}
