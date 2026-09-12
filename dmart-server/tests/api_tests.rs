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
        })
        .await
        .expect("register failed");
    assert_eq!(reg.username, "testdoc");

    let login = auth
        .authenticate("testdoc", "TestPass123!")
        .await
        .expect("authenticate failed");
    assert!(!login.token.is_empty());

    let refresh = auth.refresh_token(&login.token).await;
    assert!(refresh.is_ok());

    let revoked = auth.verify_token(login.token.as_str());
    assert!(revoked.is_ok());
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
        .authenticate("hashcheck", "Secreto_123!")
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
    dmart_server::api::build_api_router(database, auth_config, security_state)
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
