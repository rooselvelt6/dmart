//! Realtime: streaming de eventos (SSE) para datos en vivo.
//!
//! Usa un canal `broadcast`. Los handlers publican eventos (p.ej. nueva
//! medición) y `/api/realtime/stream` los emite a los clientes conectados
//! vía Server-Sent Events (SSE).

pub mod keepalive {
    //! Constantes reutilizables para el keep-alive de SSE.

    /// Intervalo del keep-alive (segundos). Menor que el timeout del proxy.
    pub const INTERVAL_SECS: u64 = 15;
}

use axum::{
    http::StatusCode,
    response::IntoResponse,
    response::sse::{Event, KeepAlive, Sse},
};
use dmart_shared::models::ApiResponse;
use futures_util::stream::unfold;
use serde_json::json;
use std::convert::Infallible;
use std::sync::OnceLock;
use tokio::sync::broadcast;

type EventSender = broadcast::Sender<String>;

/// Un hub de eventos: permite publicar y suscribirse sobre un canal `broadcast`.
pub struct RealtimeHub {
    tx: EventSender,
}

impl RealtimeHub {
    /// Crea un hub con un canal de capacidad 256.
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(256);
        Self { tx }
    }

    /// Publica un evento JSON a todos los suscriptores de este hub.
    pub fn publish(&self, event_type: &str, payload: serde_json::Value) {
        let message = json!({ "type": event_type, "data": payload }).to_string();
        // Ignorar errores ("no receivers") de forma silenciosa.
        let _ = self.tx.send(message);
    }

    /// Suscribe un receptor al canal de este hub.
    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.tx.subscribe()
    }

    /// Construye un stream SSE que emite los eventos del hub.
    pub fn sse_stream(&self) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
        let rx = self.subscribe();

        let stream = unfold(rx, |mut rx| async move {
            loop {
                match rx.recv().await {
                    Ok(message) => {
                        return Some((Ok::<_, Infallible>(Event::default().data(message)), rx));
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => return None,
                }
            }
        });

        crate::metrics::sse_connect();
        Sse::new(CountedStream { inner: stream }).keep_alive(
            KeepAlive::new().interval(std::time::Duration::from_secs(keepalive::INTERVAL_SECS)),
        )
    }
}

/// Wrapper de stream que decrementa `sse_connections_active` cuando la conexión
/// se cierra (el `Drop` se ejecuta al terminar/abandonar el stream).
struct CountedStream<S> {
    inner: S,
}

impl<S> Drop for CountedStream<S> {
    fn drop(&mut self) {
        crate::metrics::sse_disconnect();
    }
}

impl<S: futures_util::Stream> futures_util::Stream for CountedStream<S> {
    type Item = S::Item;
    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<S::Item>> {
        let inner = unsafe { self.as_mut().map_unchecked_mut(|s| &mut s.inner) };
        inner.poll_next(cx)
    }
}

impl Default for RealtimeHub {
    fn default() -> Self {
        Self::new()
    }
}

use crate::metrics::EwsSeverity;

/// Evento de score EWS para streaming en tiempo real (SPEC-014).
#[derive(Debug, Clone, serde::Serialize)]
pub struct ScoreEvent {
    pub patient_id: String,
    pub apache_score: f64,
    pub news2_score: f64,
    pub sofa_score: f64,
    pub timestamp: String,
    pub severity: EwsSeverity,
}

/// Publica un ScoreEvent en el hub global (SPEC-014).
pub fn publish_event(event: ScoreEvent) {
    global_hub().publish(
        "score",
        serde_json::to_value(event).expect("ScoreEvent serialize"),
    );
}

static REALTIME_TX: OnceLock<RealtimeHub> = OnceLock::new();

fn global_hub() -> &'static RealtimeHub {
    REALTIME_TX.get_or_init(RealtimeHub::new)
}

/// Publica un evento JSON a todos los clientes conectados (hub global).
pub fn publish(event_type: &str, payload: serde_json::Value) {
    global_hub().publish(event_type, payload);
}

/// GET /api/realtime/stream — Streaming SSE de eventos del hub global.
pub async fn realtime_stream() -> impl IntoResponse {
    global_hub().sse_stream().into_response()
}

/// GET /api/realtime/ping — Público, para probar el canal sin autenticación.
pub async fn realtime_ping() -> impl IntoResponse {
    publish(
        "ping",
        json!({ "ok": true, "ts": chrono::Utc::now().to_rfc3339() }),
    );
    (StatusCode::OK, axum::Json(ApiResponse::<()>::ok(()))).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn publish_reaches_all_subscribers() {
        let hub = RealtimeHub::new();
        let mut rx1 = hub.subscribe();
        let mut rx2 = hub.subscribe();
        hub.publish("broadcast", json!({ "ok": true, "seq": 1 }));
        let m1 = rx1.recv().await.expect("m1");
        let m2 = rx2.recv().await.expect("m2");
        assert_eq!(m1, m2);
        assert!(m1.contains("\"type\":\"broadcast\""));
    }

    #[tokio::test]
    async fn subscriber_does_not_receive_pre_subscription_events() {
        let hub = RealtimeHub::new();
        hub.publish("pre", json!({ "n": 1 }));
        let mut rx = hub.subscribe();
        let received = tokio::time::timeout(std::time::Duration::from_millis(50), rx.recv()).await;
        assert!(received.is_err(), "no debe recuperar mensajes previos");
    }
}
