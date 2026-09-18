//! SPEC-052 / tarea 3.9 — endpoints HTTP de Web Push (VAPID).
//! Rutas: `GET /push/vapid`, `POST /push/subscribe`,
//! `DELETE /push/unsubscribe`, `POST /push/test`.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use dmart_shared::models::ApiResponse;
use serde::Deserialize;
use serde_json::json;

use crate::auth::Claims;
use crate::db::Database;
use crate::push::{NewSubscription, push};

/// GET /push/vapid — clave pública VAPID para que el navegador suscriba.
pub async fn vapid_public_key() -> Response {
    let Some(service) = push() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiResponse::<()>::err("push deshabilitado")),
        )
            .into_response();
    };
    (
        StatusCode::OK,
        Json(ApiResponse::ok(json!({ "public_key": service.public_key() }))),
    )
        .into_response()
}

/// POST /push/subscribe — registra (o actualiza) la suscripción del navegador.
pub async fn subscribe(
    State(db): State<Database>,
    claims: Claims,
    body: Json<serde_json::Value>,
) -> Response {
    let Some(service) = push() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiResponse::<()>::err("push deshabilitado")),
        )
            .into_response();
    };

    let input: NewSubscription = match serde_json::from_value(body.0) {
        Ok(input) => input,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::err(format!("cuerpo inválido: {e}"))),
            )
                .into_response();
        }
    };

    let endpoint = input.endpoint.trim();
    let valid_url = endpoint.starts_with("https://") || endpoint.starts_with("http://");
    if !valid_url {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(ApiResponse::<()>::err("endpoint debe ser http(s)://")),
        )
            .into_response();
    }

    let sub = NewSubscription {
        endpoint: endpoint.to_string(),
        p256dh: input.p256dh,
        auth: input.auth,
        user_agent: input.user_agent,
    };
    match service.subscribe(&db, &claims.sub, sub).await {
        Ok(true) => {
            audit_push(&claims, "push_subscribe", "alta de suscripción").await;
            (
                StatusCode::CREATED,
                Json(ApiResponse::ok(json!({ "id": claims.sub }))),
            )
                .into_response()
        }
        Ok(false) => {
            audit_push(&claims, "push_subscribe", "suscripción reemplazada").await;
            (
                StatusCode::OK,
                Json(ApiResponse::ok(json!({ "id": claims.sub }))),
            )
                .into_response()
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

/// DELETE /push/unsubscribe — da de baja una suscripción por endpoint.
pub async fn unsubscribe(
    State(db): State<Database>,
    claims: Claims,
    body: Json<serde_json::Value>,
) -> Response {
    let Some(service) = push() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiResponse::<()>::err("push deshabilitado")),
        )
            .into_response();
    };

    #[derive(Deserialize)]
    struct UnsubscribeInput {
        endpoint: String,
    }
    let input: UnsubscribeInput = match serde_json::from_value(body.0) {
        Ok(input) => input,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::err(format!("cuerpo inválido: {e}"))),
            )
                .into_response();
        }
    };

    match service.unsubscribe(&db, &claims.sub, input.endpoint.trim()).await {
        Ok(removed) => {
            audit_push(&claims, "push_unsubscribe", &format!("baja de {removed} suscripción(es)"))
                .await;
            (
                StatusCode::OK,
                Json(ApiResponse::ok(json!({ "removed": removed }))),
            )
                .into_response()
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

/// POST /push/test — envía una notificación de prueba a toda la base suscripta.
pub async fn send_test(
    State(db): State<Database>,
    claims: Claims,
    body: Json<serde_json::Value>,
) -> Response {
    let Some(service) = push() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiResponse::<()>::err("push deshabilitado")),
        )
            .into_response();
    };

    #[derive(Deserialize)]
    struct TestInput {
        title: Option<String>,
        body: Option<String>,
    }
    let input: TestInput = match serde_json::from_value(body.0) {
        Ok(input) => input,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::err(format!("cuerpo inválido: {e}"))),
            )
                .into_response();
        }
    };
    let title = input.title.unwrap_or_else(|| "dMart UCI".to_string());
    let body = input.body.unwrap_or_else(|| "Prueba de notificación".to_string());

    let (sent, failed) = service.send_to_all(&db, &title, &body).await;
    audit_push(
        &claims,
        "push_test",
        &format!("prueba enviada ({sent} ok, {failed} fallidas)"),
    )
    .await;
    (
        StatusCode::OK,
        Json(ApiResponse::ok(json!({ "sent": sent, "failed": failed }))),
    )
        .into_response()
}

/// Registra un evento de auditoría best-effort (SPEC-052 STRIDE: Repudiation).
async fn audit_push(claims: &Claims, resource: &str, details: &str) {
    if let Some(audit) = crate::audit::audit() {
        let _ = audit
            .log_system_event(&claims.sub, &claims.username, resource, details)
            .await;
    }
}