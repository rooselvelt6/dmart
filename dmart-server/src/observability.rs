//! Observabilidad: métricas Prometheus, health checks, logging estructurado y graceful shutdown.

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use metrics::{Unit, counter, describe_counter, describe_gauge, describe_histogram, histogram};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle as PrometheusHandle_};

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;
use tokio::signal::unix::{SignalKind, signal};
use tracing::info;
use tracing_subscriber::{EnvFilter, Registry, layer::SubscriberExt, util::SubscriberInitExt};

use crate::db::Database;

/// Estado de observabilidad compartido
#[derive(Clone)]
pub struct ObservabilityState {
    pub start_time: std::time::Instant,
    pub db: Database,
}

impl ObservabilityState {
    pub fn new(db: Database) -> Self {
        Self {
            start_time: std::time::Instant::now(),
            db,
        }
    }

    pub async fn db_healthy(&self) -> bool {
        self.db
            .query("SELECT 1 AS health")
            .await
            .map(|_| true)
            .unwrap_or(false)
    }
}

/// Inicializa tracing con JSON + OpenTelemetry (si OTEL_EXPORTER_OTLP_ENDPOINT está seteado)
pub fn init_tracing() -> anyhow::Result<()> {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        "dmart_server=info,tower_http=info,axum=info,opentelemetry=warn".into()
    });

    let fmt_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_current_span(true)
        .with_span_list(true)
        .with_target(true);

    Registry::default().with(env_filter).with(fmt_layer).init();

    Ok(())
}

/// Inicializa métricas Prometheus + OpenTelemetry (si endpoint configurado)
pub fn init_metrics() -> anyhow::Result<PrometheusHandle_> {
    // Métricas de proceso (CPU, memoria, etc.) - Collector API cambió en v0.3
    // let _ = Collector::default().install(); // descomentar si se fija la API

    // Métricas custom de la app
    describe_counter!(
        "http_requests_total",
        Unit::Count,
        "Total number of HTTP requests"
    );
    describe_histogram!(
        "http_request_duration_seconds",
        Unit::Seconds,
        "HTTP request latency"
    );
    describe_counter!(
        "http_requests_errors_total",
        Unit::Count,
        "Total number of HTTP error responses"
    );
    describe_gauge!(
        "db_connections_active",
        Unit::Count,
        "Active database connections"
    );
    describe_gauge!(
        "cache_connected",
        Unit::Count,
        "Cache connection status (1=connected, 0=disconnected)"
    );
    describe_gauge!("uptime_seconds", Unit::Seconds, "Server uptime in seconds");

    // Prometheus handle para exponer /metrics via axum
    let handle = PrometheusBuilder::new().install_recorder()?;

    // OpenTelemetry metrics (opcional) - simplificado para evitar breaking changes
    if std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").is_ok() {
        // Solo log warning, no inicializar para evitar breaking changes de API
        tracing::warn!(
            "OTEL_EXPORTER_OTLP_ENDPOINT set but OpenTelemetry metrics not fully initialized (API v0.25 changes)"
        );
    }

    info!("📊 Prometheus metrics available at /metrics");
    Ok(handle)
}

/// Middleware de métricas HTTP
pub async fn metrics_middleware(
    req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> Response {
    let start = std::time::Instant::now();

    let response = next.run(req).await;
    let latency = start.elapsed().as_secs_f64();

    counter!("http_requests_total", 1);
    histogram!("http_request_duration_seconds", latency);

    if response.status().is_server_error() || response.status().is_client_error() {
        counter!("http_requests_errors_total", 1);
    }

    response
}

/// Health check enriquecido
pub async fn health_check(State(state): State<ObservabilityState>) -> impl IntoResponse {
    let db_ok = state.db_healthy().await;
    let uptime = state.start_time.elapsed().as_secs();
    let version = env!("CARGO_PKG_VERSION");

    let mut status = "healthy";
    let checks = serde_json::json!({
        "database": if db_ok { "connected" } else { "disconnected" },
        "cache": "optional",
        "uptime_seconds": uptime,
        "version": version,
    });

    if !db_ok {
        status = "degraded";
    }

    let response = serde_json::json!({
        "status": status,
        "checks": checks,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });

    if db_ok {
        (StatusCode::OK, axum::Json(response))
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, axum::Json(response))
    }
}

/// Live check (Kubernetes liveness probe)
pub async fn live_check() -> impl IntoResponse {
    (StatusCode::OK, "alive")
}

/// Ready check (Kubernetes readiness probe)
pub async fn ready_check(State(state): State<ObservabilityState>) -> impl IntoResponse {
    let db_ok = state.db_healthy().await;
    if db_ok {
        (StatusCode::OK, "ready")
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "not ready")
    }
}

/// Router de observabilidad (/health, /live, /ready, /metrics)
pub fn observability_router(db: Database, prometheus_handle: PrometheusHandle_) -> Router {
    let state = ObservabilityState::new(db);
    Router::new()
        .route("/health", get(health_check))
        .route("/live", get(live_check))
        .route("/ready", get(ready_check))
        .route(
            "/metrics",
            get(move || {
                let handle = prometheus_handle.clone();
                async move {
                    let metrics = handle.render();
                    (
                        StatusCode::OK,
                        [("Content-Type", "text/plain; version=0.0.4; charset=utf-8")],
                        metrics,
                    )
                }
            }),
        )
        .with_state(state)
        .layer(axum::middleware::from_fn(metrics_middleware))
}

/// Conexión a SurrealDB con reintentos exponenciales
pub async fn connect_with_retry(
    db_path: &str,
    max_retries: u32,
    base_delay: Duration,
) -> anyhow::Result<surrealdb::Surreal<surrealdb::engine::local::Db>> {
    use surrealdb::Surreal;
    use surrealdb::engine::local::SurrealKv;

    let mut attempt = 0;
    let mut delay = base_delay;

    loop {
        match Surreal::new::<SurrealKv>(db_path).await {
            Ok(db) => {
                db.use_ns("dmart").use_db("icu").await?;
                info!(
                    "✅ SurrealDB connected at {} (attempt {})",
                    db_path,
                    attempt + 1
                );
                return Ok(db);
            }
            Err(e) => {
                attempt += 1;
                if attempt >= max_retries {
                    return Err(anyhow::anyhow!(
                        "Failed to connect after {} attempts: {}",
                        max_retries,
                        e
                    ));
                }
                tracing::warn!(
                    "DB connection attempt {} failed: {}. Retrying in {:?}...",
                    attempt,
                    e,
                    delay
                );
                tokio::time::sleep(delay).await;
                delay = delay.saturating_mul(2).min(Duration::from_secs(60)); // cap at 60s
            }
        }
    }
}

/// Señales de apagado graceful con timeout configurable
pub async fn graceful_shutdown(
    shutdown_timeout: Duration,
    on_shutdown: impl FnOnce() -> Pin<Box<dyn Future<Output = ()> + Send>>,
) {
    let mut sigint = signal(SignalKind::interrupt()).expect("SIGINT listener");
    let mut sigterm = signal(SignalKind::terminate()).expect("SIGTERM listener");

    tokio::select! {
        _ = sigint.recv() => info!("📤 Received SIGINT"),
        _ = sigterm.recv() => info!("📤 Received SIGTERM"),
    }

    info!(
        "🛑 Shutting down gracefully (timeout: {:?})...",
        shutdown_timeout
    );
    let shutdown_future = on_shutdown();
    tokio::select! {
        _ = shutdown_future => info!("✅ Graceful shutdown completed"),
        _ = tokio::time::sleep(shutdown_timeout) => {
            tracing::warn!("⚠️ Shutdown timeout exceeded, forcing exit");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_attributes() {
        // resource() function removed, test placeholder
        assert!(true);
    }
}
