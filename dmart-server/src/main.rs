mod db;
mod cache;
pub mod api;
mod crypto;
pub mod auth;
pub mod rbac;
mod audit;
mod security;

use std::net::SocketAddr;
use axum::{
    Router,
    routing::{get, post},
    http::{Method, StatusCode, HeaderName, HeaderValue},
    response::{Html, IntoResponse},
};
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::{
    cors::{CorsLayer, Any},
    services::ServeDir,
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use serde::Serialize;

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    version: String,
    timestamp: String,
}

async fn health_check() -> impl IntoResponse {
    let now = chrono::Utc::now().to_rfc3339();
    let response = HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        timestamp: now,
    };
    let mut res = (StatusCode::OK, axum::Json(response)).into_response();
    // Add security headers
    for (name, value) in security::security_headers() {
        res.headers_mut().insert(name, value);
    }
    res
}

async fn spa_handler() -> impl IntoResponse {
    let dist_path = std::env::var("DMART_DIST_PATH")
        .unwrap_or_else(|_| "./dist".to_string());
    let index_path = format!("{}/index.html", dist_path);
    
    match std::fs::read_to_string(&index_path) {
        Ok(content) => (StatusCode::OK, Html(content)),
        Err(_) => (StatusCode::NOT_FOUND, Html("<h1>404 - Not Found</h1><p>Index not found</p>".to_string())),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Setup panic hook FIRST - before any async code
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        eprintln!("💥 PANIC in dmart-server: {:?}", info);
        default_panic(info);
    }));

    // Logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "dmart_server=info,tower_http=info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

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
    
    let database = db::connect(&db_path).await?;
    tracing::info!("✅ SurrealDB connected at {}", db_path);

    // Seed default admin user if no users exist
    auth::seed_default_admin(&database).await?;

    // Seed initial beds (4 camas) if none exist
    let camas_existentes = db::list_camas(&database).await?;
    if camas_existentes.is_empty() {
        db::init_camas(&database, 2, dmart_shared::models::TipoCama::General).await?;
        db::init_camas(&database, 1, dmart_shared::models::TipoCama::Aislamiento).await?;
        db::init_camas(&database, 1, dmart_shared::models::TipoCama::Pediatrica).await?;
        tracing::info!("🛏️ Seeded 4 initial beds (2 General, 1 Aislamiento, 1 Pediátrica)");
    }

    // Cache (opcional — no bloquea si no está disponible)
    let valkey_url = std::env::var("DMART_VALKEY_URL")
        .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    let _cache = cache::Cache::connect(&valkey_url).await;

    // CORS
    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
        .allow_headers(Any)
        .allow_origin(Any);

    // API Router
    let api_router = Router::new()
        // Health check
        .route("/health", get(health_check))
        // Stats
        .route("/stats", get(api::stats::get_stats))
        // Admin
        .route("/admin/stats", get(api::admin::get_admin_stats))
        .route("/admin/camas/init", post(api::admin::init_camas_api))
        .route("/admin/camas", get(api::admin::list_camas_api).post(api::admin::create_cama_api))
        .route("/admin/camas/{id}", get(api::admin::get_cama_api).put(api::admin::update_cama_api).delete(api::admin::delete_cama_api))
        .route("/admin/camas/disponibles", get(api::admin::get_camas_disponibles))
        .route("/admin/check-camas", get(api::admin::check_camas_disponibles))
        .route("/admin/equipos", get(api::admin::list_equipos_api).post(api::admin::create_equipo_api))
        .route("/admin/equipos/disponibles", get(api::admin::get_equipos_disponibles_api))
        .route("/admin/equipos/{id}", get(api::admin::get_equipo_api).put(api::admin::update_equipo_api).delete(api::admin::delete_equipo_api))
        .route("/admin/equipos/cama/{cama_id}", get(api::admin::list_equipos_por_cama_api))
        .route("/admin/equipos/asignar", post(api::admin::asignar_equipo_cama_api))
        .route("/admin/equipos/{equipo_id}/desvincular", post(api::admin::desvincular_equipo_api))
        .route("/admin/staff", get(api::admin::list_staff_api).post(api::admin::create_staff_api))
        .route("/admin/staff/{id}", get(api::admin::get_staff_api).put(api::admin::update_staff_api).delete(api::admin::delete_staff_api))
        .route("/admin/staff/{id}/toggle", post(api::admin::toggle_user_active))
        // Auth
        .nest("/auth", api::auth::router())
        // Patients
        .route("/patients", get(api::patients::list_patients).post(api::patients::create_patient))
        .route("/patients/{id}", get(api::patients::get_patient).put(api::patients::update_patient).delete(api::patients::delete_patient))
        .route("/patients/{id}/egreso", post(api::patients::egreso_paciente))
        // Measurements (registro completo)
        .route("/patients/{id}/measurements", get(api::measurements::get_measurements).post(api::measurements::create_measurement))
        .route("/patients/{id}/measurements/last", get(api::measurements::get_last_measurement))
        // Escalas individuales
        .route("/patients/{id}/scales/apache", axum::routing::post(api::scales::calc_apache))
        .route("/patients/{id}/scales/gcs", axum::routing::post(api::scales::calc_gcs))
        .route("/patients/{id}/scales/news2", axum::routing::post(api::scales::calc_news2))
        .route("/patients/{id}/scales/sofa", axum::routing::post(api::scales::calc_sofa))
        .route("/patients/{id}/scales/saps3", axum::routing::post(api::scales::calc_saps3))
        .route("/patients/{id}/scales/history", get(api::scales::scale_history))
        // Export
        .route("/patients/{id}/export/csv", get(api::export::export_csv))
        .route("/patients/{id}/export/pdf", get(api::export::export_pdf))
        .with_state(database);

    // Static files (serving compiled WASM frontend)
    let dist_path = std::env::var("DMART_DIST_PATH")
        .unwrap_or_else(|_| "./dist".to_string());

    // Use a simpler approach - add security headers to all responses via a callback
    // For rate limiting, we'll check on each API request
    
    let app = Router::new()
        .nest("/api", api_router)
        .fallback_service(ServeDir::new(&dist_path))
        .route("/", get(spa_handler))
        .route("/login", get(spa_handler))
        .route("/patients", get(spa_handler))
        .route("/patients/new", get(spa_handler))
        .route("/patients/{id}", get(spa_handler))
        .route("/patients/{id}/edit", get(spa_handler))
        .route("/patients/{id}/measure", get(spa_handler))
        .route("/patients/{id}/scales/apache", get(spa_handler))
        .route("/patients/{id}/scales/gcs", get(spa_handler))
        .route("/patients/{id}/scales/news2", get(spa_handler))
        .route("/patients/{id}/scales/sofa", get(spa_handler))
        .route("/patients/{id}/scales/saps3", get(spa_handler))
        .route("/scales", get(spa_handler))
        .route("/stats", get(spa_handler))
        .route("/admin", get(spa_handler))
        .route("/admin/{*path}", get(spa_handler))
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    // Add security headers to all responses
    let app = app.layer(SetResponseHeaderLayer::overriding(
        HeaderName::from_static("x-frame-options"),
        HeaderValue::from_static("DENY"),
    )).layer(SetResponseHeaderLayer::overriding(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    )).layer(SetResponseHeaderLayer::overriding(
        HeaderName::from_static("x-xss-protection"),
        HeaderValue::from_static("1; mode=block"),
    )).layer(SetResponseHeaderLayer::overriding(
        HeaderName::from_static("referrer-policy"),
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    )).layer(SetResponseHeaderLayer::overriding(
        HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static("geolocation=(), microphone=(), camera=()"),
    )).layer(SetResponseHeaderLayer::overriding(
        HeaderName::from_static("cross-origin-opener-policy"),
        HeaderValue::from_static("same-origin"),
    ));

    // Server
    let port: u16 = std::env::var("DMART_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    // Graceful shutdown setup
    let shutdown_signal = async {
        let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt()).unwrap();
        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap();
        tokio::select! {
            _ = sigint.recv() => tracing::info!("📤 Received SIGINT"),
            _ = sigterm.recv() => tracing::info!("📤 Received SIGTERM"),
        }
    };

    tracing::info!("🚀 Server running at http://{}", addr);
    tracing::info!("    API:      http://{}/api/patients", addr);
    tracing::info!("    Frontend: http://{}/", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal)
        .await?;
    
    tracing::info!("🛑 Server shutdown complete");
    Ok(())
}
