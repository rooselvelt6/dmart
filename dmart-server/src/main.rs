use axum::{
    Json, Router,
    extract::DefaultBodyLimit,
    http::{HeaderValue, Method, StatusCode, header},
    middleware as axum_mw,
    response::IntoResponse,
};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    services::{ServeDir, ServeFile},
    trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer},
};

use dmart_server::api;
use dmart_server::audit;
use dmart_server::auth;
use dmart_server::auth::AuthService;
use dmart_server::cache;
use dmart_server::crypto;
use dmart_server::db;
use dmart_server::ingest::{IngestConfig, IngestState};
use dmart_server::middleware::auth_mod::AuthMiddlewareConfig;
use dmart_server::observability::{
    connect_with_retry, graceful_shutdown, init_metrics, init_tracing, observability_router,
};
use dmart_server::security;
use dmart_server::security::create_security_state;
use dmart_server::server_ingest;

/// 404 JSON para rutas API/observabilidad inexistentes (no deben caer al SPA).
async fn api_not_found() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({ "success": false, "error": "not found" })),
    )
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env file first (before any env var reads)
    dotenvy::dotenv().ok();

    // Fail fast if a strong DMART_MASTER_KEY is not configured
    crypto::validate_master_key().map_err(|e| anyhow::anyhow!("{}", e))?;

    // Setup panic hook FIRST - before any async code
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        eprintln!("💥 PANIC in dmart-server: {:?}", info);
        default_panic(info);
    }));

    // ── Observability: Tracing (JSON + optional OpenTelemetry) ─────────
    init_tracing()?;
    tracing::info!("🏥 UCI-DMART Server initializing...");

    // Use absolute path for data persistence
    let db_path = std::env::var("DMART_DB_PATH").unwrap_or_else(|_| {
        let base = std::env::current_dir().unwrap_or_default();
        base.join("data/dmart.db")
            .to_str()
            .unwrap_or("./data/dmart.db")
            .to_string()
    });

    // Ensure data directory exists
    if let Some(parent) = std::path::Path::new(&db_path).parent() {
        std::fs::create_dir_all(parent)?;
        tracing::info!("📁 Data directory: {}", parent.display());
    }

    // Connect to DB with exponential backoff retry
    let max_retries = std::env::var("DMART_DB_MAX_RETRIES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);
    let base_delay = Duration::from_secs(
        std::env::var("DMART_DB_BASE_DELAY_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(2),
    );

    let database = Arc::new(connect_with_retry(&db_path, max_retries, base_delay).await?);
    tracing::info!("✅ SurrealDB connected at {}", db_path);

    // Initialize audit service
    audit::init_global_audit((*database).clone());
    if let Some(audit) = audit::audit() {
        audit.init_chain().await;
    }

    // Seed default admin user if no users exist
    auth::seed_default_admin(&database).await?;

    // Seed institution config if none exists
    db::seed_institucion_config(&database).await?;

    // Seed CIE-10 catalog if it does not survive yet (task F2-5)
    db::seed_diagnosticos(&database).await?;

    // Seed initial beds (4 camas) if none exist
    let camas_existentes = db::list_camas(&database).await?;
    if camas_existentes.is_empty() {
        db::init_camas(&database, 2, dmart_shared::models::TipoCama::General).await?;
        db::init_camas(&database, 1, dmart_shared::models::TipoCama::Aislamiento).await?;
        db::init_camas(&database, 1, dmart_shared::models::TipoCama::Pediatrica).await?;
        tracing::info!("🛏️ Seeded 4 initial beds (2 General, 1 Aislamiento, 1 Pediátrica)");
    }

    // Cache (opcional — no bloquea si no está disponible)
    let valkey_url =
        std::env::var("DMART_VALKEY_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    let cache_ok = cache::init_global_cache(&valkey_url).await;
    if cache_ok {
        tracing::info!("✅ Valkey cache connected");
    }

    // ── Ingest Hardening (SPEC-031) ───────────────────────────────────
    let ingest_config = IngestConfig::default();
    let ingest_state = Arc::new(IngestState::new(ingest_config.clone()));
    // SPEC-044: la consola de soporte y el self-healing acceden al estado global.
    dmart_server::ingest::init_global_ingest(ingest_state.clone());
    tracing::info!(
        "🛡️ Ingest hardening enabled: rate_limit={} rps, burst={}, CB threshold={}%",
        ingest_config.rate_limit_rps,
        ingest_config.rate_limit_burst,
        (ingest_config.cb_error_threshold * 100.0) as u32
    );

    // ── Observability: Metrics ──────────────────────────────────────
    let prometheus_handle = init_metrics()?;

    // ── Security Setup ──────────────────────────────────────────────

    // Create AuthService for JWT verification
    let auth_service = AuthService::new((*database).clone());
    let auth_config = AuthMiddlewareConfig::new(auth_service);

    // Create security state (rate limiter + login throttle)
    let security_state = create_security_state();

    // ── CORS ────────────────────────────────────────────────────────
    // Orígenes permitidos vía `DMART_CORS_ORIGIN` (CSV). Si la variable está
    // vacía o mal formada se hace *fail-closed* a los orígenes locales por
    // defecto, nunca se abre la API a cualquier origen.

    let cors_origins = std::env::var("DMART_CORS_ORIGIN")
        .unwrap_or_else(|_| "http://localhost:3000,http://127.0.0.1:3000".to_string());

    let origins: Vec<HeaderValue> = cors_origins
        .split(',')
        .filter_map(|s| HeaderValue::from_str(s.trim()).ok())
        .collect();

    // Allowlist estricta de headers: el frontend (Leptos) solo envía
    // `Authorization` y `Content-Type`; no hace falta `Any`.
    let allowed_headers = [header::AUTHORIZATION, header::CONTENT_TYPE, header::ACCEPT];

    let default_origins = [
        HeaderValue::from_static("http://localhost:3000"),
        HeaderValue::from_static("http://127.0.0.1:3000"),
    ];

    let cors = if origins.is_empty() {
        tracing::warn!(
            "⚠️ DMART_CORS_ORIGIN vacío o inválido — fallback a localhost; NO se habilita cualquier origen"
        );
        CorsLayer::new()
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::DELETE,
                Method::OPTIONS,
            ])
            .allow_headers(allowed_headers)
            .allow_origin(AllowOrigin::list(default_origins))
    } else {
        tracing::info!("🔒 CORS restricted to origins: {:?}", origins);
        CorsLayer::new()
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::DELETE,
                Method::OPTIONS,
            ])
            .allow_headers(allowed_headers)
            .allow_origin(AllowOrigin::list(origins))
    };

    // ── API Router (rutas + middleware de seguridad) ────────────────

    let api_router = api::build_api_router(database.clone(), auth_config, security_state);

    // ── Observability Router (/health, /live, /ready, /metrics) ───────
    let obs_router = observability_router(database.clone(), prometheus_handle);

    // ── Static files ────────────────────────────────────────────────
    // SPA fallback: si el fichero pedido no existe en `dist/`, se sirve
    // `index.html` para que el router de Leptos resuelva la ruta. Esto evita
    // mantener a mano la lista de rutas del frontend (bug de `/perfil`).
    let dist_path = std::env::var("DMART_DIST_PATH").unwrap_or_else(|_| "./dist".to_string());
    let index_path = format!("{}/index.html", dist_path);
    let static_files = ServeDir::new(&dist_path).fallback(ServeFile::new(&index_path));

    let app = Router::new()
        .nest("/api", api_router.fallback(api_not_found))
        .nest("/obs", obs_router.fallback(api_not_found))
        .fallback_service(static_files)
        .layer(DefaultBodyLimit::max(1024 * 1024)) // 1MB request body limit
        .layer(cors)
        .layer(axum_mw::from_fn(security::security_headers_middleware))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().include_headers(true))
                .on_response(DefaultOnResponse::new().include_headers(true)),
        );

    // Server
    let port: u16 = std::env::var("DMART_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    tracing::info!("🚀 Server running at http://{}", addr);
    tracing::info!("    API:      http://{}/api/patients", addr);
    tracing::info!("    Frontend: http://{}/", addr);
    tracing::info!("    Obs:      http://{}/obs/health", addr);
    tracing::info!("    Metrics:  http://localhost:9090/metrics");

    // Listener HL7/MLLP para monitores de cama (opcional, se aísla por VLAN).
    if let Some(hl7_port) = std::env::var("DMART_HL7_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
    {
        let hl7_db = database.clone();
        let hl7_ingest = ingest_state.clone();
        let hl7_addr = SocketAddr::from(([0, 0, 0, 0], hl7_port));
        tokio::spawn(async move {
            if let Err(e) = server_ingest::serve(hl7_addr, hl7_db, hl7_ingest).await {
                tracing::error!("[mllp] listener cerrado: {e}");
            }
        });
        tracing::info!("    HL7/MLLP: tcp://{}", hl7_addr);
    }

    let shutdown_timeout = Duration::from_secs(
        std::env::var("DMART_SHUTDOWN_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30),
    );

    let listener = tokio::net::TcpListener::bind(addr).await?;
    let server = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        graceful_shutdown(shutdown_timeout, || {
            Box::pin(async {
                // Here you could add cleanup logic:
                // - Flush metrics
                // - Close DB connections
                // - Shutdown cache
                tracing::info!("🧹 Cleanup completed");
            })
        })
        .await;
    });

    server.await?;

    tracing::info!("🛑 Server shutdown complete");
    Ok(())
}
