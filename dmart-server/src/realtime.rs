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
    extract::ConnectInfo,
    http::StatusCode,
    response::sse::{Event, KeepAlive, Sse},
    response::{IntoResponse, Response},
};
use dmart_shared::models::ApiResponse;
use futures_util::stream::unfold;
use serde_json::json;
use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::OnceLock;
use tokio::sync::broadcast;

/// Máximo de conexiones SSE concurrentes por IP (previene acaparar el hub con
/// miles de streams desde un único origen).
pub const MAX_SSE_PER_IP: usize = 10;

/// Registro de conexiones SSE activas por IP.
static SSE_CONNECTIONS: OnceLock<std::sync::RwLock<HashMap<String, usize>>> = OnceLock::new();

fn sse_connections() -> &'static std::sync::RwLock<HashMap<String, usize>> {
    SSE_CONNECTIONS.get_or_init(|| std::sync::RwLock::new(HashMap::new()))
}

/// Intenta reservar una plaza SSE para `ip`. Devuelve `false` si la IP ya tiene
/// `MAX_SSE_PER_IP` conexiones activas.
fn sse_try_acquire(ip: &str) -> bool {
    let mut map = sse_connections()
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let count = map.entry(ip.to_string()).or_default();
    if *count >= MAX_SSE_PER_IP {
        false
    } else {
        *count += 1;
        true
    }
}

/// Libera una plaza SSE para `ip` (llamado cuando el stream se cae).
fn sse_release(ip: &str) {
    let mut map = sse_connections()
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(count) = map.get_mut(ip) {
        *count = count.saturating_sub(1);
        if *count == 0 {
            map.remove(ip);
        }
    }
}

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
        // Ignorar errores ("no receivers") de forma silenciosa; la telemetría de
        // SPEC-044 solo cuenta la actividad (publish sin suscriptores es normal).
        let _ = self.tx.send(message);
        crate::support::note("realtime", true);
    }

    /// Suscribe un receptor al canal de este hub.
    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.tx.subscribe()
    }

    /// Construye un stream SSE que emite los eventos del hub. `client_ip` se
    /// usa para el registro de conexiones concurrentes por IP.
    pub fn sse_stream(
        &self,
        client_ip: String,
    ) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
        let rx = self.subscribe();
        let ip = client_ip;

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
        Sse::new(CountedStream { inner: stream, ip }).keep_alive(
            KeepAlive::new().interval(std::time::Duration::from_secs(keepalive::INTERVAL_SECS)),
        )
    }
}

/// Wrapper de stream que decrementa `sse_connections_active` y libera la plaza
/// por IP cuando la conexión se cierra (el `Drop` se ejecuta al terminar o
/// abandonar el stream).
struct CountedStream<S> {
    inner: S,
    ip: String,
}

impl<S> Drop for CountedStream<S> {
    fn drop(&mut self) {
        crate::metrics::sse_disconnect();
        sse_release(&self.ip);
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
///
/// Limita a `MAX_SSE_PER_IP` conexiones concurrentes por IP origen.
pub async fn realtime_stream(ConnectInfo(addr): ConnectInfo<SocketAddr>) -> Response {
    let ip = addr.ip().to_string();
    if !sse_try_acquire(&ip) {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            axum::Json(ApiResponse::<()>::err(
                "Demasiadas conexiones en tiempo real desde esta IP".to_string(),
            )),
        )
            .into_response();
    }
    global_hub().sse_stream(ip).into_response()
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

    #[test]
    fn sse_per_ip_limit_enforced() {
        let ip = "10.0.0.99";
        // limpiar por si quedó basura de otros tests
        sse_release(ip);
        assert!(sse_try_acquire(ip));
        for _ in 1..MAX_SSE_PER_IP {
            assert!(sse_try_acquire(ip), "debe permitir hasta MAX_SSE_PER_IP");
        }
        assert!(
            !sse_try_acquire(ip),
            "debe rechazar la conexión #(MAX_SSE_PER_IP + 1)"
        );
        sse_release(ip);
        assert!(sse_try_acquire(ip), "debe liberar plaza tras release");
        sse_release(ip);
    }
}
