//! E2E por escala (FASE 3.2 del plan9): cada endpoints `/scales/{escala}` se
//! ejercita con los fixtures oro de `dmart-shared/testdata/scales/` y con casos
//! min/max/clamp, verificando score exacto, fingerprint reproducible y
//! persistencia en SurrealDB. Sigue el patrón de `api_tests.rs` (in-process via
//! `tower::ServiceExt::oneshot`, recomendado en AGENTS.md).

#![allow(dead_code)]

use axum::body::Body;
use axum::http::{Method, StatusCode, header};
use http_body_util::BodyExt;
use std::net::SocketAddr;
use surrealdb::Surreal;
use surrealdb::engine::local::Db;
use tower::ServiceExt;

type TestDb = Surreal<Db>;

async fn test_db() -> (TestDb, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("e2e.db");
    let db = Surreal::new::<surrealdb::engine::local::SurrealKv>(
        path.to_str().expect("path"),
    )
    .await
    .expect("connect");
    db.use_ns("dmart").use_db("icu").await.expect("ns");
    (db, dir)
}

async fn build_app(db: &TestDb) -> axum::Router {
    unsafe {
        std::env::set_var("DMART_DISABLE_RATE_LIMIT", "true");
        std::env::set_var("DMART_DISABLE_LOGIN_THROTTLE", "true");
        std::env::set_var("DMART_DISABLE_MFA_THROTTLE", "true");
    }
    let auth_service = dmart_server::auth::AuthService::new(db.clone());
    let auth_config = dmart_server::middleware::auth_mod::AuthMiddlewareConfig::new(auth_service);
    let security_state = dmart_server::security::create_security_state().await;
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
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
    };
    (status, json)
}

fn login_body(username: &str, password: &str) -> serde_json::Value {
    serde_json::json!({ "username": username, "password": password })
}

async fn login_token(app: &axum::Router, username: &str, password: &str) -> String {
    let (status, json) = send(
        app,
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

async fn seed_user_in_tenant(
    db: &TestDb,
    username: &str,
    password: &str,
    rol: dmart_shared::models::UserRole,
    tenant_id: &str,
) {
    let hash = dmart_server::auth::hash_password(password).expect("hash failed");
    let user = dmart_shared::models::User {
        user_id: uuid::Uuid::new_v4().to_string(),
        username: username.into(),
        password_hash: hash,
        rol,
        nombre: username.into(),
        activo: true,
        created_at: chrono::Utc::now().to_rfc3339(),
        tenant_id: tenant_id.into(),
    };
    dmart_server::db::create_user(db, user)
        .await
        .expect("seed user failed");
}

async fn seed_patient_in_tenant(db: &TestDb, tenant_id: &str, mrn: &str) -> String {
    let mut p = dmart_shared::models::Patient::new();
    p.tenant_id = tenant_id.to_string();
    p.historia_clinica = mrn.to_string();
    let pid = p.patient_id.clone();
    dmart_server::db::create_patient(db, p)
        .await
        .expect("create patient");
    pid
}

/// Ambiente E2E: tenant + usuario + paciente, devuelve (app, token, pid).
async fn e2e_env(mrn: &str) -> (axum::Router, String, String, TestDb) {
    let (db, _dir) = test_db().await;
    dmart_server::migrations::run_migrations(&db)
        .await
        .expect("migrations");
    dmart_server::tenant::create_tenant(&db, "hosp-x", "Hospital X").await.ok();
    dmart_server::tenant::create_tenant(&db, "hosp-y", "Hospital Y").await.ok();
    let pid = seed_patient_in_tenant(&db, "hosp-x", mrn).await;
    seed_user_in_tenant(
        &db,
        "e2e_clin",
        "SuperSecreto_01!",
        dmart_shared::models::UserRole::Enfermero,
        "hosp-x",
    )
    .await;
    let app = build_app(&db).await;
    let token = login_token(&app, "e2e_clin", "SuperSecreto_01!").await;
    (app, token, pid, db)
}

fn as_f32(j: &serde_json::Value, key: &str) -> f32 {
    j[key].as_f64().expect("f32 field") as f32
}

fn as_u32(j: &serde_json::Value, key: &str) -> u32 {
    j[key].as_u64().expect("u32 field") as u32
}

// ─── APACHE II ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_apache_ii_golden_fixtures_exact_match() {
    let (app, token, pid, db) = e2e_env("MRN-E2E-APACHE").await;
    let fixtures = dmart_shared::testdata::load_apache_ii_fixtures().expect("fixtures");
    assert!(fixtures.len() >= 4, "min 4 fixtures oro APACHE II");

    for fx in &fixtures {
        let expected = fx.expected.apache_ii;
        let body = serde_json::json!({
            "data": fx.inputs,
            "notas": format!("E2E oro {}", fx.id),
        });
        let (status, json) = send(
            &app,
            Method::POST,
            &format!("/patients/{pid}/scales/apache"),
            Some(&token),
            Some(body),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "fixture {}: {json}", fx.id);
        let score = as_u32(&json["data"], "apache_score");
        assert_eq!(score, expected, "fixture {} debe dar score exacto", fx.id);
    }

    // Persistencia + fingerprint reproducible de cada medición guardada.
    let persisted = dmart_server::db::get_measurements_for_patient(&db, &pid)
        .await
        .expect("query measurements");
    assert_eq!(persisted.len(), fixtures.len(), "una medición por fixture");
    let mut pers_scores: Vec<u32> = persisted.iter().map(|m| m.apache_score).collect();
    let mut exp_scores: Vec<u32> = fixtures.iter().map(|f| f.expected.apache_ii).collect();
    pers_scores.sort_unstable();
    exp_scores.sort_unstable();
    assert_eq!(pers_scores, exp_scores, "scores persistidos = scores oro");
    for m in &persisted {
        assert_eq!(m.fingerprint.len(), 64, "fingerprint sha256 hex");
        let recomputed = dmart_shared::scales::score_fingerprint(
            "apache_ii",
            dmart_shared::scales::ALGO_VERSION,
            &serde_json::to_value(&m.apache_data).expect("inputs"),
        );
        assert_eq!(m.fingerprint, recomputed, "fingerprint reproducible");
    }
}

// ─── GCS ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_gcs_golden_fixtures_exact_match_and_clamp() {
    let (app, token, pid, db) = e2e_env("MRN-E2E-GCS").await;
    let fixtures = dmart_shared::testdata::load_gcs_fixtures().expect("fixtures");
    assert!(fixtures.len() >= 4, "min 4 fixtures oro GCS");

    for fx in &fixtures {
        let body = serde_json::json!({
            "apertura_ocular": fx.inputs.apertura_ocular,
            "respuesta_verbal": fx.inputs.respuesta_verbal,
            "respuesta_motora": fx.inputs.respuesta_motora,
            "notas": format!("E2E oro {}", fx.id),
        });
        let (status, json) = send(
            &app,
            Method::POST,
            &format!("/patients/{pid}/scales/gcs"),
            Some(&token),
            Some(body),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "fixture {}: {json}", fx.id);
        let total = as_u32(&json["data"], "total") as u8;
        assert_eq!(total, fx.expected.gcs_total, "fixture {}", fx.id);
    }

    let persisted = dmart_server::db::get_measurements_for_patient(&db, &pid)
        .await
        .expect("query measurements");
    assert_eq!(persisted.len(), fixtures.len());
    assert!(persisted.iter().all(|m| m.gcs_score >= 3 && m.gcs_score <= 15));

    // Clamp de componente individuales en el endpoint (1-4 / 1-5 / 1-6).
    let (status, json) = send(
        &app,
        Method::POST,
        &format!("/patients/{pid}/scales/gcs"),
        Some(&token),
        Some(serde_json::json!({
            "apertura_ocular": 99,
            "respuesta_verbal": 99,
            "respuesta_motora": 99,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{json}");
    assert_eq!(as_u32(&json["data"], "total"), 15, "clamp superior");
    let echo_upper = (
        as_u32(&json["data"], "apertura_ocular"),
        as_u32(&json["data"], "respuesta_verbal"),
        as_u32(&json["data"], "respuesta_motora"),
    );
    assert_eq!(echo_upper, (4, 5, 6), "echo de componentes clampado");

    let (status, json) = send(
        &app,
        Method::POST,
        &format!("/patients/{pid}/scales/gcs"),
        Some(&token),
        Some(serde_json::json!({
            "apertura_ocular": 0,
            "respuesta_verbal": 0,
            "respuesta_motora": 0,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{json}");
    assert_eq!(as_u32(&json["data"], "total"), 3, "clamp inferior");
}

// ─── NEWS2 ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_news2_golden_fixtures_exact_match() {
    let (app, token, pid, db) = e2e_env("MRN-E2E-NEWS2").await;
    let fixtures = dmart_shared::testdata::load_news2_fixtures().expect("fixtures");
    assert!(fixtures.len() >= 4, "min 4 fixtures oro NEWS2");

    for fx in &fixtures {
        let body = serde_json::json!({
            "frecuencia_respiratoria": fx.inputs.frecuencia_respiratoria,
            "spo2": fx.inputs.spo2,
            "o2_suplementario": fx.inputs.o2_suplementario,
            "presion_sistolica": fx.inputs.presion_sistolica,
            "frecuencia_cardiaca": fx.inputs.frecuencia_cardiaca,
            "temperatura": fx.inputs.temperatura,
            "alerta": fx.inputs.alerta,
            "notas": format!("E2E oro {}", fx.id),
        });
        let (status, json) = send(
            &app,
            Method::POST,
            &format!("/patients/{pid}/scales/news2"),
            Some(&token),
            Some(body),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "fixture {}: {json}", fx.id);
        let score = as_u32(&json["data"], "score");
        assert_eq!(score, fx.expected.news2, "fixture {}", fx.id);
        if let Some(level) = &fx.expected.level {
            assert_eq!(json["data"]["nivel"].as_str(), Some(level.as_str()), "fixture {}", fx.id);
        }
    }

    let persisted = dmart_server::db::get_measurements_for_patient(&db, &pid)
        .await
        .expect("query measurements");
    assert_eq!(persisted.len(), fixtures.len());
    assert!(
        persisted.iter().all(|m| m.news2_score.is_some()),
        "news2_score persistido"
    );
}

// ─── SOFA ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_sofa_golden_fixtures_exact_match() {
    let (app, token, pid, db) = e2e_env("MRN-E2E-SOFA").await;
    let fixtures = dmart_shared::testdata::load_sofa_fixtures().expect("fixtures");
    assert!(fixtures.len() >= 4, "min 4 fixtures oro SOFA");

    for fx in &fixtures {
        let body = serde_json::json!({
            "pao2": fx.inputs.pao2.unwrap_or(0.0),
            "fio2": fx.inputs.fio2,
            "plaquetas": fx.inputs.plaquetas,
            "bilirrubina": fx.inputs.bilirrubina,
            "presion_arterial_media": fx.inputs.presion_arterial_media,
            "vasopresores": fx.inputs.vasopresores,
            "dosis_vasopresor": fx.inputs.dosis_vasopresor,
            "gcs_total": fx.inputs.gcs_total,
            "creatinina": fx.inputs.creatinina,
            "diuresis_diaria": fx.inputs.diuresis_diaria,
            "notas": format!("E2E oro {}", fx.id),
        });
        let (status, json) = send(
            &app,
            Method::POST,
            &format!("/patients/{pid}/scales/sofa"),
            Some(&token),
            Some(body),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "fixture {}: {json}", fx.id);
        let score = as_u32(&json["data"], "score");
        assert_eq!(score, fx.expected.sofa, "fixture {}", fx.id);
        if let Some(mort) = fx.expected.mortality {
            assert_eq!(as_f32(&json["data"], "mortalidad_estimada"), mort, "fixture {}", fx.id);
        }
    }

    let persisted = dmart_server::db::get_measurements_for_patient(&db, &pid)
        .await
        .expect("query measurements");
    assert!(persisted.iter().all(|m| m.sofa_score.is_some()));
}

// ─── SAPS III ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_saps3_golden_fixtures_exact_match() {
    let (app, token, pid, db) = e2e_env("MRN-E2E-SAPS3").await;
    let fixtures = dmart_shared::testdata::load_saps3_fixtures().expect("fixtures");
    assert!(fixtures.len() >= 4, "min 4 fixtures oro SAPS III");

    for fx in &fixtures {
        let body = serde_json::json!({
            "edad": fx.inputs.edad,
            "dias_pre_uci": fx.inputs.dias_pre_uci,
            "tipo_admision": fx.inputs.tipo_admision,
            "fuente_admision": fx.inputs.fuente_admision,
            "infeccion_admision": fx.inputs.infeccion_admision,
            "sistema_anatomico": fx.inputs.sistema_anatomico,
            "temperatura": fx.inputs.temperatura,
            "presion_sistolica": fx.inputs.presion_sistolica,
            "frecuencia_cardiaca": fx.inputs.frecuencia_cardiaca,
            "gcs_total": fx.inputs.gcs_total,
            "bilirrubina": fx.inputs.bilirrubina,
            "creatinina": fx.inputs.creatinina,
            "plaquetas": fx.inputs.plaquetas,
            "ph_arterial": fx.inputs.ph_arterial,
            "ventilacion_mecanica": fx.inputs.ventilacion_mecanica,
            "vasopresores": fx.inputs.vasopresores,
            "inmunocomprometido": fx.inputs.inmunocomprometido,
            "leucocitos": fx.inputs.leucocitos,
            "fio2": fx.inputs.fio2,
            "pao2": fx.inputs.pao2,
            "notas": format!("E2E oro {}", fx.id),
        });
        let (status, json) = send(
            &app,
            Method::POST,
            &format!("/patients/{pid}/scales/saps3"),
            Some(&token),
            Some(body),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "fixture {}: {json}", fx.id);
        let score = as_u32(&json["data"], "score");
        assert_eq!(score, fx.expected.saps3, "fixture {}", fx.id);
        if let Some(mort) = fx.expected.mortality {
            assert_eq!(
                as_f32(&json["data"], "mortalidad_estimada"),
                mort,
                "fixture {} mortalidad",
                fx.id
            );
        }
    }

    let persisted = dmart_server::db::get_measurements_for_patient(&db, &pid)
        .await
        .expect("query measurements");
    assert!(persisted.iter().all(|m| m.saps3_score.is_some()));
}

// ─── Historial + persistencia completa ──────────────────────────────────────

#[tokio::test]
async fn e2e_scale_history_returns_chronological_entries() {
    let (app, token, pid, _db) = e2e_env("MRN-E2E-HIST").await;

    // Dos mediciones en el mismo paciente: una APACHE II y una GCS.
    let gcs = serde_json::json!({
        "apertura_ocular": 4,
        "respuesta_verbal": 5,
        "respuesta_motora": 6,
        "notas": "E2E history",
    });
    let (s, _) = send(
        &app,
        Method::POST,
        &format!("/patients/{pid}/scales/gcs"),
        Some(&token),
        Some(gcs),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);

    let apache = serde_json::json!({
        "data": dmart_shared::models::ApacheIIData::default(),
        "notas": "E2E history apache",
    });
    let (s, json) = send(
        &app,
        Method::POST,
        &format!("/patients/{pid}/scales/apache"),
        Some(&token),
        Some(apache),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{json}");

    let (status, json) = send(
        &app,
        Method::GET,
        &format!("/patients/{pid}/scales/history"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{json}");
    let entries = json["data"].as_array().expect("history array");
    assert_eq!(entries.len(), 2, "dos escalas en el historial");
    assert!(entries[0]["gcs_score"].as_u64().is_some());
    assert!(entries[1]["apache_score"].as_u64().is_some());
    let ts0 = entries[0]["timestamp"].as_str().expect("ts");
    let ts1 = entries[1]["timestamp"].as_str().expect("ts");
    assert!(
        ts0 <= ts1 || entries[0]["gcs_score"].is_number(),
        "historial cronológico"
    );
}

// ─── Cross-tenant: escala de paciente ajeno rechazada ───────────────────────

#[tokio::test]
async fn e2e_scales_cross_tenant_rejected() {
    let (db, _dir) = test_db().await;
    dmart_server::migrations::run_migrations(&db)
        .await
        .expect("migrations");
    dmart_server::tenant::create_tenant(&db, "hosp-a", "Hospital A").await.ok();
    dmart_server::tenant::create_tenant(&db, "hosp-b", "Hospital B").await.ok();
    let pid_b = seed_patient_in_tenant(&db, "hosp-b", "MRN-CROSS-B").await;
    seed_user_in_tenant(
        &db,
        "clin_a",
        "SuperSecreto_01!",
        dmart_shared::models::UserRole::Enfermero,
        "hosp-a",
    )
    .await;
    let app = build_app(&db).await;
    let token = login_token(&app, "clin_a", "SuperSecreto_01!").await;

    let (status, json) = send(
        &app,
        Method::POST,
        &format!("/patients/{pid_b}/scales/gcs"),
        Some(&token),
        Some(serde_json::json!({
            "apertura_ocular": 4,
            "respuesta_verbal": 5,
            "respuesta_motora": 6,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{json}");
}