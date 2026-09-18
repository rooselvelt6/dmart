use axum::{
    Router,
    extract::State,
    http::{HeaderName, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde_json::json;
use std::sync::OnceLock;
use tower_http::set_header::SetResponseHeaderLayer;
use utoipa::OpenApi;

use crate::db::Database;
use crate::middleware::auth_mod::{AuthMiddlewareConfig, auth_middleware};
use crate::security::{SecurityState, login_throttle_middleware, rate_limit_middleware};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "dMart UCI API",
        version = "1.0.0",
        description = "API para gestión de UCI - dMart"
    ),
    servers(
        (url = "/api/v1", description = "API v1")
    ),
    components(
        schemas(
            crate::api::HealthResponse,
            crate::api::support::SupportSystem,
            crate::api::support::SliIndicator,
            crate::api::support::Diagnostic,
            crate::api::support::ActionResponse,
            crate::support::SupportEvent,
            crate::support::EventOrigin,
            crate::audit::AuditLog,
            crate::audit::AuditAction,
            crate::audit::AuditBatch,
            crate::audit::IntegrityReport,
            crate::audit::AuditExport,
            dmart_shared::models::FeatureFlag,
            dmart_shared::models::CreateFeatureFlagRequest,
            dmart_shared::models::UpdateFeatureFlagRequest,
            dmart_shared::models::FeatureFlagListResponse,
        )
    ),
    security(
        ("BearerAuth" = [])
    ),
    tags(
        (name = "health", description = "Health checks"),
        (name = "auth", description = "Autenticación y autorización"),
        (name = "patients", description = "Gestión de pacientes"),
        (name = "measurements", description = "Signos vitales y mediciones"),
        (name = "scales", description = "Escalas clínicas (APACHE, SOFA, NEWS2, GCS, SAPS3)"),
        (name = "ml", description = "Machine Learning: predicción mortalidad, forecasting, similaridad"),
        (name = "devices", description = "Registro de dispositivos/monitores"),
        (name = "data-quality", description = "Calidad de datos HL7"),
        (name = "escalation", description = "Políticas de escalamiento de alertas"),
        (name = "tenants", description = "Multi-tenancy (super-admin)"),
        (name = "admin", description = "Administración: camas, equipos, personal, auditoría"),
        (name = "flags", description = "Feature flags por tenant"),
        (name = "institucion", description = "Configuración de institución"),
        (name = "diagnosticos", description = "Catálogo CIE-10"),
        (name = "cds", description = "Clinical Decision Support"),
        (name = "fhir", description = "FHIR R4 endpoints"),
        (name = "retention", description = "Retención y downsampling"),
        (name = "teleicu", description = "Tele-ICU sessions"),
        (name = "sandbox", description = "Entorno de pruebas"),
        (name = "export", description = "Export CSV/PDF"),
        (name = "support", description = "Consola técnica de soporte (SPEC-044): diagnóstico y auto-recuperación"),
    )
)]
pub struct ApiDoc;

static API_VERSION: OnceLock<String> = OnceLock::new();

pub fn api_version() -> &'static str {
    API_VERSION
        .get_or_init(|| std::env::var("DMART_API_VERSION").unwrap_or_else(|_| "v1".to_string()))
}

pub fn legacy_enabled() -> bool {
    std::env::var("DMART_API_LEGACY_ENABLED")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(true)
}

#[derive(Clone)]
pub struct VersionState {
    pub version: String,
    pub legacy: bool,
}

impl Default for VersionState {
    fn default() -> Self {
        Self {
            version: api_version().to_string(),
            legacy: legacy_enabled(),
        }
    }
}

async fn version_header_middleware(
    State(state): State<VersionState>,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    let path = request.uri().path().to_string();
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(
        "X-API-Version",
        HeaderValue::from_str(&state.version).unwrap_or_else(|_| HeaderValue::from_static("v1")),
    );
    if state.legacy && !path.starts_with(&format!("/api/{}/", state.version)) {
        headers.insert("X-API-Deprecated", HeaderValue::from_static("true"));
    }
    response
}

async fn openapi_json_handler() -> impl IntoResponse {
    let openapi = ApiDoc::openapi();
    let json = serde_json::to_value(&openapi).unwrap_or(json!({}));
    (StatusCode::OK, axum::Json(json)).into_response()
}

async fn openapi_yaml_handler() -> impl IntoResponse {
    let openapi = ApiDoc::openapi();
    let yaml = serde_yaml::to_string(&openapi).unwrap_or_default();
    (StatusCode::OK, [("Content-Type", "application/yaml")], yaml).into_response()
}

async fn health_check_v1(State(db): State<Database>) -> impl IntoResponse {
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

    let response = crate::api::HealthResponse {
        status: if db_status == "connected" {
            "healthy".to_string()
        } else {
            "degraded".to_string()
        },
        version: env!("CARGO_PKG_VERSION").to_string(),
        timestamp: now,
        uptime_seconds: crate::api::uptime_seconds(),
        database: db_status,
        cache: cache_status,
    };
    (StatusCode::OK, axum::Json(response)).into_response()
}

pub fn build_v1_router(
    database: Database,
    auth_config: AuthMiddlewareConfig,
    security_state: SecurityState,
    version_state: VersionState,
) -> Router {
    // Public routes (no auth required)
    let public_router = Router::new()
        .route("/health", get(health_check_v1))
        .route("/openapi.json", get(openapi_json_handler))
        .route("/openapi.yaml", get(openapi_yaml_handler))
        .route("/monitores/hl7", post(crate::api::monitores::hl7_ingest))
        .route("/monitores/health", get(crate::api::monitores::hl7_health))
        .route("/realtime/stream", get(crate::realtime::realtime_stream))
        .route("/realtime/ping", get(crate::realtime::realtime_ping))
        .with_state(database.clone());

    // Protected routes (auth required)
    let protected_router = Router::new()
        .route("/stats", get(crate::api::stats::get_stats))
        .route("/admin/stats", get(crate::api::admin::get_admin_stats))
        .route("/admin/camas/init", post(crate::api::admin::init_camas_api))
        .route(
            "/admin/camas",
            get(crate::api::admin::list_camas_api).post(crate::api::admin::create_cama_api),
        )
        .route(
            "/admin/camas/{id}",
            get(crate::api::admin::get_cama_api)
                .put(crate::api::admin::update_cama_api)
                .delete(crate::api::admin::delete_cama_api),
        )
        .route(
            "/admin/camas/disponibles",
            get(crate::api::admin::get_camas_disponibles),
        )
        .route(
            "/admin/check-camas",
            get(crate::api::admin::check_camas_disponibles),
        )
        .route(
            "/admin/equipos",
            get(crate::api::admin::list_equipos_api).post(crate::api::admin::create_equipo_api),
        )
        .route(
            "/admin/equipos/disponibles",
            get(crate::api::admin::get_equipos_disponibles_api),
        )
        .route(
            "/admin/equipos/{id}",
            get(crate::api::admin::get_equipo_api)
                .put(crate::api::admin::update_equipo_api)
                .delete(crate::api::admin::delete_equipo_api),
        )
        .route(
            "/admin/equipos/cama/{cama_id}",
            get(crate::api::admin::list_equipos_por_cama_api),
        )
        .route(
            "/admin/equipos/asignar",
            post(crate::api::admin::asignar_equipo_cama_api),
        )
        .route(
            "/admin/equipos/{equipo_id}/desvincular",
            post(crate::api::admin::desvincular_equipo_api),
        )
        .route(
            "/admin/staff",
            get(crate::api::admin::list_staff_api).post(crate::api::admin::create_staff_api),
        )
        .route(
            "/admin/staff/{id}",
            get(crate::api::admin::get_staff_api)
                .put(crate::api::admin::update_staff_api)
                .delete(crate::api::admin::delete_staff_api),
        )
        .route(
            "/admin/staff/{id}/toggle",
            post(crate::api::admin::toggle_user_active),
        )
        .route("/admin/audit", get(crate::api::admin::get_audit_logs_api))
        .route(
            "/admin/audit/critical",
            get(crate::api::admin::get_audit_critical_api),
        )
        .route(
            "/admin/audit/cleanup",
            post(crate::api::admin::run_audit_retention_cleanup),
        )
        .route(
            "/admin/audit/seal",
            post(crate::api::admin::seal_audit_batch),
        )
        .route(
            "/admin/audit/verify",
            get(crate::api::admin::verify_audit_chain),
        )
        .route("/admin/audit/export", get(crate::api::admin::export_audit))
        .route(
            "/admin/audit/scores",
            get(crate::api::score_audit::verify_scores),
        )
        .route(
            "/admin/support/systems",
            get(crate::api::support::get_systems),
        )
        .route(
            "/admin/support/diagnostics",
            get(crate::api::support::get_diagnostics),
        )
        .route(
            "/admin/support/actions/{action}",
            post(crate::api::support::run_action),
        )
        .route(
            "/admin/support/history",
            get(crate::api::support::get_history),
        )
        .route(
            "/admin/tenants",
            get(crate::api::tenant::list_tenants_api).post(crate::api::tenant::create_tenant_api),
        )
        .route(
            "/admin/tenants/audit",
            get(crate::api::tenant::audit_tenancy_api),
        )
        .route(
            "/admin/tenants/{id}/impersonate",
            post(crate::api::tenant::impersonate_tenant_api),
        )
        .route(
            "/admin/flags",
            get(crate::api::flags::list_flags).post(crate::api::flags::create_flag),
        )
        .route(
            "/admin/flags/{key}",
            get(crate::api::flags::get_flag)
                .put(crate::api::flags::update_flag)
                .delete(crate::api::flags::delete_flag),
        )
        .merge(crate::api::ml::routes())
        .merge(crate::api::retention::routes())
        .nest("/auth", crate::api::auth::router())
        .route(
            "/patients",
            get(crate::api::patients::list_patients).post(crate::api::patients::create_patient),
        )
        .route(
            "/patients/{id}",
            get(crate::api::patients::get_patient)
                .put(crate::api::patients::update_patient)
                .delete(crate::api::patients::delete_patient),
        )
        .route(
            "/patients/{id}/egreso",
            post(crate::api::patients::egreso_paciente),
        )
        .route(
            "/patients/{id}/timeline",
            get(crate::api::patients::patient_timeline),
        )
        .route("/cds/plans", get(crate::api::cds::list_plans))
        .route("/cds/evaluate", post(crate::api::cds::evaluate_cds))
        .route(
            "/patients/{id}/measurements",
            get(crate::api::measurements::get_measurements)
                .post(crate::api::measurements::create_measurement),
        )
        .route(
            "/patients/{id}/measurements/last",
            get(crate::api::measurements::get_last_measurement),
        )
        .route(
            "/patients/{id}/scales/apache",
            post(crate::api::scales::calc_apache),
        )
        .route(
            "/patients/{id}/scales/gcs",
            post(crate::api::scales::calc_gcs),
        )
        .route(
            "/patients/{id}/scales/news2",
            post(crate::api::scales::calc_news2),
        )
        .route(
            "/patients/{id}/scales/sofa",
            post(crate::api::scales::calc_sofa),
        )
        .route(
            "/patients/{id}/scales/saps3",
            post(crate::api::scales::calc_saps3),
        )
        .route(
            "/patients/{id}/scales/history",
            get(crate::api::scales::scale_history),
        )
        .route(
            "/patients/{id}/export/csv",
            get(crate::api::export::export_csv),
        )
        .route(
            "/patients/{id}/export/pdf",
            get(crate::api::export::export_pdf),
        )
        .route(
            "/admin/institucion",
            get(crate::api::institucion::get_institucion)
                .put(crate::api::institucion::upsert_institucion),
        )
        .route(
            "/diagnosticos",
            get(crate::api::diagnosticos::list_diagnosticos),
        )
        .route(
            "/diagnosticos/search",
            get(crate::api::diagnosticos::search_diagnosticos),
        )
        .route(
            "/sandbox/generate",
            post(crate::api::sandbox::generate_patients),
        )
        .route("/sandbox/clear", post(crate::api::sandbox::clear_sandbox))
        .route(
            "/devices/status",
            get(crate::api::registry::device_status_summary),
        )
        .route(
            "/devices",
            get(crate::api::registry::list_devices).post(crate::api::registry::register_device),
        )
        .route("/devices/{id}", get(crate::api::registry::get_device))
        .route(
            "/devices/{id}/heartbeat",
            post(crate::api::registry::heartbeat_device),
        )
        .route(
            "/data-quality/validate",
            post(crate::api::quality::validate_message),
        )
        .route(
            "/data-quality/report",
            get(crate::api::quality::quality_report),
        )
        .route(
            "/data-quality/summary",
            get(crate::api::quality::quality_summary),
        )
        .route(
            "/escalation/policies",
            get(crate::api::escalation::list_policies).post(crate::api::escalation::set_policy),
        )
        .route(
            "/escalation/active",
            get(crate::api::escalation::active_escalations),
        )
        .route(
            "/escalation/{id}/ack",
            post(crate::api::escalation::acknowledge_alert),
        )
        .route(
            "/escalation/{id}/escalate",
            post(crate::api::escalation::escalate_alert),
        )
        .route(
            "/push/vapid",
            get(crate::api::push::vapid_public_key),
        )
        .route(
            "/push/subscribe",
            post(crate::api::push::subscribe),
        )
        .route(
            "/push/unsubscribe",
            axum::routing::delete(crate::api::push::unsubscribe),
        )
        .route("/push/test", post(crate::api::push::send_test))
        .route(
            "/teleicu/sessions",
            get(crate::api::teleicu::list_sessions).post(crate::api::teleicu::start_session),
        )
        .route(
            "/teleicu/sessions/{id}/end",
            post(crate::api::teleicu::end_session),
        )
        .route("/teleicu/live/{id}", get(crate::api::teleicu::live_view))
        .route("/fhir/Patient", get(crate::api::fhir::fhir_patient_search))
        .route(
            "/fhir/Patient/{id}",
            get(crate::api::fhir::fhir_patient_get),
        )
        .route(
            "/fhir/Patient/{id}/Observation",
            get(crate::api::fhir::fhir_observation_list),
        )
        .route(
            "/fhir/Patient/{id}/Condition",
            get(crate::api::fhir::fhir_condition_list),
        )
        .route(
            "/fhir/Patient/{id}/DiagnosticReport",
            get(crate::api::fhir::fhir_diagnostic_report),
        )
        .route(
            "/fhir/Patient/{id}/DiagnosticReport/QR",
            get(crate::api::fhir::fhir_diagnostic_report_qr),
        )
        .with_state(database.clone());

    // Apply security middleware only to protected routes
    let protected_router = protected_router
        .layer(middleware::from_fn_with_state(
            security_state.clone(),
            login_throttle_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            security_state.clone(),
            rate_limit_middleware,
        ))
        .layer(middleware::from_fn_with_state(auth_config, auth_middleware));

    // Merge public and protected, apply version header to both
    Router::new()
        .merge(public_router)
        .merge(protected_router)
        .layer(middleware::from_fn_with_state(
            version_state.clone(),
            version_header_middleware,
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static("x-api-version"),
            HeaderValue::from_static(api_version()),
        ))
}

pub fn build_legacy_router(
    database: Database,
    auth_config: AuthMiddlewareConfig,
    security_state: SecurityState,
    version_state: VersionState,
) -> Option<Router> {
    if !version_state.legacy {
        return None;
    }

    let v1_router = build_v1_router(database, auth_config, security_state, version_state.clone());
    Some(
        Router::new()
            .merge(v1_router)
            .layer(middleware::from_fn_with_state(
                version_state,
                version_header_middleware,
            )),
    )
}

pub fn build_unified_router(
    database: Database,
    auth_config: AuthMiddlewareConfig,
    security_state: SecurityState,
) -> Router {
    let version_state = VersionState::default();
    let v1_router = build_v1_router(
        database.clone(),
        auth_config.clone(),
        security_state.clone(),
        version_state.clone(),
    );

    // v1 routes under /v1, legacy at root (main.rs will nest under /api)
    let mut router = Router::new().nest(&format!("/{}", version_state.version), v1_router);

    if let Some(legacy) = build_legacy_router(database, auth_config, security_state, version_state)
    {
        router = router.merge(legacy);
    }

    router
}
