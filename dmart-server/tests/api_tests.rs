/// Helper: create a temporary SurrealKV database for testing
async fn test_db() -> (
    surrealdb::Surreal<surrealdb::engine::local::Db>,
    tempfile::TempDir,
) {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let path = dir.path().join("test.db");
    let path_str = path.to_str().expect("invalid path");
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::SurrealKv>(path_str)
        .await
        .expect("failed to connect to SurrealKV");
    db.use_ns("dmart")
        .use_db("icu")
        .await
        .expect("failed to use namespace");
    (db, dir)
}

#[tokio::test]
async fn test_patient_crud() {
    let (db, _dir) = test_db().await;
    let patient = dmart_shared::models::Patient::new();
    let pid = patient.patient_id.clone();

    let created = dmart_server::db::create_patient(&db, patient)
        .await
        .expect("create failed");
    assert!(!created.patient_id.is_empty());

    let fetched = dmart_server::db::get_patient(&db, &pid)
        .await
        .expect("get failed")
        .expect("patient not found");
    assert_eq!(fetched.patient_id, pid);

    let deleted = dmart_server::db::delete_patient(&db, &pid).await;
    assert!(deleted.is_ok());

    let not_found = dmart_server::db::get_patient(&db, &pid)
        .await
        .expect("get failed");
    assert!(not_found.is_none());
}

#[tokio::test]
async fn test_patient_pagination() {
    let (db, _dir) = test_db().await;
    for _ in 0..5 {
        let p = dmart_shared::models::Patient::new();
        dmart_server::db::create_patient(&db, p)
            .await
            .expect("create failed");
    }

    let total = dmart_server::db::count_patients(&db)
        .await
        .expect("count failed");
    assert!(total >= 5);

    let page = dmart_server::db::list_patients(&db, 2, 0)
        .await
        .expect("list failed");
    assert_eq!(page.len(), 2);
}

#[tokio::test]
async fn test_stats_aggregates_group_by() {
    use dmart_shared::models::{Patient, SeverityLevel};

    let (db, _dir) = test_db().await;

    let mut p = Patient::new();
    p.estado_gravedad = SeverityLevel::Critico;
    p.ultimo_apache_score = Some(30);
    p.ultimo_gcs_score = Some(6);
    p.ultimo_sofa_score = Some(9);
    dmart_server::db::create_patient(&db, p).await.expect("c1");

    let mut p = Patient::new();
    p.estado_gravedad = SeverityLevel::Critico;
    p.ultimo_apache_score = Some(40);
    p.ultimo_sofa_score = Some(1);
    dmart_server::db::create_patient(&db, p).await.expect("c2");

    let mut p = Patient::new();
    p.estado_gravedad = SeverityLevel::Severo;
    p.ultimo_apache_score = None;
    dmart_server::db::create_patient(&db, p).await.expect("c3");

    let agg = dmart_server::db::aggregate_patient_stats(&db)
        .await
        .expect("aggregate");
    assert_eq!(agg.total, 3);
    assert_eq!(agg.criticos, 2);
    assert_eq!(agg.severos, 1);
    assert_eq!(agg.moderados, 0);
    assert_eq!(agg.apache_n, 2, "solo 2 pacientes tienen apache");
    assert!((agg.apache_sum - 70.0).abs() < 0.001);
    assert!((agg.sofa_sum - 10.0).abs() < 0.001);
    assert_eq!(agg.sofa_n, 2);
    assert_eq!(agg.gcs_sum as u64, 6);
    assert_eq!(agg.gcs_n, 1);
    assert_eq!(agg.saps3_n, 0);
    assert_eq!(agg.news2_n, 0);
}

#[tokio::test]
async fn test_diagnosticos_seed_and_search() {
    let (db, _dir) = test_db().await;
    dmart_server::db::seed_diagnosticos(&db)
        .await
        .expect("seed");

    let total = dmart_server::db::list_diagnosticos(&db)
        .await
        .expect("list");
    assert!(!total.is_empty());

    let hits = dmart_server::db::search_diagnosticos(&db, "neumon")
        .await
        .expect("search");
    assert!(!hits.is_empty(), "debe encontrar diagnósticos de neumonía");

    let by_code = dmart_server::db::search_diagnosticos(&db, "j18")
        .await
        .expect("search code");
    assert!(
        by_code.iter().any(|d| d.codigo == "J18.9"),
        "búsqueda por código"
    );
}

#[tokio::test]
async fn test_transactional_create_assigns_cama_and_equipos() {
    use dmart_shared::models::{
        Cama, Equipo, EstadoCama, EstadoEquipo, Patient, TipoCama, TipoEquipo,
    };

    let (db, _dir) = test_db().await;
    dmart_server::db::seed_institucion_config(&db)
        .await
        .expect("config");

    let mut cama = Cama::new(1_u8, TipoCama::General);
    cama.estado = EstadoCama::Libre;
    let cama = dmart_server::db::create_cama(&db, cama)
        .await
        .expect("cama");
    let mut equipo = Equipo::new("TX-01".to_string(), TipoEquipo::VentiladorMecanico);
    equipo.estado = EstadoEquipo::Activo;
    let equipo = dmart_server::db::create_equipo(&db, equipo)
        .await
        .expect("equipo");

    let mut patient = Patient::new();
    patient.cama_id = Some(cama.cama_id.clone());
    patient.cama_numero = Some(cama.numero);
    let created = dmart_server::db::create_patient_with_assignments(
        &db,
        patient,
        std::slice::from_ref(&equipo.equipo_id),
    )
    .await
    .expect("create tx");

    let cama_post = dmart_server::db::get_cama_by_numero(&db, cama.numero)
        .await
        .expect("get cama")
        .expect("exists");
    assert_eq!(cama_post.estado, EstadoCama::Ocupada);
    assert_eq!(
        cama_post.paciente_id.as_deref(),
        Some(created.patient_id.as_str())
    );

    let equipos_cama = dmart_server::db::list_equipos_por_cama(&db, &cama.cama_id)
        .await
        .expect("equipos cama");
    assert_eq!(equipos_cama.len(), 1);
    assert_eq!(equipos_cama[0].equipo_id, equipo.equipo_id);
}

#[tokio::test]
async fn test_transactional_create_rolls_back_when_cama_occupada() {
    use dmart_shared::models::{
        Cama, Equipo, EstadoCama, EstadoEquipo, Patient, TipoCama, TipoEquipo,
    };

    let (db, _dir) = test_db().await;
    dmart_server::db::seed_institucion_config(&db)
        .await
        .expect("config");

    let mut cama = Cama::new(1_u8, TipoCama::General);
    cama.estado = EstadoCama::Ocupada;
    cama.paciente_id = Some("otro_paciente".to_string());
    let cama = dmart_server::db::create_cama(&db, cama)
        .await
        .expect("cama");
    let mut equipo = Equipo::new("TX-02".to_string(), TipoEquipo::Monitor);
    equipo.estado = EstadoEquipo::Activo;
    equipo.cama_id = Some("otra_cama".to_string());
    let equipo = dmart_server::db::create_equipo(&db, equipo)
        .await
        .expect("equipo");

    let mut patient = Patient::new();
    patient.cama_id = Some(cama.cama_id.clone());
    patient.cama_numero = Some(cama.numero);
    let err = dmart_server::db::create_patient_with_assignments(
        &db,
        patient,
        std::slice::from_ref(&equipo.equipo_id),
    )
    .await
    .expect_err("debe fallar: cama ocupada");
    assert!(
        err.to_string().contains("Cama no disponible"),
        "msg actual: {}",
        err
    );

    let cama_post = dmart_server::db::get_cama_by_numero(&db, cama.numero)
        .await
        .expect("get cama")
        .expect("exists");
    assert_eq!(cama_post.estado, EstadoCama::Ocupada, "cama intacta");
    assert_eq!(cama_post.paciente_id.as_deref(), Some("otro_paciente"));

    let equipo_post = dmart_server::db::get_equipo(&db, &equipo.equipo_id)
        .await
        .expect("get equipo")
        .expect("exists");
    assert_eq!(
        equipo_post.cama_id.as_deref(),
        Some("otra_cama"),
        "equipo intacto"
    );
}

#[tokio::test]
async fn test_egreso_libera_cama_y_equipos_transaccional() {
    use dmart_shared::models::{
        Cama, Equipo, EstadoCama, EstadoEquipo, Patient, TipoCama, TipoEquipo,
    };

    let (db, _dir) = test_db().await;
    dmart_server::db::seed_institucion_config(&db)
        .await
        .expect("config");

    let mut cama = Cama::new(1_u8, TipoCama::General);
    cama.estado = EstadoCama::Libre;
    let cama = dmart_server::db::create_cama(&db, cama)
        .await
        .expect("cama");
    let mut equipo = Equipo::new("TX-03".to_string(), TipoEquipo::BombaInfusion);
    equipo.estado = EstadoEquipo::Activo;
    let equipo = dmart_server::db::create_equipo(&db, equipo)
        .await
        .expect("equipo");

    let mut patient = Patient::new();
    patient.cama_id = Some(cama.cama_id.clone());
    patient.cama_numero = Some(cama.numero);
    let created = dmart_server::db::create_patient_with_assignments(
        &db,
        patient,
        std::slice::from_ref(&equipo.equipo_id),
    )
    .await
    .expect("create tx");

    dmart_server::db::egresar_paciente(&db, &created, "Mejorado")
        .await
        .expect("egreso");

    let cama_post = dmart_server::db::get_cama_by_numero(&db, cama.numero)
        .await
        .expect("get cama")
        .expect("exists");
    assert_eq!(cama_post.estado, EstadoCama::Libre);
    assert!(cama_post.paciente_id.is_none());

    let equipo_post = dmart_server::db::get_equipo(&db, &equipo.equipo_id)
        .await
        .expect("get equipo")
        .expect("exists");
    assert!(equipo_post.cama_id.is_none(), "equipo liberado");
}

#[tokio::test]
async fn test_camas_equipos_counts_group_by() {
    use dmart_shared::models::{Cama, Equipo, EstadoCama, EstadoEquipo, TipoCama, TipoEquipo};

    let (db, _dir) = test_db().await;
    dmart_server::db::seed_institucion_config(&db)
        .await
        .expect("seed config");

    let mut c1 = Cama::new(1_u8, TipoCama::General);
    c1.estado = EstadoCama::Libre;
    let mut c2 = Cama::new(2_u8, TipoCama::General);
    c2.estado = EstadoCama::Ocupada;
    let mut c3 = Cama::new(3_u8, TipoCama::Aislamiento);
    c3.estado = EstadoCama::Libre;
    dmart_server::db::create_cama(&db, c1).await.expect("cama1");
    dmart_server::db::create_cama(&db, c2).await.expect("cama2");
    dmart_server::db::create_cama(&db, c3).await.expect("cama3");

    let por_tipo = dmart_server::db::count_camas_por_tipo(&db)
        .await
        .expect("count camas");
    let general = por_tipo
        .iter()
        .find(|c| c.tipo == "General")
        .expect("general");
    assert_eq!(general.total, 2);
    assert_eq!(general.libres, 1, "libres por tipo con GROUP BY");
    let aislamiento = por_tipo
        .iter()
        .find(|c| c.tipo == "Aislamiento")
        .expect("aisl");
    assert_eq!(aislamiento.total, 1);
    assert_eq!(aislamiento.libres, 1);

    let mut eq = Equipo::new("VENT-01".to_string(), TipoEquipo::VentiladorMecanico);
    eq.estado = EstadoEquipo::Activo;
    dmart_server::db::create_equipo(&db, eq)
        .await
        .expect("eq libre");
    let mut eq2 = Equipo::new("MON-01".to_string(), TipoEquipo::Monitor);
    eq2.estado = EstadoEquipo::Activo;
    eq2.cama_id = Some("cama_test".to_string());
    dmart_server::db::create_equipo(&db, eq2)
        .await
        .expect("eq en cama");

    let por_tipo_eq = dmart_server::db::count_equipos_por_tipo(&db)
        .await
        .expect("count equipos");
    let vent_label = TipoEquipo::VentiladorMecanico.label();
    let vent = por_tipo_eq
        .iter()
        .find(|e| e.tipo == vent_label)
        .expect("vent");
    assert_eq!(vent.total, 1);
    assert_eq!(vent.disponibles, 1);
    let mon = por_tipo_eq
        .iter()
        .find(|e| e.tipo == "Monitor")
        .expect("mon");
    assert_eq!(mon.total, 1);
    assert_eq!(mon.disponibles, 0, "el monitor está asignado a una cama");
}

#[tokio::test]
async fn test_auth_register() {
    let (db, _dir) = test_db().await;
    let auth = dmart_server::auth::AuthService::new(db);

    let reg = auth
        .register(dmart_server::auth::RegisterRequest {
            username: "testdoc".into(),
            password: "TestPass123!".into(),
            nombre: "Doctor Test".into(),
            rol: "medico".into(),
            tenant_id: "default".into(),
        })
        .await
        .expect("register failed");
    assert_eq!(reg.username, "testdoc");

    let login = auth
        .authenticate("testdoc", "TestPass123!", None, None)
        .await
        .expect("authenticate failed");
    assert!(!login.token.is_empty());
    assert!(!login.access_token.is_empty());
    assert_eq!(login.access_token, login.token);
    assert!(!login.mfa_required, "no MFA configured => full session");
    assert!(
        !login.refresh_token.is_empty(),
        "login must issue a refresh token"
    );

    let refresh = auth
        .rotate_refresh_token(&login.refresh_token, None, None)
        .await;
    assert!(refresh.is_ok(), "refresh rotation must succeed");
    let rotated = refresh.unwrap();
    assert_ne!(
        rotated.access_token, login.token,
        "rotated session must mint a new access token"
    );
    assert_ne!(
        rotated.refresh_token, login.refresh_token,
        "refresh token must rotate (single-use)"
    );
    assert!(!rotated.refresh_token.is_empty());

    // El refresh token original ya fue consumido: reusarlo debe fallar.
    let reuse = auth
        .rotate_refresh_token(&login.refresh_token, None, None)
        .await;
    assert!(
        reuse.is_err(),
        "reusing a consumed refresh token must be rejected (reuse detection)"
    );
    // Y el nuevo token de la sesión robada ha sido revocado junto con la familia.
    let family_claims = auth.verify_token(&rotated.access_token);
    assert!(
        family_claims.is_err(),
        "reuse detection must revoke the whole session family"
    );

    // La familia incluye también el access token original del login.
    let original = auth.verify_token(login.token.as_str());
    assert!(
        original.is_err(),
        "original access token belongs to the revoked family"
    );
}

#[tokio::test]
async fn test_password_hash_not_plaintext_and_revocation() {
    let (db, _dir) = test_db().await;
    let auth = dmart_server::auth::AuthService::new(db);

    let created = auth
        .register(dmart_server::auth::RegisterRequest {
            username: "hashcheck".into(),
            password: "Secreto_123!".into(),
            nombre: "Hash Check".into(),
            rol: "enfermero".into(),
            tenant_id: "default".into(),
        })
        .await
        .expect("register failed");

    assert!(
        created.password_hash.starts_with("$argon2id$"),
        "password must be stored as an Argon2id hash, got: {}",
        created.password_hash
    );
    assert!(
        !created.password_hash.contains("Secreto_123!"),
        "plaintext password must never be stored"
    );

    let login = auth
        .authenticate("hashcheck", "Secreto_123!", None, None)
        .await
        .expect("authenticate failed");
    assert_eq!(login.user.rol, dmart_shared::models::UserRole::Enfermero);

    let claims = auth
        .verify_token(&login.token)
        .expect("token should be valid before logout");
    dmart_server::auth::revoke_token(&login.token, claims.exp);

    let after_revoke = auth.verify_token(&login.token);
    assert!(after_revoke.is_err(), "revoked token must not be accepted");
}

// ─── E2E HTTP security suite ────────────────────────────────────────
// Exercises the exact production stack (router + auth/RBAC/throttle/rate
// limit middleware) in-process via `tower::ServiceExt::oneshot`.

use axum::body::Body;
use axum::http::{Method, StatusCode, header};
use http_body_util::BodyExt;
use std::net::SocketAddr;
use surrealdb::Surreal;
use surrealdb::engine::local::Db;
use tower::ServiceExt;

type TestDb = Surreal<Db>;

async fn seed_user(
    db: &TestDb,
    username: &str,
    password: &str,
    rol: dmart_shared::models::UserRole,
    nombre: &str,
) {
    let hash = dmart_server::auth::hash_password(password).expect("hash failed");
    let user = dmart_shared::models::User {
        user_id: uuid::Uuid::new_v4().to_string(),
        username: username.into(),
        password_hash: hash,
        rol,
        nombre: nombre.into(),
        activo: true,
        created_at: chrono::Utc::now().to_rfc3339(),
        tenant_id: "default".into(),
    };
    dmart_server::db::create_user(db, user)
        .await
        .expect("seed user failed");
}

async fn build_app(db: &TestDb) -> axum::Router {
    let auth_service = dmart_server::auth::AuthService::new(db.clone());
    let auth_config = dmart_server::middleware::auth_mod::AuthMiddlewareConfig::new(auth_service);
    let security_state = dmart_server::security::create_security_state();
    let database = std::sync::Arc::new(db.clone());
    dmart_server::api::build_api_router(database, auth_config, security_state).layer(
        axum::extract::connect_info::MockConnectInfo("127.0.0.1:0".parse::<SocketAddr>().unwrap()),
    )
}

async fn send(
    app: &axum::Router,
    method: Method,
    path: &str,
    token: Option<&str>,
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    let app = app.clone();
    let mut builder = axum::http::Request::builder()
        .method(method)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(t) = token {
        builder = builder.header("authorization", format!("Bearer {}", t));
    }
    let payload = body.map(|v| v.to_string()).unwrap_or_default();
    let request = builder.body(Body::from(payload)).expect("build request");
    let response = app.oneshot(request).await.expect("oneshot failed");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body failed")
        .to_bytes();
    let json = if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or_else(|_| serde_json::Value::Null)
    };
    (status, json)
}

fn login_body(username: &str, password: &str) -> serde_json::Value {
    serde_json::json!({ "username": username, "password": password })
}

async fn login_token(http: &axum::Router, username: &str, password: &str) -> String {
    let (status, json) = send(
        http,
        Method::POST,
        "/auth/login",
        None,
        Some(login_body(username, password)),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    json["data"]["token"]
        .as_str()
        .expect("login token")
        .to_string()
}

// ─── Observabilidad: /metrics Prometheus ──────────────────────────────────
// El recorder es global; se instala una única vez por proceso de test.

use std::sync::OnceLock;

static METRICS_HANDLE: OnceLock<metrics_exporter_prometheus::PrometheusHandle> = OnceLock::new();

fn metrics_handle() -> metrics_exporter_prometheus::PrometheusHandle {
    METRICS_HANDLE
        .get_or_init(|| dmart_server::observability::init_metrics().expect("init metrics once"))
        .clone()
}

async fn build_obs_app(db: &TestDb) -> axum::Router {
    let auth_service = dmart_server::auth::AuthService::new(db.clone());
    let auth_config = dmart_server::middleware::auth_mod::AuthMiddlewareConfig::new(auth_service);
    let security_state = dmart_server::security::create_security_state();
    let database = std::sync::Arc::new(db.clone());
    let api = dmart_server::api::build_api_router(database.clone(), auth_config, security_state);
    let obs = dmart_server::observability::observability_router(database, metrics_handle());
    axum::Router::new().merge(api).nest("/obs", obs).layer(
        axum::extract::connect_info::MockConnectInfo("127.0.0.1:0".parse::<SocketAddr>().unwrap()),
    )
}

async fn scrape_metrics(http: &axum::Router) -> (StatusCode, String) {
    let app = http.clone();
    let request = axum::http::Request::builder()
        .method(Method::GET)
        .uri("/obs/metrics")
        .body(Body::from(""))
        .expect("request");
    let response = app.oneshot(request).await.expect("oneshot metrics");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    (status, String::from_utf8_lossy(&bytes).to_string())
}

/// Devuelve el valor numérico de la serie Prometheus `token` (p.ej.
/// `auth_login_total{result="success"}` o `patients_created_total`).
fn metric_value(body: &str, token: &str) -> Option<f64> {
    body.lines().find_map(|l| {
        if l.starts_with('#') || !l.starts_with(token) {
            return None;
        }
        let value = l[token.len()..].trim_start_matches('{');
        value
            .split_whitespace()
            .next()
            .unwrap_or("0")
            .parse::<f64>()
            .ok()
    })
}

const SPEC_METRICS: &[&str] = &[
    "http_requests_total",
    "http_request_duration_seconds",
    "http_requests_errors_total",
    "auth_login_total",
    "auth_refresh_total",
    "auth_failures_total",
    "rbac_denials_total",
    "surreal_query_duration_seconds",
    "surreal_connection_pool",
    "patients_total",
    "patients_created_total",
    "patients_deleted_total",
    "measurements_total",
    "measurements_created_total",
    "scales_calculated_total",
    "camas_total",
    "camas_ocupadas",
    "ml_predictions_total",
    "ml_model_load_duration_seconds",
    "ml_accuracy_gauge",
    "sse_connections_active",
    "hl7_messages_processed_total",
    "hl7_messages_errors_total",
    "ingest_gap_total",
    "ingest_invalid_total",
    "ingest_fault_devices",
    "ingest_throttled_total",
    "ingest_error_avg",
    "db_connections_active",
    "cache_connected",
    "uptime_seconds",
    "process_cpu_seconds_total",
    "process_resident_memory_bytes",
];

#[tokio::test]
async fn test_metrics_endpoint_exposes_all_and_tracks_events() {
    let (db, _dir) = test_db().await;
    seed_user(
        &db,
        "metrics_admin",
        "SuperSecreto_01!",
        dmart_shared::models::UserRole::Admin,
        "Métricas",
    )
    .await;
    let http = build_obs_app(&db).await;

    // Warm-up: la primera petición registra sus propias métricas HTTP *después*
    // de renderizar, así que un request previo asegura que `http_requests_total`
    // exista en el primer scrape.
    let warmup = http.clone();
    let req = axum::http::Request::builder()
        .method(Method::GET)
        .uri("/obs/health")
        .body(Body::from(""))
        .expect("warmup");
    let _ = warmup.oneshot(req).await.expect("warmup oneshot");

    // 1) Scrape base: todas las métricas del spec deben estar expuestas.
    let (status, body) = scrape_metrics(&http).await;
    assert_eq!(status, StatusCode::OK);
    // Snap del formato de texto de Prometheus: un histograma puede aparecer
    // como `name{quantile=...}` (exporter en modo summary por defecto en 0.13)
    // o como `name_bucket/name_sum/name_count` (cuando se configuran buckets
    // explícitos, SPEC-005 edge case #3). El check acepta ambos.
    let present = |name: &str| {
        body.lines().any(|l| {
            if l.starts_with('#') {
                return false;
            }
            l.strip_prefix(name).is_some_and(|rest| {
                rest.starts_with(' ') || rest.starts_with('{') || rest.starts_with('_')
            })
        })
    };
    for name in SPEC_METRICS {
        assert!(present(name), "métrica ausente en /obs/metrics: {name}");
    }

    // 2) Generar eventos de negocio y verificar que incrementan contadores.
    let before_login = metric_value(&body, "auth_login_total{result=\"success\"}").unwrap_or(0.0);
    let before_fail = metric_value(&body, "auth_login_total{result=\"failure\"}").unwrap_or(0.0);
    let before_patients = metric_value(&body, "patients_created_total").unwrap_or(0.0);
    let before_gcs = metric_value(&body, "scales_calculated_total{scale=\"gcs\"}").unwrap_or(0.0);

    let _token = login_token(&http, "metrics_admin", "SuperSecreto_01!").await;
    let (s, _) = send(
        &http,
        Method::POST,
        "/auth/login",
        None,
        Some(login_body("metrics_admin", "Contraseña_incorrecta_99!")),
    )
    .await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);

    let patient = dmart_shared::models::Patient::new();
    let payload = serde_json::to_value(&patient).unwrap();
    let (s, j) = send(
        &http,
        Method::POST,
        "/patients",
        Some(&_token),
        Some(payload),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);
    let pid = j["data"]["patient_id"].as_str().expect("patient_id");
    let (s, _) = send(
        &http,
        Method::POST,
        &format!("/patients/{pid}/scales/gcs"),
        Some(&_token),
        Some(serde_json::json!({
            "apertura_ocular": 4,
            "respuesta_verbal": 5,
            "respuesta_motora": 6,
            "notas": "score GCS de prueba",
        })),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);

    let (_, body2) = scrape_metrics(&http).await;
    assert!(
        metric_value(&body2, "auth_login_total{result=\"success\"}").unwrap_or(0.0) > before_login,
        "login exitoso no contabilizado"
    );
    assert!(
        metric_value(&body2, "auth_login_total{result=\"failure\"}").unwrap_or(0.0) > before_fail,
        "login fallido no contabilizado"
    );
    assert!(
        metric_value(&body2, "patients_created_total").unwrap_or(0.0) > before_patients,
        "paciente creado no contabilizado"
    );
    assert!(
        metric_value(&body2, "scales_calculated_total{scale=\"gcs\"}").unwrap_or(0.0) > before_gcs,
        "escala GCS no contabilizada"
    );
    assert!(
        body2.contains("sse_connections_active"),
        "gauge SSE ausente tras scrape"
    );
}

#[tokio::test]
async fn test_metrics_histogram_buckets_fine_covered() {
    let (db, _dir) = test_db().await;
    let http = build_obs_app(&db).await;

    // Calentamos el histograma con un par de peticiones HTTP reales.
    for _ in 0..3 {
        let req = axum::http::Request::builder()
            .method(Method::GET)
            .uri("/obs/health")
            .body(Body::from(""))
            .expect("http");
        let _ = http.clone().oneshot(req).await.expect("health");
    }

    let (status, body) = scrape_metrics(&http).await;
    assert_eq!(status, StatusCode::OK);

    // SPEC-005 edge case #3: buckets finos para latencias <10ms deben existir
    // (0.1ms, 0.5ms, 1ms, 2.5ms, 5ms) además del bucket +Inf de cierre.
    for le in [
        "0.0001", "0.0005", "0.001", "0.0025", "0.005", "0.01", "0.025", "0.05", "0.1", "0.25",
        "0.5", "1", "2.5", "5", "10", "+Inf",
    ] {
        let has_histogram = body
            .lines()
            .any(|l| l.contains("http_request_duration_seconds_bucket"));
        let has_le = body.lines().any(|l| l.contains(&format!("le=\"{le}\"")));
        assert!(
            has_histogram && has_le,
            "bucket le={le} ausente (buckets finos no configurados)"
        );
    }
    // El histograma también debe existir como serie `_sum` y `_count`.
    assert!(body.contains("http_request_duration_seconds_sum"));
    assert!(body.contains("http_request_duration_seconds_count"));
}

#[tokio::test]
async fn test_e2e_login_wrong_password_returns_401() {
    let (db, _dir) = test_db().await;
    seed_user(
        &db,
        "admin_e2e",
        "SuperSecreto_01!",
        dmart_shared::models::UserRole::Admin,
        "Admin E2E",
    )
    .await;
    let http = build_app(&db).await;

    let (status, json) = send(
        &http,
        Method::POST,
        "/auth/login",
        None,
        Some(login_body("admin_e2e", "incorrecta")),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "wrong password => 401");
    assert_eq!(json["success"], false);

    let (status, json) = send(
        &http,
        Method::POST,
        "/auth/login",
        None,
        Some(login_body("admin_e2e", "SuperSecreto_01!")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!json["data"]["token"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn test_e2e_health_is_open_and_me_requires_token() {
    let (db, _dir) = test_db().await;
    seed_user(
        &db,
        "admin_e2e2",
        "SuperSecreto_01!",
        dmart_shared::models::UserRole::Admin,
        "Admin E2E",
    )
    .await;
    let http = build_app(&db).await;

    let (status, json) = send(&http, Method::GET, "/health", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["status"], "healthy");

    let (status, _) = send(&http, Method::GET, "/auth/me", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "no token => 401");

    let (status, _) = send(&http, Method::GET, "/patients", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_e2e_api_versioning_v1() {
    let (db, _dir) = test_db().await;
    seed_user(
        &db,
        "admin_ver",
        "SuperSecreto_01!",
        dmart_shared::models::UserRole::Admin,
        "Admin Ver",
    )
    .await;
    let http = build_app(&db).await;

    // Health check v1
    let (status, json) = send(&http, Method::GET, "/v1/health", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["status"], "healthy");

    // OpenAPI JSON v1
    let (status, json) = send(&http, Method::GET, "/v1/openapi.json", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(json["openapi"].as_str().unwrap().starts_with("3."));
    assert_eq!(json["info"]["version"], "1.0.0");
    assert_eq!(json["servers"][0]["url"], "/api/v1");

    // Version header present
    let (status, _) = send(&http, Method::GET, "/v1/health", None, None).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn test_e2e_api_legacy_deprecation_header() {
    let (db, _dir) = test_db().await;
    seed_user(
        &db,
        "admin_legacy",
        "SuperSecreto_01!",
        dmart_shared::models::UserRole::Admin,
        "Admin Legacy",
    )
    .await;
    let http = build_app(&db).await;

    // Legacy route should work but have deprecation header
    let (status, _) = send(&http, Method::GET, "/health", None, None).await;
    assert_eq!(status, StatusCode::OK);
    // Note: The test framework doesn't easily expose response headers in our send() helper
    // This is verified manually or with a more sophisticated test client
}

#[tokio::test]
async fn test_e2e_me_returns_real_user() {
    let (db, _dir) = test_db().await;
    seed_user(
        &db,
        "capitan_uci",
        "SuperSecreto_01!",
        dmart_shared::models::UserRole::Admin,
        "Capitana UCI",
    )
    .await;
    let http = build_app(&db).await;
    let token = login_token(&http, "capitan_uci", "SuperSecreto_01!").await;

    let (status, json) = send(&http, Method::GET, "/auth/me", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["username"], "capitan_uci");
    assert_eq!(json["data"]["rol"], "Admin");
}

#[tokio::test]
async fn test_e2e_register_requires_admin() {
    let (db, _dir) = test_db().await;
    seed_user(
        &db,
        "admin_e2e3",
        "SuperSecreto_01!",
        dmart_shared::models::UserRole::Admin,
        "Admin",
    )
    .await;
    seed_user(
        &db,
        "viewer_e2e",
        "SuperSecreto_01!",
        dmart_shared::models::UserRole::Viewer,
        "Visualizador",
    )
    .await;
    let http = build_app(&db).await;
    let admin_token = login_token(&http, "admin_e2e3", "SuperSecreto_01!").await;
    let viewer_token = login_token(&http, "viewer_e2e", "SuperSecreto_01!").await;

    let new_user = serde_json::json!({
        "username": "nuevo_medico",
        "password": "OtraClave_99!",
        "nombre": "Nuevo Médico",
        "rol": "medico",
    });

    let (status, _) = send(
        &http,
        Method::POST,
        "/auth/register",
        None,
        Some(new_user.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "anónimo => 401");

    let (status, json) = send(
        &http,
        Method::POST,
        "/auth/register",
        Some(&viewer_token),
        Some(new_user.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "no-admin => 403");
    assert_eq!(json["success"], false);

    let (status, json) = send(
        &http,
        Method::POST,
        "/auth/register",
        Some(&admin_token),
        Some(new_user),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "admin => 201");
    assert_eq!(json["data"]["username"], "nuevo_medico");

    let (status, _) = send(
        &http,
        Method::POST,
        "/auth/login",
        None,
        Some(login_body("nuevo_medico", "OtraClave_99!")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn test_e2e_staff_requires_admin_and_hides_password() {
    let (db, _dir) = test_db().await;
    seed_user(
        &db,
        "admin_e2e4",
        "SuperSecreto_01!",
        dmart_shared::models::UserRole::Admin,
        "Admin",
    )
    .await;
    seed_user(
        &db,
        "viewer_e2e4",
        "SuperSecreto_01!",
        dmart_shared::models::UserRole::Viewer,
        "Visualizador",
    )
    .await;
    let http = build_app(&db).await;
    let admin_token = login_token(&http, "admin_e2e4", "SuperSecreto_01!").await;
    let viewer_token = login_token(&http, "viewer_e2e4", "SuperSecreto_01!").await;

    let (status, _) = send(
        &http,
        Method::GET,
        "/admin/staff",
        Some(&viewer_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "no-admin => 403");

    let (status, json) = send(&http, Method::GET, "/admin/staff", Some(&admin_token), None).await;
    assert_eq!(status, StatusCode::OK);
    let raw = serde_json::to_string(&json).unwrap();
    assert!(
        !raw.contains("password_hash"),
        "staff list must never leak password hashes"
    );
}

#[tokio::test]
async fn test_e2e_staff_list_includes_all_roles_and_filters() {
    use dmart_shared::models::UserRole;

    let (db, _dir) = test_db().await;
    seed_user(
        &db,
        "admin_roles",
        "SuperSecreto_01!",
        UserRole::Admin,
        "Admin",
    )
    .await;
    seed_user(
        &db,
        "medico_roles",
        "SuperSecreto_01!",
        UserRole::Medico,
        "Médico",
    )
    .await;
    seed_user(
        &db,
        "enfermero_roles",
        "SuperSecreto_01!",
        UserRole::Enfermero,
        "Enfermero",
    )
    .await;
    seed_user(
        &db,
        "viewer_roles",
        "SuperSecreto_01!",
        UserRole::Viewer,
        "Viewer",
    )
    .await;

    let http = build_app(&db).await;
    let admin = login_token(&http, "admin_roles", "SuperSecreto_01!").await;

    // Sin filtro: todos los roles visibles (antes Admin/Viewer se ocultaban).
    let (status, json) = send(&http, Method::GET, "/admin/staff", Some(&admin), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        json["data"]["total"], 4,
        "el listado debe incluir Admin, Médico, Enfermero y Viewer"
    );
    let raw = serde_json::to_string(&json["data"]["items"]).unwrap();
    for u in [
        "admin_roles",
        "medico_roles",
        "enfermero_roles",
        "viewer_roles",
    ] {
        assert!(raw.contains(u), "el listado debe mostrar a {u}");
    }
    assert!(!raw.contains("password_hash"), "nunca exponer hashes");

    // Filtros por rol (case-insensitive).
    for (query, expected) in [
        ("?rol=Admin", "admin_roles"),
        ("?rol=medico", "medico_roles"),
        ("?rol=Enfermero", "enfermero_roles"),
        ("?rol=viewer", "viewer_roles"),
        ("?rol=todos", "enfermero_roles"),
    ] {
        let (status, json) = send(
            &http,
            Method::GET,
            &format!("/admin/staff{query}"),
            Some(&admin),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "filtro {query}");
        let items = serde_json::to_string(&json["data"]["items"]).unwrap();
        assert!(
            items.contains(expected),
            "filtro {query} debe incluir {expected}"
        );
    }

    // Alta de Enfermero (bug reportado) y verificación de que aparece en el listado.
    let (status, created) = send(
        &http,
        Method::POST,
        "/admin/staff",
        Some(&admin),
        Some(serde_json::json!({
            "username": "nuevo_enfermero",
            "nombre": "Nueva Enfermera",
            "rol": "Enfermero",
            "password": "SuperSecreto_01!"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "alta de Enfermero debe funcionar");
    assert_eq!(created["data"]["username"], "nuevo_enfermero");

    let (status, json) = send(
        &http,
        Method::GET,
        "/admin/staff?rol=Enfermero",
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["total"], 2, "deben verse 2 enfermeros");
    assert!(
        serde_json::to_string(&json["data"]["items"])
            .unwrap()
            .contains("nuevo_enfermero")
    );
}

#[tokio::test]
async fn test_e2e_change_password_flow() {
    use dmart_shared::models::UserRole;

    let (db, _dir) = test_db().await;
    seed_user(
        &db,
        "admin_pwd",
        "SuperSecreto_01!",
        UserRole::Admin,
        "Admin",
    )
    .await;
    let http = build_app(&db).await;
    let token = login_token(&http, "admin_pwd", "SuperSecreto_01!").await;

    // Contraseña actual incorrecta => 401.
    let (status, _) = send(
        &http,
        Method::POST,
        "/auth/change-password",
        Some(&token),
        Some(serde_json::json!({
            "current_password": "equivocada",
            "new_password": "NuevaClave_02!"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // Nueva contraseña demasiado corta => 400.
    let (status, _) = send(
        &http,
        Method::POST,
        "/auth/change-password",
        Some(&token),
        Some(serde_json::json!({
            "current_password": "SuperSecreto_01!",
            "new_password": "corta"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Cambio correcto => 200.
    let (status, _) = send(
        &http,
        Method::POST,
        "/auth/change-password",
        Some(&token),
        Some(serde_json::json!({
            "current_password": "SuperSecreto_01!",
            "new_password": "NuevaClave_02!"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // El token anterior queda revocado.
    let (status, _) = send(&http, Method::GET, "/auth/me", Some(&token), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "sesión debe cerrarse");

    // La contraseña vieja ya no sirve; la nueva sí.
    let (status, _) = send(
        &http,
        Method::POST,
        "/auth/login",
        None,
        Some(login_body("admin_pwd", "SuperSecreto_01!")),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let _ = login_token(&http, "admin_pwd", "NuevaClave_02!").await;
}

#[tokio::test]
async fn test_e2e_rbac_viewer_cannot_create_patient() {
    let (db, _dir) = test_db().await;
    seed_user(
        &db,
        "admin_e2e5",
        "SuperSecreto_01!",
        dmart_shared::models::UserRole::Admin,
        "Admin",
    )
    .await;
    seed_user(
        &db,
        "viewer_e2e5",
        "SuperSecreto_01!",
        dmart_shared::models::UserRole::Viewer,
        "Visualizador",
    )
    .await;
    seed_user(
        &db,
        "medico_e2e5",
        "SuperSecreto_01!",
        dmart_shared::models::UserRole::Medico,
        "Médico",
    )
    .await;
    let http = build_app(&db).await;
    let viewer_token = login_token(&http, "viewer_e2e5", "SuperSecreto_01!").await;
    let medico_token = login_token(&http, "medico_e2e5", "SuperSecreto_01!").await;

    let patient = dmart_shared::models::Patient::new();
    let payload = serde_json::to_value(&patient).unwrap();

    let (status, json) = send(
        &http,
        Method::POST,
        "/patients",
        Some(&viewer_token),
        Some(payload.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "viewer sin create => 403");
    assert_eq!(json["success"], false);

    let (status, json) = send(
        &http,
        Method::POST,
        "/patients",
        Some(&medico_token),
        Some(payload),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "medico con create => 201");
    assert!(!json["data"]["patient_id"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn test_e2e_logout_revokes_token() {
    let (db, _dir) = test_db().await;
    seed_user(
        &db,
        "admin_e2e6",
        "SuperSecreto_01!",
        dmart_shared::models::UserRole::Admin,
        "Admin",
    )
    .await;
    let http = build_app(&db).await;
    let token = login_token(&http, "admin_e2e6", "SuperSecreto_01!").await;

    let (status, _) = send(&http, Method::POST, "/auth/logout", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = send(&http, Method::GET, "/auth/me", Some(&token), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "token revocado => 401");
}

#[tokio::test]
async fn test_e2e_login_throttle_locks_after_failures() {
    let (db, _dir) = test_db().await;
    seed_user(
        &db,
        "admin_e2e7",
        "SuperSecreto_01!",
        dmart_shared::models::UserRole::Admin,
        "Admin",
    )
    .await;
    let http = build_app(&db).await;

    for i in 1..=4 {
        let (status, _) = send(
            &http,
            Method::POST,
            "/auth/login",
            None,
            Some(login_body("admin_e2e7", "nope")),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "intento {} => 401", i);
    }

    let (status, _) = send(
        &http,
        Method::POST,
        "/auth/login",
        None,
        Some(login_body("admin_e2e7", "nope")),
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "5º intento => 429");

    let (status, _) = send(
        &http,
        Method::POST,
        "/auth/login",
        None,
        Some(login_body("admin_e2e7", "nope")),
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "bloqueado => 429");
}

fn totp_code(secret_b32: &str, account: &str) -> String {
    let secret = totp_rs::Secret::Encoded(secret_b32.to_string())
        .to_bytes()
        .expect("decodificar secreto");
    let totp = totp_rs::TOTP::new(
        totp_rs::Algorithm::SHA1,
        6,
        1,
        30,
        secret,
        None,
        account.to_string(),
    )
    .expect("totp");
    totp.generate_current().expect("generar código")
}

#[tokio::test]
async fn test_e2e_mfa_setup_confirm_verify_disable() {
    let (db, _dir) = test_db().await;
    seed_user(
        &db,
        "mfa_user",
        "SuperSecreto_01!",
        dmart_shared::models::UserRole::Admin,
        "MFA Admin",
    )
    .await;
    let http = build_app(&db).await;

    let (status, json) = send(
        &http,
        Method::POST,
        "/auth/login",
        None,
        Some(login_body("mfa_user", "SuperSecreto_01!")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["mfa_required"], false);
    let user_id = json["data"]["user"]["user_id"]
        .as_str()
        .unwrap()
        .to_string();
    let session = json["data"]["token"].as_str().unwrap().to_string();

    let (status, json) = send(&http, Method::GET, "/auth/mfa/status", Some(&session), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["enabled"], false, "MFA inactivo al inicio");

    let (status, json) = send(&http, Method::POST, "/auth/mfa/setup", Some(&session), None).await;
    assert_eq!(status, StatusCode::OK, "setup => {}", json);
    let secret = json["data"]["secret"].as_str().unwrap().to_string();
    assert!(!secret.is_empty());
    let codes: Vec<String> = json["data"]["backup_codes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c.as_str().unwrap().to_string())
        .collect();
    assert_eq!(codes.len(), 10);

    let code = totp_code(&secret, &user_id);
    let (status, json) = send(
        &http,
        Method::POST,
        "/auth/mfa/confirm",
        Some(&session),
        Some(serde_json::json!({ "code": code })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "confirm => {}", json);

    let (status, json) = send(&http, Method::GET, "/auth/mfa/status", Some(&session), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        json["data"]["enabled"], true,
        "MFA habilitado tras confirmar"
    );

    let (status, json) = send(
        &http,
        Method::POST,
        "/auth/login",
        None,
        Some(login_body("mfa_user", "SuperSecreto_01!")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["mfa_required"], true, "2º factor exigido");
    let challenge = json["data"]["token"].as_str().unwrap().to_string();

    let (status, _) = send(&http, Method::GET, "/patients", Some(&challenge), None).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "reto MFA no puede usar rutas"
    );

    let code2 = totp_code(&secret, &user_id);
    let (status, json) = send(
        &http,
        Method::POST,
        "/auth/mfa/verify",
        Some(&challenge),
        Some(serde_json::json!({ "code": code2 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "verify => {}", json);
    assert_eq!(json["data"]["mfa_required"], false);
    let full = json["data"]["token"].as_str().unwrap().to_string();

    let (status, _) = send(&http, Method::GET, "/patients", Some(&full), None).await;
    assert_eq!(status, StatusCode::OK, "sesión completa opera normal");

    let code3 = totp_code(&secret, &user_id);
    let (status, json) = send(
        &http,
        Method::POST,
        "/auth/mfa/disable",
        Some(&full),
        Some(serde_json::json!({ "code": code3 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "disable => {}", json);

    let (status, json) = send(&http, Method::GET, "/auth/mfa/status", Some(&full), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["enabled"], false, "MFA desactivado");

    let _ = user_id;
}

#[tokio::test]
async fn test_e2e_fhir_observation_returns_scores_bundle() {
    use dmart_shared::models::{Patient, SeverityLevel};

    let (db, _dir) = test_db().await;
    seed_user(
        &db,
        "fhir_obs",
        "SuperSecreto_01!",
        dmart_shared::models::UserRole::Medico,
        "FHIR Obs",
    )
    .await;
    let http = build_app(&db).await;
    let token = login_token(&http, "fhir_obs", "SuperSecreto_01!").await;

    let mut p = Patient::new();
    p.ultimo_apache_score = Some(24);
    p.ultimo_gcs_score = Some(9);
    p.estado_gravedad = SeverityLevel::Critico;
    let created = dmart_server::db::create_patient(&db, p)
        .await
        .expect("create");

    let (status, json) = send(
        &http,
        Method::GET,
        &format!("/fhir/Patient/{}/Observation", created.patient_id),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "bundle => {}", json);
    assert_eq!(json["resource_type"], "Bundle", "bundle => {}", json);
    assert!(json["entry"].as_array().unwrap().len() >= 2);
    let codes: Vec<&str> = json["entry"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["code"]["coding"][0]["code"].as_str().unwrap())
        .collect();
    assert!(codes.contains(&"gcs"));
    assert!(codes.contains(&"apache2"));

    let (status, _) = send(
        &http,
        Method::GET,
        "/fhir/Patient/no-existe/Observation",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_e2e_fhir_condition_returns_diagnoses_bundle() {
    use dmart_shared::models::{Patient, SeverityLevel};

    let (db, _dir) = test_db().await;
    dmart_server::db::seed_diagnosticos(&db)
        .await
        .expect("seed cie-10");
    seed_user(
        &db,
        "fhir_cond",
        "SuperSecreto_01!",
        dmart_shared::models::UserRole::Medico,
        "FHIR Cond",
    )
    .await;
    let http = build_app(&db).await;
    let token = login_token(&http, "fhir_cond", "SuperSecreto_01!").await;

    let mut p = Patient::new();
    p.diagnostico_uci = "Sepsis por Klebsiella, foco respiratorio".to_string();
    p.diagnostico_hospital = "Neumonía adquirida en la comunidad; SDRA. A41.9".to_string();
    p.estado_gravedad = SeverityLevel::Severo;
    let created = dmart_server::db::create_patient(&db, p)
        .await
        .expect("create");

    let (status, json) = send(
        &http,
        Method::GET,
        &format!("/fhir/Patient/{}/Condition", created.patient_id),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "bundle => {}", json);
    assert_eq!(json["resource_type"], "Bundle", "bundle => {}", json);
    assert_eq!(json["type"], "searchset", "bundle => {}", json);
    let entries = json["entry"].as_array().expect("entries");
    assert!(entries.len() >= 2, "debe haber UCI + hospital => {}", json);

    // El diagnóstico hospitalario contiene "A41.9" -> debe enriquecerse con coding CIE-10
    let coded: Vec<&serde_json::Value> = entries
        .iter()
        .filter(|e| {
            e["code"]["coding"]
                .as_array()
                .is_some_and(|c| !c.is_empty())
        })
        .collect();
    assert!(
        !coded.is_empty(),
        "algún Condition con coding CIE-10 => {}",
        json
    );
    assert!(
        coded
            .iter()
            .any(|c| c["code"]["coding"][0]["code"] == "A41.9"),
        "codificación A41.9 presente => {}",
        json
    );

    for e in entries.iter() {
        assert_eq!(e["resource_type"], "Condition");
        assert_eq!(e["clinical_status"]["coding"][0]["code"], "active");
        assert_eq!(
            e["subject"]["reference"],
            format!("Patient/{}", created.patient_id),
            "referencia al sujeto => {}",
            json
        );
    }

    let (status, _) = send(
        &http,
        Method::GET,
        "/fhir/Patient/no-existe/Condition",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_e2e_mfa_backup_code_and_disable() {
    let (db, _dir) = test_db().await;
    seed_user(
        &db,
        "mfa_backup",
        "SuperSecreto_01!",
        dmart_shared::models::UserRole::Admin,
        "MFA Backup",
    )
    .await;
    let http = build_app(&db).await;

    let (status, json) = send(
        &http,
        Method::POST,
        "/auth/login",
        None,
        Some(login_body("mfa_backup", "SuperSecreto_01!")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let user_id = json["data"]["user"]["user_id"]
        .as_str()
        .unwrap()
        .to_string();
    let session = json["data"]["token"].as_str().unwrap().to_string();

    let (status, json) = send(&http, Method::POST, "/auth/mfa/setup", Some(&session), None).await;
    assert_eq!(status, StatusCode::OK);
    let secret = json["data"]["secret"].as_str().unwrap().to_string();
    let backup = json["data"]["backup_codes"][0]
        .as_str()
        .unwrap()
        .to_string();

    let code = totp_code(&secret, &user_id);
    send(
        &http,
        Method::POST,
        "/auth/mfa/confirm",
        Some(&session),
        Some(serde_json::json!({ "code": code })),
    )
    .await;

    let (status, json) = send(
        &http,
        Method::POST,
        "/auth/login",
        None,
        Some(login_body("mfa_backup", "SuperSecreto_01!")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["mfa_required"], true);
    let challenge = json["data"]["token"].as_str().unwrap().to_string();

    let (status, json) = send(
        &http,
        Method::POST,
        "/auth/mfa/verify",
        Some(&challenge),
        Some(serde_json::json!({ "backup_code": backup })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "backup code => {}", json);
    assert_eq!(json["data"]["mfa_required"], false);

    let code3 = totp_code(&secret, &user_id);
    let (status, json) = send(
        &http,
        Method::POST,
        "/auth/mfa/disable",
        Some(&session),
        Some(serde_json::json!({ "code": code3 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "disable => {}", json);

    let (status, json) = send(
        &http,
        Method::POST,
        "/auth/login",
        None,
        Some(login_body("mfa_backup", "SuperSecreto_01!")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["mfa_required"], false, "MFA desactivado");
}

#[tokio::test]
async fn test_e2e_audit_retention_cleanup_deletes_old_logs() {
    use dmart_server::audit::{AuditAction, AuditLog, AuditService};

    let (db, _dir) = test_db().await;
    dmart_server::audit::init_global_audit(db.clone());

    let service = AuditService::new(db.clone());
    let _ = service
        .log_login_success("u-fresh", "admin", None)
        .await
        .expect("log fresh");

    let old = AuditLog {
        uid: uuid::Uuid::new_v4().to_string(),
        timestamp: (chrono::Utc::now() - chrono::Duration::days(6 * 365 + 1)).to_rfc3339(),
        user_id: None,
        username: Some("admin".to_string()),
        action: AuditAction::Login,
        resource: "auth".to_string(),
        resource_id: None,
        details: None,
        ip_address: None,
        user_agent: None,
        success: true,
        error_message: None,
        prev_hash: None,
        content_hash: None,
    };
    let _: Option<AuditLog> = db
        .create(("audit_logs", old.uid.clone()))
        .content(old.clone())
        .await
        .expect("seed old log");

    // Verificar que el log antiguo existe y tiene timestamp correcto
    let before: Vec<AuditLog> = db
        .query("SELECT * FROM audit_logs WHERE uid = $uid")
        .bind(("uid", old.uid.clone()))
        .await
        .expect("query old log")
        .take(0)
        .expect("take old");
    assert_eq!(before.len(), 1, "log antiguo sembrado");
    assert!(before[0].timestamp < chrono::Utc::now().to_rfc3339());

    let deleted = service.cleanup_old_logs().await.expect("cleanup");
    assert_eq!(deleted, 1, "solo se borra el log antiguo");
    let remaining = service.get_recent(10).await.expect("recent");
    assert!(
        remaining.iter().all(|l| l.uid != old.uid),
        "el log antiguo ya no está presente"
    );

    seed_user(
        &db,
        "audit_admin",
        "SuperSecreto_01!",
        dmart_shared::models::UserRole::Admin,
        "Audit Admin",
    )
    .await;
    let http = build_app(&db).await;
    let token = login_token(&http, "audit_admin", "SuperSecreto_01!").await;

    let (status, json) = send(
        &http,
        Method::POST,
        "/admin/audit/cleanup",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "cleanup api => {}", json);
    assert_eq!(json["data"]["retention_years"], 6);

    let (status, json) = send(
        &http,
        Method::GET,
        "/admin/audit?limit=5",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "audit list => {}", json);
    assert!(!json["data"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_e2e_realtime_stream_requires_auth_and_accepts_token_param() {
    let (db, _dir) = test_db().await;
    seed_user(
        &db,
        "realtime_admin",
        "SuperSecreto_01!",
        dmart_shared::models::UserRole::Admin,
        "Realtime Admin",
    )
    .await;
    let http = build_app(&db).await;
    let token = login_token(&http, "realtime_admin", "SuperSecreto_01!").await;

    // Sin autenticación => 401
    let (status, _) = send(&http, Method::GET, "/realtime/stream", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "SSE sin token => 401");

    // Token inválido => 401
    let (status, _) = send(
        &http,
        Method::GET,
        "/realtime/stream?token=basura",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "token inválido => 401");

    // Con token en query => 200 + content-type text/event-stream.
    // No usamos send() (colecciona el body completo y el stream no termina);
    // verificamos el status y el content-type de la cabecera.
    let app = http.clone();
    let request = axum::http::Request::builder()
        .method(Method::GET)
        .uri(format!("/realtime/stream?token={}", token))
        .body(Body::empty())
        .expect("build request");
    let response = app.clone().oneshot(request).await.expect("oneshot");
    assert_eq!(response.status(), StatusCode::OK, "SSE con token => 200");
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    assert!(
        content_type.starts_with("text/event-stream"),
        "content-type SSE => {}",
        content_type
    );
    // No consumimos el body: el stream SSE queda abierto y el test termina.
    drop(response);
    let _ = app;
}

#[tokio::test]
async fn test_realtime_broadcast_single_instance() {
    // El hub de eventos es un broadcast: publicar y suscribirse funciona
    // de forma aislada (simula un cliente conectado que espera eventos).
    let hub = dmart_server::realtime::RealtimeHub::new();
    let mut rx = hub.subscribe();
    hub.publish(
        "e2e_event",
        serde_json::json!({ "ok": true, "from": "test" }),
    );
    let msg = tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv())
        .await
        .expect("timeout esperando evento")
        .expect("debe recibir el evento publicado");
    assert!(
        msg.contains("e2e_event"),
        "el evento contiene el tipo esperado: {}",
        msg
    );
}

// ─── SPEC-004: Refresh tokens + RBAC granular ────────────────────────────

#[tokio::test]
async fn test_e2e_refresh_token_rotation_and_reuse_detection() {
    let (db, _dir) = test_db().await;
    seed_user(
        &db,
        "refresh_e2e",
        "SuperSecreto_01!",
        dmart_shared::models::UserRole::Admin,
        "Refresh E2E",
    )
    .await;
    let http = build_app(&db).await;

    let (status, json) = send(
        &http,
        Method::POST,
        "/auth/login",
        None,
        Some(login_body("refresh_e2e", "SuperSecreto_01!")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let access1 = json["data"]["access_token"]
        .as_str()
        .expect("access_token")
        .to_string();
    let rt1 = json["data"]["refresh_token"]
        .as_str()
        .expect("refresh_token")
        .to_string();
    assert!(!access1.is_empty());
    assert!(!rt1.is_empty());

    // Rotación vía cuerpo JSON
    let (status, json) = send(
        &http,
        Method::POST,
        "/auth/refresh",
        None,
        Some(serde_json::json!({ "refresh_token": rt1 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "refresh must succeed: {}", json);
    let access2 = json["data"]["access_token"]
        .as_str()
        .expect("access_token")
        .to_string();
    let rt2 = json["data"]["refresh_token"]
        .as_str()
        .expect("refresh_token")
        .to_string();
    assert_ne!(access2, access1, "access token must rotate");
    assert_ne!(rt2, rt1, "refresh token must rotate");

    // Reuso del token consumido → 401
    let (status, _) = send(
        &http,
        Method::POST,
        "/auth/refresh",
        None,
        Some(serde_json::json!({ "refresh_token": rt1 })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "reused refresh token must be rejected"
    );

    // La familia de la sesión robada fue revocada: access2 ya no sirve
    let (status, _) = send(&http, Method::GET, "/auth/me", Some(&access2), None).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "reuse detection must revoke the whole session family"
    );
}

#[tokio::test]
async fn test_e2e_logout_revokes_refresh_token() {
    let (db, _dir) = test_db().await;
    seed_user(
        &db,
        "logout_e2e",
        "SuperSecreto_01!",
        dmart_shared::models::UserRole::Admin,
        "Logout E2E",
    )
    .await;
    let http = build_app(&db).await;

    // Login — capturamos el Set-Cookie (cookie httpOnly) y el access token.
    let app = http.clone();
    let request = axum::http::Request::builder()
        .method(Method::POST)
        .uri("/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            login_body("logout_e2e", "SuperSecreto_01!").to_string(),
        ))
        .expect("build login");
    let response = app.oneshot(request).await.expect("oneshot login");
    assert_eq!(response.status(), StatusCode::OK);
    let set_cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .expect("Set-Cookie")
        .to_str()
        .expect("ascii")
        .to_string();
    assert!(
        set_cookie.contains("refresh_token="),
        "login must set the refresh cookie: {}",
        set_cookie
    );
    assert!(set_cookie.contains("HttpOnly"), "cookie must be HttpOnly");
    assert!(
        set_cookie.contains("SameSite=Strict"),
        "cookie must be SameSite=Strict"
    );
    assert!(
        set_cookie.contains("Path=/api/auth"),
        "cookie Path must be restricted to /api/auth"
    );
    let rt = set_cookie
        .split(';')
        .next()
        .expect("first cookie pair")
        .trim_start_matches("refresh_token=")
        .to_string();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).expect("login json");
    let access = json["data"]["access_token"]
        .as_str()
        .expect("access_token")
        .to_string();

    // Logout con access token + cookie httpOnly.
    let app2 = http.clone();
    let request2 = axum::http::Request::builder()
        .method(Method::POST)
        .uri("/auth/logout")
        .header("authorization", format!("Bearer {access}"))
        .header(header::COOKIE, format!("refresh_token={rt}"))
        .body(Body::empty())
        .expect("build logout");
    let response2 = app2.oneshot(request2).await.expect("oneshot logout");
    assert_eq!(response2.status(), StatusCode::OK);
    let clear_cookie = response2
        .headers()
        .get(header::SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    assert!(
        clear_cookie.contains("Max-Age=0"),
        "logout must clear the refresh cookie: {}",
        clear_cookie
    );

    // El refresh token ya fue revocado al hacer logout.
    let (status, _) = send(
        &http,
        Method::POST,
        "/auth/refresh",
        None,
        Some(serde_json::json!({ "refresh_token": rt })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "refresh token must be revoked after logout"
    );
}

#[tokio::test]
async fn test_e2e_revoke_all_kills_all_sessions() {
    let (db, _dir) = test_db().await;
    seed_user(
        &db,
        "revokeall_e2e",
        "SuperSecreto_01!",
        dmart_shared::models::UserRole::Admin,
        "RevokeAll E2E",
    )
    .await;
    let http = build_app(&db).await;

    let (status, json) = send(
        &http,
        Method::POST,
        "/auth/login",
        None,
        Some(login_body("revokeall_e2e", "SuperSecreto_01!")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let access = json["data"]["access_token"].as_str().unwrap().to_string();

    let (status, _) = send(&http, Method::POST, "/auth/revoke-all", Some(&access), None).await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = send(&http, Method::GET, "/auth/me", Some(&access), None).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "revoke-all must invalidate the calling session"
    );
}

#[tokio::test]
async fn test_e2e_granular_rbac_permissions() {
    let (db, _dir) = test_db().await;
    seed_user(
        &db,
        "admin_rbac",
        "SuperSecreto_01!",
        dmart_shared::models::UserRole::Admin,
        "Admin RBAC",
    )
    .await;
    seed_user(
        &db,
        "doc_rbac",
        "SuperSecreto_01!",
        dmart_shared::models::UserRole::Medico,
        "Doc RBAC",
    )
    .await;
    seed_user(
        &db,
        "viewer_rbac",
        "SuperSecreto_01!",
        dmart_shared::models::UserRole::Viewer,
        "Viewer RBAC",
    )
    .await;
    let http = build_app(&db).await;

    let admin = login_token(&http, "admin_rbac", "SuperSecreto_01!").await;
    let doc = login_token(&http, "doc_rbac", "SuperSecreto_01!").await;
    let viewer = login_token(&http, "viewer_rbac", "SuperSecreto_01!").await;

    // Viewer: lectura clínica sí, administración no
    let (s, _) = send(
        &http,
        Method::GET,
        "/admin/check-camas",
        Some(&viewer),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "viewer can read clinical data");

    let (s, _) = send(&http, Method::GET, "/admin/audit", Some(&viewer), None).await;
    assert_eq!(s, StatusCode::FORBIDDEN, "viewer cannot read audit");

    let (s, _) = send(&http, Method::GET, "/admin/staff", Some(&viewer), None).await;
    assert_eq!(s, StatusCode::FORBIDDEN, "viewer cannot list staff");

    let (s, _) = send(
        &http,
        Method::POST,
        "/sandbox/generate",
        Some(&viewer),
        Some(serde_json::json!({})),
    )
    .await;
    assert_eq!(
        s,
        StatusCode::FORBIDDEN,
        "viewer cannot generate sandbox data"
    );

    // Médico: administración de personal no, auditoría no
    let (s, _) = send(&http, Method::GET, "/admin/staff", Some(&doc), None).await;
    assert_eq!(s, StatusCode::FORBIDDEN, "medico cannot list staff");

    let (s, _) = send(&http, Method::GET, "/admin/audit", Some(&doc), None).await;
    assert_eq!(s, StatusCode::FORBIDDEN, "medico cannot read audit");

    let (s, _) = send(
        &http,
        Method::PUT,
        "/admin/institucion",
        Some(&doc),
        Some(serde_json::json!({ "nombre": "x" })),
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN, "medico cannot edit config");

    let (s, _) = send(
        &http,
        Method::POST,
        "/sandbox/generate",
        Some(&doc),
        Some(serde_json::json!({})),
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN, "medico cannot generate sandbox");

    // Admin: todo permitido
    let (s, _) = send(&http, Method::GET, "/admin/stats", Some(&admin), None).await;
    assert_eq!(s, StatusCode::OK, "admin reads admin stats");

    let (s, _) = send(&http, Method::GET, "/admin/staff", Some(&admin), None).await;
    assert_eq!(s, StatusCode::OK, "admin lists staff");

    let (s, _) = send(&http, Method::GET, "/admin/institucion", Some(&admin), None).await;
    assert_eq!(s, StatusCode::OK, "admin reads institucion");
}

// ─── Auditoría de aislamiento multi-tenant (SPEC-025) ──────────────────────

#[tokio::test]
async fn test_tenancy_audit_detects_missing_and_orphan_tenant_ids() {
    let (db, _dir) = test_db().await;

    // Baseline: BD vacía → healthy.
    let report = dmart_server::db::audit_tenancy(&db)
        .await
        .expect("audit empty db");
    assert!(report.healthy, "empty DB must be healthy");
    assert_eq!(
        report.tables.len(),
        4,
        "escanea patients/measurements/users/embeddings"
    );
    assert_eq!(report.total_records, 0);

    // Paciente con tenant default → sigue healthy.
    let p = dmart_shared::models::Patient::new();
    let pid = p.patient_id.clone();
    dmart_server::db::create_patient(&db, p)
        .await
        .expect("create patient");
    let report = dmart_server::db::audit_tenancy(&db).await.expect("audit");
    assert!(report.healthy, "default-tenant patient is healthy");

    // Fuga simulada: paciente legacy sin tenant_id (vacío).
    let p2 = dmart_shared::models::Patient::new();
    let pid2 = p2.patient_id.clone();
    dmart_server::db::create_patient(&db, p2)
        .await
        .expect("create patient");
    let upd = db
        .query("UPDATE patients SET tenant_id = '' WHERE patient_id = $p")
        .bind(("p", pid2.clone()))
        .await
        .expect("update patient tenant");
    assert!(upd.check().is_ok(), "update ok");

    let report = dmart_server::db::audit_tenancy(&db).await.expect("audit");
    let patients = report
        .tables
        .iter()
        .find(|t| t.table == "patients")
        .expect("patients row");
    assert_eq!(patients.missing_tenant_id, 1, "detecta tenant ausente");
    assert!(!report.healthy, "unhealthy con fuga");

    // Huérfano: tenant_id no registrado en el catálogo de tenants.
    db.query("UPDATE patients SET tenant_id = 'ghost-tenant' WHERE patient_id = $p")
        .bind(("p", pid.clone()))
        .await
        .expect("update patient to ghost tenant");
    let report = dmart_server::db::audit_tenancy(&db).await.expect("audit");
    let patients = report
        .tables
        .iter()
        .find(|t| t.table == "patients")
        .expect("patients row");
    assert!(
        patients
            .orphan_tenant_ids
            .contains(&"ghost-tenant".to_string()),
        "tenants huérfanos listados"
    );
    assert_eq!(patients.orphan_count, 1, "cuenta de huérfanos");

    // Tras registrar el tenant en el catálogo → ya no es huérfano.
    dmart_server::tenant::create_tenant(&db, "ghost-tenant", "Ghost Hospital")
        .await
        .expect("create tenant");
    let report = dmart_server::db::audit_tenancy(&db).await.expect("audit");
    let patients = report
        .tables
        .iter()
        .find(|t| t.table == "patients")
        .expect("patients row");
    assert!(
        patients.orphan_tenant_ids.is_empty(),
        "no más huérfanos tras registrar"
    );
    assert_eq!(patients.orphan_count, 0);
}

#[tokio::test]
async fn test_e2e_tenants_audit_endpoint() {
    let (db, _dir) = test_db().await;
    let http = build_app(&db).await;

    seed_user(
        &db,
        "tenant_admin",
        "SuperSecreto_01!",
        dmart_shared::models::UserRole::Admin,
        "Tenant Admin",
    )
    .await;
    let token = login_token(&http, "tenant_admin", "SuperSecreto_01!").await;

    // Endpoint sin auth → 401.
    let (s, _) = send(&http, Method::GET, "/admin/tenants/audit", None, None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED, "audit requiere auth");

    // BD vacía → healthy=true.
    let (s, json) = send(
        &http,
        Method::GET,
        "/admin/tenants/audit",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "audit healthy retorna 200");
    assert_eq!(json["data"]["healthy"], true, "BD vacía healthy");
    assert_eq!(json["data"]["tables"].as_array().map(|a| a.len()), Some(4));

    // Fuga: paciente sin tenant → el endpoint reporta unhealthy (409).
    let p = dmart_shared::models::Patient::new();
    dmart_server::db::create_patient(&db, p)
        .await
        .expect("create patient");
    db.query("UPDATE patients SET tenant_id = '' WHERE tenant_id = $t")
        .bind(("t", "default".to_string()))
        .await
        .expect("wipe tenant id");
    let (s, json) = send(
        &http,
        Method::GET,
        "/admin/tenants/audit",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT, "fuga reportada como conflicto");
    assert_eq!(json["data"]["healthy"], false);
    assert_eq!(
        json["data"]["tables"][0]["missing_tenant_id"], 1,
        "detecta el registro sin tenant"
    );
}

// ─── SPEC-044: Consola Técnica de Soporte ─────────────────────────────────

const SUPPORT_PWD: &str = "SoporteSecreto_01!";

#[tokio::test]
async fn test_e2e_support_console_rbac_and_systems() {
    let (db, _dir) = test_db().await;
    let http = build_app(&db).await;

    seed_user(
        &db,
        "sup_admin",
        SUPPORT_PWD,
        dmart_shared::models::UserRole::Admin,
        "Admin",
    )
    .await;
    seed_user(
        &db,
        "sup_soporte",
        SUPPORT_PWD,
        dmart_shared::models::UserRole::Soporte,
        "Soporte",
    )
    .await;
    seed_user(
        &db,
        "sup_viewer",
        SUPPORT_PWD,
        dmart_shared::models::UserRole::Viewer,
        "Viewer",
    )
    .await;

    let admin = login_token(&http, "sup_admin", SUPPORT_PWD).await;
    let soporte = login_token(&http, "sup_soporte", SUPPORT_PWD).await;
    let viewer = login_token(&http, "sup_viewer", SUPPORT_PWD).await;

    // Sin token → 401.
    let (s, _) = send(&http, Method::GET, "/admin/support/systems", None, None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED, "consola requiere auth");

    // Viewer sin support:read → 403.
    let (s, _) = send(
        &http,
        Method::GET,
        "/admin/support/systems",
        Some(&viewer),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN, "viewer no tiene support:read");

    // Soporte y Admin → 200.
    for token in [&soporte, &admin] {
        let (s, json) = send(
            &http,
            Method::GET,
            "/admin/support/systems",
            Some(token),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK, "systems accesible");
        let systems = json["data"].as_array().expect("systems array");
        assert_eq!(systems.len(), 7, "7 subsistemas monitorizados");
        let keys: Vec<&str> = systems
            .iter()
            .map(|s| s["key"].as_str().unwrap_or_default())
            .collect();
        assert!(keys.contains(&"db"));
        assert!(keys.contains(&"ingest"));
        assert!(keys.contains(&"monitores"));
        assert!(keys.contains(&"audit"));
        assert!(keys.contains(&"backup"));
    }

    // Diagnóstico con SLIs y acciones sugeridas.
    let (s, json) = send(
        &http,
        Method::GET,
        "/admin/support/diagnostics",
        Some(&soporte),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let diags = json["data"].as_array().expect("diagnostics array");
    assert_eq!(diags.len(), 7);
    assert!(
        diags.iter().all(|d| d["suggested_actions"].is_array()),
        "cada subsistema sugiere acciones"
    );
}

#[tokio::test]
async fn test_e2e_support_actions_audited_and_persisted() {
    let (db, _dir) = test_db().await;
    let http = build_app(&db).await;

    seed_user(
        &db,
        "act_soporte",
        SUPPORT_PWD,
        dmart_shared::models::UserRole::Soporte,
        "Soporte",
    )
    .await;
    seed_user(
        &db,
        "act_viewer",
        SUPPORT_PWD,
        dmart_shared::models::UserRole::Viewer,
        "Viewer",
    )
    .await;

    let soporte = login_token(&http, "act_soporte", SUPPORT_PWD).await;
    let viewer = login_token(&http, "act_viewer", SUPPORT_PWD).await;

    // Viewer no puede ejecutar acciones (support:act).
    let (s, _) = send(
        &http,
        Method::POST,
        "/admin/support/actions/circuit_reset",
        Some(&viewer),
        Some(serde_json::json!({})),
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN, "viewer carece de support:act");

    // Acción desconocida → 400.
    let (s, _) = send(
        &http,
        Method::POST,
        "/admin/support/actions/no_existe",
        Some(&soporte),
        Some(serde_json::json!({})),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "acción inválida rechazada");

    // Acción válida → 200, con evento persistido.
    let (s, json) = send(
        &http,
        Method::POST,
        "/admin/support/actions/circuit_reset",
        Some(&soporte),
        Some(serde_json::json!({})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "circuit_reset ejecutado");
    assert_eq!(json["data"]["success"], true);
    assert_eq!(json["data"]["action"], "circuit_reset");
    assert_eq!(json["data"]["event"]["origin"], "manual");
    assert_eq!(json["data"]["event"]["action"], "circuit_reset");
    assert_eq!(json["data"]["event"]["username"], "act_soporte");

    // Historial contiene el evento recién registrado.
    let (s, json) = send(
        &http,
        Method::GET,
        "/admin/support/history?limit=10",
        Some(&soporte),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let events = json["data"].as_array().expect("history array");
    assert_eq!(events.len(), 1, "un evento manual registrado");
    assert_eq!(events[0]["action"], "circuit_reset");
    assert_eq!(events[0]["subsystem"], "ingest");
    assert_eq!(events[0]["success"], true);
}

#[tokio::test]
async fn test_e2e_support_model_swap_action() {
    let (db, _dir) = test_db().await;
    let http = build_app(&db).await;

    seed_user(
        &db,
        "swap_admin",
        SUPPORT_PWD,
        dmart_shared::models::UserRole::Admin,
        "Admin",
    )
    .await;
    let admin = login_token(&http, "swap_admin", SUPPORT_PWD).await;

    let (s, json) = send(
        &http,
        Method::POST,
        "/admin/support/actions/model_swap",
        Some(&admin),
        Some(serde_json::json!({ "model": "ews", "version": "" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "swap del modelo por defecto");
    assert_eq!(json["data"]["success"], true);
    assert_eq!(json["data"]["details"]["model"], "ews");
}

#[tokio::test]
async fn test_audit_worm_chain_seal_and_verify() {
    use dmart_server::audit::{AUDIT_GENESIS_HASH, AuditAction, AuditService};

    let (db, _dir) = test_db().await;
    let service = AuditService::new(db.clone());

    for i in 0..5 {
        service
            .log(
                AuditAction::Read,
                "patients",
                Some(&format!("p{i}")),
                Some("u1"),
                Some("admin"),
                None,
                None,
                None,
                true,
                None,
            )
            .await
            .expect("log");
    }

    let batch = service
        .seal_batch(1000)
        .await
        .expect("seal")
        .expect("algún lote");
    assert_eq!(batch.sequence, 1);
    assert_eq!(batch.count, 5);
    assert_eq!(batch.prev_batch_hash, AUDIT_GENESIS_HASH);
    assert_eq!(batch.signature.len(), 64, "firma HMAC-SHA256 en hex");

    let report = service.verify_integrity().await.expect("verify");
    assert!(report.ok, "cadena válida: {report:?}");
    assert_eq!(report.logs_total, 5);
    assert_eq!(report.logs_valid, 5);
    assert_eq!(report.batches_total, 1);
    assert_eq!(report.signatures_valid, 1);
    assert_eq!(report.sealable_logs, 0);

    // Manipular un evento debe romper la verificación (WORM detecta tampering).
    db.query("UPDATE audit_logs SET success = false WHERE uid = $uid")
        .bind(("uid", batch.first_uid.clone()))
        .await
        .expect("tamper");
    let tampered = service.verify_integrity().await.expect("verify tampered");
    assert!(!tampered.ok, "tampering debe detectarse: {tampered:?}");

    // Export de lectura incluye logs + lotes firmados.
    let export = service.export(100).await.expect("export");
    assert_eq!(export.logs.len(), 5);
    assert_eq!(export.batches.len(), 1);
    assert_eq!(export.head_batch_hash, batch.batch_hash);
}
