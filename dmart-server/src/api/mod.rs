pub mod admin;
pub mod auth;
pub mod cds;
pub mod diagnosticos;
pub mod escalation;
pub mod export;
pub mod fhir;
pub mod flags;
pub mod institucion;
pub mod measurements;
pub mod ml;
pub mod monitores;
pub mod patients;
pub mod quality;
pub mod registry;
pub mod retention;
pub mod sandbox;
pub mod scales;
pub mod score_audit;
pub mod stats;
pub mod teleicu;
pub mod tenant;
pub mod versioning;

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    middleware as axum_mw,
    response::IntoResponse,
    routing::{get, post},
};
use serde::Serialize;
use std::sync::OnceLock;
use std::time::Instant;

use crate::db::Database;
use crate::middleware::auth_mod::{AuthMiddlewareConfig, auth_middleware};
use crate::security::{SecurityState, login_throttle_middleware, rate_limit_middleware};

pub use versioning::{build_unified_router, VersionState};

fn start_instant() -> &'static Instant {
    static INSTANT: OnceLock<Instant> = OnceLock::new();
    INSTANT.get_or_init(Instant::now)
}

pub fn uptime_seconds() -> u64 {
    start_instant().elapsed().as_secs()
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub timestamp: String,
    pub uptime_seconds: u64,
    pub database: String,
    pub cache: String,
}



/// Builds the versioned API router (mounted under `/api/v1` and optionally `/api` legacy)
/// with all security middleware applied. Shared by the server binary and the E2E security tests.
pub fn build_api_router(
    database: Database,
    auth_config: AuthMiddlewareConfig,
    security_state: SecurityState,
) -> Router {
    build_unified_router(database, auth_config, security_state)
}
