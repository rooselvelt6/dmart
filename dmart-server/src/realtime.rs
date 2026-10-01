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

use crate::auth::Claims;
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

/// Filtrado de eventos por tenant en el stream SSE.
///
/// Un evento se entrega al suscriptor si:
/// - su sobre lleva `tenant_id` y coincide con el del suscriptor, o
/// - el sobre **no** lleva `tenant_id` (evento no clínico/no particionado).
///
/// En multi-tenancy (`DMART_MULTI_TENANT=1`) los eventos sin `tenant_id` se
/// **suprimen** para que un evento mal publicado no pueda filtrar PHI a un
/// hospital equivocado. En single-tenant se comportan como antes.
fn event_visible_to(message: &str, tenant_id: &str) -> bool {
    let tenant: Option<String> = serde_json::from_str::<serde_json::Value>(message)
        .ok()
        .and_then(|v| v.get("tenant").and_then(|t| t.as_str()).map(str::to_owned));

    match tenant.as_deref() {
        Some(t) => t == tenant_id,
        None => !crate::tenant::multi_tenant_enabled(),
    }
}

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

    fn broadcast(&self, tenant_id: Option<&str>, event_type: &str, payload: serde_json::Value) {
        let envelope = match tenant_id {
            Some(t) => json!({ "type": event_type, "tenant": t, "data": payload }),
            None => json!({ "type": event_type, "data": payload }),
        };
        let message = envelope.to_string();
        // Ignorar errores ("no receivers") de forma silenciosa; la telemetría de
        // SPEC-044 solo cuenta la actividad (publish sin suscriptores es normal).
        let _ = self.tx.send(message);
        crate::support::note("realtime", true);
        crate::slo::record_sse_publish();
    }

    /// Publica un evento **particionado por tenant**: solo llega a suscriptores
    /// del mismo hospital.
    pub fn publish_for_tenant(
        &self,
        tenant_id: &str,
        event_type: &str,
        payload: serde_json::Value,
    ) {
        self.broadcast(Some(tenant_id), event_type, payload);
    }

    /// Publica un evento JSON a todos los suscriptores de este hub.
    pub fn publish(&self, event_type: &str, payload: serde_json::Value) {
        self.broadcast(None, event_type, payload);
    }

    /// Suscribe un receptor al canal de este hub.
    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.tx.subscribe()
    }

    /// Construye un stream SSE que emite los eventos del hub **acotados al
    /// tenant** del suscriptor. `client_ip` se usa para el registro de
    /// conexiones concurrentes por IP.
    pub fn sse_stream(
        &self,
        client_ip: String,
        tenant_id: String,
    ) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
        let rx = self.subscribe();
        let ip = client_ip;

        let stream = unfold((rx, tenant_id.clone()), |(mut rx, t)| async move {
            loop {
                match rx.recv().await {
                    Ok(message) => {
                        if !event_visible_to(&message, &t) {
                            continue;
                        }
                        return Some((
                            Ok::<_, Infallible>(Event::default().data(message)),
                            (rx, t),
                        ));
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
    pub tenant_id: String,
    pub apache_score: f64,
    pub news2_score: f64,
    pub sofa_score: f64,
    pub timestamp: String,
    pub severity: EwsSeverity,
}

/// Publica un ScoreEvent en el hub global (SPEC-014), acotado a su tenant.
pub fn publish_event(event: ScoreEvent) {
    let tenant_id = event.tenant_id.clone();
    global_hub().publish_for_tenant(
        &tenant_id,
        "score",
        serde_json::to_value(event).expect("ScoreEvent serialize"),
    );
}

static REALTIME_TX: OnceLock<RealtimeHub> = OnceLock::new();

fn global_hub() -> &'static RealtimeHub {
    REALTIME_TX.get_or_init(RealtimeHub::new)
}

/// Publica un evento JSON **acotado al tenant** indicado (hub global).
pub fn publish_for_tenant(tenant_id: &str, event_type: &str, payload: serde_json::Value) {
    global_hub().publish_for_tenant(tenant_id, event_type, payload);
}

/// Publica un evento JSON a todos los clientes conectados (hub global).
///
/// ⚠️ Reservado a eventos **no particionados** (ping, estado de servicio). Los
/// eventos clínicos deben usar [`publish_for_tenant`] con el tenant del
/// paciente: en multi-tenancy un evento sin `tenant` se suprime en el stream.
pub fn publish(event_type: &str, payload: serde_json::Value) {
    global_hub().publish(event_type, payload);
}

/// Publica un evento clínico resolviendo el tenant desde el paciente.
///
/// Si el paciente no existe en la base no se publica nada: es preferible perder
/// el evento a emitirlo sin particionar (en multi-tenancy se suprimiría, pero en
/// single-tenant filtraría PHI de un hospital a otro).
pub async fn publish_for_patient(
    db: &crate::db::Database,
    patient_id: &str,
    event_type: &str,
    payload: serde_json::Value,
) {
    match crate::db::get_patient(db, patient_id).await {
        Ok(Some(p)) => publish_for_tenant(&p.tenant_id, event_type, payload),
        Ok(None) => tracing::warn!(
            "[realtime] evento {event_type} suprimido: paciente {patient_id} no encontrado"
        ),
        Err(e) => {
            tracing::warn!("[realtime] no se pudo resolver tenant para evento {event_type}: {e}")
        }
    }
}

/// GET /api/realtime/stream — Streaming SSE de eventos del hub global.
///
/// El `tenant_id` se toma de los claims del JWT y se aplica como filtro en el
/// servidor: un suscriptor solo recibe eventos de su propio hospital, aunque
/// el hub sea compartido por toda la instancia.
pub async fn realtime_stream(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    claims: Claims,
) -> Response {
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
    global_hub()
        .sse_stream(ip, claims.tenant_id)
        .into_response()
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

    /// El sobre de un evento particionado lleva `tenant`, para que el
    /// suscriptor pueda filtrarlo sin tocar el payload clínico.
    #[tokio::test]
    async fn tenant_event_envelope_carries_tenant() {
        let hub = RealtimeHub::new();
        let mut rx = hub.subscribe();
        hub.publish_for_tenant("hosp-b", "measurement", json!({ "apache_score": 12 }));
        let message = rx.recv().await.expect("message");
        assert!(message.contains("\"tenant\":\"hosp-b\""));
        assert!(message.contains("\"apache_score\":12"));
        assert!(event_visible_to(&message, "hosp-b"));
        assert!(!event_visible_to(&message, "hosp-a"));
    }

    /// SPEC-025: un evento clínico de `hosp-b` no puede pasar a un suscriptor de
    /// `hosp-a`, ni siquiera si comparten instancia/hub.
    #[tokio::test]
    async fn tenant_event_is_not_visible_to_other_tenant() {
        let hub = RealtimeHub::new();
        let mut rx = hub.subscribe();
        hub.publish_for_tenant("hosp-b", "measurement", json!({ "patient_id": "p2" }));
        let message = rx.recv().await.expect("message");
        assert!(
            !event_visible_to(&message, "hosp-a"),
            "PHI de hosp-b filtrada a hosp-a"
        );
        assert!(event_visible_to(&message, "hosp-b"));
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
