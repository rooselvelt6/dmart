mod db;
mod cache;
mod api;
mod crypto;

use std::net::SocketAddr;
use axum::{
    Router,
    routing::get,
    http::{Method, StatusCode},
    response::{Html, IntoResponse},
};
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

async fn health_check() -> impl axum::response::IntoResponse {
    let now = chrono::Utc::now().to_rfc3339();
    let response = HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        timestamp: now,
    };
    (axum::http::StatusCode::OK, axum::Json(response))
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
    // Logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "dmart_server=info,tower_http=info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("🏥  UCI-DMART Server iniciando...");

    // Database
    let db_path = std::env::var("DMART_DB_PATH")
        .unwrap_or_else(|_| "./data/dmart.db".to_string());
    let database = db::connect(&db_path).await?;
    tracing::info!("✅  SurrealDB (RocksDB) conectado en {}", db_path);

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
        // Patients
        .route("/patients", get(api::patients::list_patients).post(api::patients::create_patient))
        .route("/patients/{id}", get(api::patients::get_patient).put(api::patients::update_patient).delete(api::patients::delete_patient))
        // Measurements
        .route("/patients/{id}/measurements", get(api::measurements::get_measurements).post(api::measurements::create_measurement))
        .route("/patients/{id}/measurements/last", get(api::measurements::get_last_measurement))
        // Export
        .route("/patients/{id}/export/csv", get(api::export::export_csv))
        .route("/patients/{id}/export/pdf", get(api::export::export_pdf))
        .with_state(database);

    // Static files (serving compiled WASM frontend)
    let dist_path = std::env::var("DMART_DIST_PATH")
        .unwrap_or_else(|_| "./dist".to_string());

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
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    // Server
    let port: u16 = std::env::var("DMART_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    tracing::info!("🚀  Servidor corriendo en http://{}", addr);
    tracing::info!("    API:      http://{}/api/patients", addr);
    tracing::info!("    Frontend: http://{}/", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
