pub mod admin;
pub mod auth;
pub mod cds;
pub mod diagnosticos;
pub mod escalation;
pub mod export;
pub mod fhir;
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

fn start_instant() -> &'static Instant {
    static INSTANT: OnceLock<Instant> = OnceLock::new();
    INSTANT.get_or_init(Instant::now)
}

pub fn uptime_seconds() -> u64 {
    start_instant().elapsed().as_secs()
}

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    version: String,
    timestamp: String,
    uptime_seconds: u64,
    database: String,
    cache: String,
}

async fn health_check(State(db): State<Database>) -> impl IntoResponse {
    let now = chrono::Utc::now().to_rfc3339();

    let db_status = match db.query("SELECT * FROM patients LIMIT 1").await {
        Ok(_) => "connected".to_string(),
        Err(e) => format!("error: {}", e),
    };

    let cache_status = if crate::cache::cache_available() {
        "connected".to_string()
    } else {
        "unavailable".to_string()
    };

    let response = HealthResponse {
        status: if db_status == "connected" {
            "healthy".to_string()
        } else {
            "degraded".to_string()
        },
        version: env!("CARGO_PKG_VERSION").to_string(),
        timestamp: now,
        uptime_seconds: uptime_seconds(),
        database: db_status,
        cache: cache_status,
    };
    (StatusCode::OK, axum::Json(response)).into_response()
}

/// Builds the API router (mounted under `/api`) with all security middleware
/// applied. Shared by the server binary and the E2E security tests so both run
/// the exact same stack.
pub fn build_api_router(
    database: Database,
    auth_config: AuthMiddlewareConfig,
    security_state: SecurityState,
) -> Router {
    let api_router = Router::new()
        // Health check
        .route("/health", get(health_check))
        // Realtime (SSE)
        .route("/realtime/stream", get(crate::realtime::realtime_stream))
        .route("/realtime/ping", get(crate::realtime::realtime_ping))
        // Monitores (HL7) — token vía query para gateways de red cerrada
        .route("/monitores/hl7", post(monitores::hl7_ingest))
        .route("/monitores/health", get(monitores::hl7_health))
        // Stats
        .route("/stats", get(stats::get_stats))
        // Admin
        .route("/admin/stats", get(admin::get_admin_stats))
        .route("/admin/camas/init", post(admin::init_camas_api))
        .route(
            "/admin/camas",
            get(admin::list_camas_api).post(admin::create_cama_api),
        )
        .route(
            "/admin/camas/{id}",
            get(admin::get_cama_api)
                .put(admin::update_cama_api)
                .delete(admin::delete_cama_api),
        )
        .route(
            "/admin/camas/disponibles",
            get(admin::get_camas_disponibles),
        )
        .route("/admin/check-camas", get(admin::check_camas_disponibles))
        .route(
            "/admin/equipos",
            get(admin::list_equipos_api).post(admin::create_equipo_api),
        )
        .route(
            "/admin/equipos/disponibles",
            get(admin::get_equipos_disponibles_api),
        )
        .route(
            "/admin/equipos/{id}",
            get(admin::get_equipo_api)
                .put(admin::update_equipo_api)
                .delete(admin::delete_equipo_api),
        )
        .route(
            "/admin/equipos/cama/{cama_id}",
            get(admin::list_equipos_por_cama_api),
        )
        .route(
            "/admin/equipos/asignar",
            post(admin::asignar_equipo_cama_api),
        )
        .route(
            "/admin/equipos/{equipo_id}/desvincular",
            post(admin::desvincular_equipo_api),
        )
        .route(
            "/admin/staff",
            get(admin::list_staff_api).post(admin::create_staff_api),
        )
        .route(
            "/admin/staff/{id}",
            get(admin::get_staff_api)
                .put(admin::update_staff_api)
                .delete(admin::delete_staff_api),
        )
        .route("/admin/staff/{id}/toggle", post(admin::toggle_user_active))
        // Auditoría (HIPAA, retención 6 años)
        .route("/admin/audit", get(admin::get_audit_logs_api))
        .route("/admin/audit/critical", get(admin::get_audit_critical_api))
        .route(
            "/admin/audit/cleanup",
            post(admin::run_audit_retention_cleanup),
        )
        // Auditoría de fingerprints de scores (SPEC-029)
        .route("/admin/audit/scores", get(score_audit::verify_scores))
        // Multi-tenancy (SPEC-025) — solo super_admin
        .route(
            "/admin/tenants",
            get(tenant::list_tenants_api).post(tenant::create_tenant_api),
        )
        .route(
            "/admin/tenants/{id}/impersonate",
            post(tenant::impersonate_tenant_api),
        )
        // ML Serving (SPEC-032) + Patient Similarity (SPEC-033)
        .merge(ml::routes())
        // Retención / downsampling (SPEC-030)
        .merge(retention::routes())
        // Auth
        .nest("/auth", auth::router())
        // Patients
        .route(
            "/patients",
            get(patients::list_patients).post(patients::create_patient),
        )
        .route(
            "/patients/{id}",
            get(patients::get_patient)
                .put(patients::update_patient)
                .delete(patients::delete_patient),
        )
        .route("/patients/{id}/egreso", post(patients::egreso_paciente))
        .route("/patients/{id}/timeline", get(patients::patient_timeline))
        // Clinical Decision Support
        .route("/cds/plans", get(cds::list_plans))
        .route("/cds/evaluate", post(cds::evaluate_cds))
        // Measurements (registro completo)
        .route(
            "/patients/{id}/measurements",
            get(measurements::get_measurements).post(measurements::create_measurement),
        )
        .route(
            "/patients/{id}/measurements/last",
            get(measurements::get_last_measurement),
        )
        // Escalas individuales
        .route("/patients/{id}/scales/apache", post(scales::calc_apache))
        .route("/patients/{id}/scales/gcs", post(scales::calc_gcs))
        .route("/patients/{id}/scales/news2", post(scales::calc_news2))
        .route("/patients/{id}/scales/sofa", post(scales::calc_sofa))
        .route("/patients/{id}/scales/saps3", post(scales::calc_saps3))
        .route("/patients/{id}/scales/history", get(scales::scale_history))
        // Export
        .route("/patients/{id}/export/csv", get(export::export_csv))
        .route("/patients/{id}/export/pdf", get(export::export_pdf))
        // Institucion
        .route(
            "/admin/institucion",
            get(institucion::get_institucion).put(institucion::upsert_institucion),
        )
        // Diagnosticos CIE-10
        .route("/diagnosticos", get(diagnosticos::list_diagnosticos))
        .route(
            "/diagnosticos/search",
            get(diagnosticos::search_diagnosticos),
        )
        // Sandbox
        .route("/sandbox/generate", post(sandbox::generate_patients))
        .route("/sandbox/clear", post(sandbox::clear_sandbox))
        // Device Registry (SPEC-017)
        .route("/devices/status", get(registry::device_status_summary))
        .route(
            "/devices",
            get(registry::list_devices).post(registry::register_device),
        )
        .route("/devices/{id}", get(registry::get_device))
        .route("/devices/{id}/heartbeat", post(registry::heartbeat_device))
        // Data Quality (SPEC-018)
        .route("/data-quality/validate", post(quality::validate_message))
        .route("/data-quality/report", get(quality::quality_report))
        .route("/data-quality/summary", get(quality::quality_summary))
        // Alert Escalation (SPEC-019)
        .route(
            "/escalation/policies",
            get(escalation::list_policies).post(escalation::set_policy),
        )
        .route("/escalation/active", get(escalation::active_escalations))
        .route("/escalation/{id}/ack", post(escalation::acknowledge_alert))
        .route(
            "/escalation/{id}/escalate",
            post(escalation::escalate_alert),
        )
        // Tele-ICU (SPEC-020)
        .route(
            "/teleicu/sessions",
            get(teleicu::list_sessions).post(teleicu::start_session),
        )
        .route("/teleicu/sessions/{id}/end", post(teleicu::end_session))
        .route("/teleicu/live/{id}", get(teleicu::live_view))
        // FHIR R4
        .route("/fhir/Patient", get(fhir::fhir_patient_search))
        .route("/fhir/Patient/{id}", get(fhir::fhir_patient_get))
        .route(
            "/fhir/Patient/{id}/Observation",
            get(fhir::fhir_observation_list),
        )
        .route(
            "/fhir/Patient/{id}/Condition",
            get(fhir::fhir_condition_list),
        )
        .route(
            "/fhir/Patient/{id}/DiagnosticReport",
            get(fhir::fhir_diagnostic_report),
        )
        .route(
            "/fhir/Patient/{id}/DiagnosticReport/QR",
            get(fhir::fhir_diagnostic_report_qr),
        )
        .with_state(database);

    // Apply security middleware (layers wrap from outside in)
    api_router
        .layer(axum_mw::from_fn_with_state(
            security_state.clone(),
            login_throttle_middleware,
        ))
        .layer(axum_mw::from_fn_with_state(
            security_state.clone(),
            rate_limit_middleware,
        ))
        .layer(axum_mw::from_fn_with_state(auth_config, auth_middleware))
}
