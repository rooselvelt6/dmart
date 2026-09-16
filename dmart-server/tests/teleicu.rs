//! SPEC-020 — Tele-ICU conformance.
//! Gate: start_session, list_sessions, end_session, live_view over the real
//! HTTP stack (auth + RBAC + rate-limit middleware) via oneshot E2E.

use axum::body::Body;
use axum::http::{Method, StatusCode, header};
use dmart_server::db::{self, Database};
use dmart_shared::models::{GcsData, Measurement, UserRole};
use http_body_util::BodyExt;
use std::net::SocketAddr;
use tempfile::TempDir;
use tower::ServiceExt;

async fn make_test_db() -> (Database, TempDir) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.db");
    let db = db::connect(path.to_str().unwrap()).await.unwrap();
    (db, dir)
}

async fn build_app(db: &Database) -> axum::Router {
    let auth_service = dmart_server::auth::AuthService::new((**db).clone());
    let auth_config = dmart_server::middleware::auth_mod::AuthMiddlewareConfig::new(auth_service);
    let security_state = dmart_server::security::create_security_state();
    dmart_server::api::build_api_router(db.clone(), auth_config, security_state).layer(
        axum::extract::connect_info::MockConnectInfo("127.0.0.1:0".parse::<SocketAddr>().unwrap()),
    )
}

async fn seed_user(db: &Database, username: &str, password: &str, rol: UserRole, nombre: &str) {
    let hash = dmart_server::auth::hash_password(password).expect("hash failed");
    let user = dmart_shared::models::User {
        user_id: uuid::Uuid::new_v4().to_string(),
        username: username.to_string(),
        password_hash: hash,
        rol,
        nombre: nombre.to_string(),
        activo: true,
        created_at: chrono::Utc::now().to_rfc3339(),
        tenant_id: "default".into(),
    };
    dmart_server::db::create_user(db, user)
        .await
        .expect("seed user failed");
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

async fn login_token(http: &axum::Router, username: &str, password: &str) -> String {
    let (status, json) = send(
        http,
        Method::POST,
        "/auth/login",
        None,
        Some(serde_json::json!({ "username": username, "password": password })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    json["data"]["token"]
        .as_str()
        .expect("login token")
        .to_string()
}

async fn create_patient(db: &Database, nombre: &str, apellido: &str) -> String {
    let mut p = dmart_shared::models::Patient::new();
    p.nombre = nombre.to_string();
    p.apellido = apellido.to_string();
    p.cama_id = Some("CAMA-01".to_string());
    p.cama_numero = Some(1);
    dmart_server::db::create_patient(db, p)
        .await
        .expect("create patient")
        .patient_id
}

async fn setup_authed_app(db: &Database) -> (axum::Router, String) {
    let app = build_app(db).await;
    seed_user(db, "drtele", "Clave_123!", UserRole::Admin, "Dr. Tele").await;
    let token = login_token(&app, "drtele", "Clave_123!").await;
    (app, token)
}

// ─── Store ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn store_session_lifecycle() {
    let (db, _tmp) = make_test_db().await;
    let pid = create_patient(&db, "Ana", "Pérez").await;

    let session = dmart_server::teleicu::TeleIcuSession::new(
        pid.clone(),
        "dr-spec-020".into(),
        "video".into(),
    );
    let created = dmart_server::teleicu::create_session(&db, &session)
        .await
        .expect("create");
    assert_eq!(created.status, dmart_server::teleicu::SessionStatus::Active);
    assert!(!created.session_id.is_empty());
    assert!(created.ended_at.is_none());

    let active = dmart_server::teleicu::find_active_session_for_patient(&db, &pid)
        .await
        .expect("find active")
        .expect("active session exists");
    assert_eq!(active.session_id, created.session_id);

    let closed = dmart_server::teleicu::close_session(&db, &created.session_id)
        .await
        .expect("close")
        .expect("closed session");
    assert_eq!(closed.status, dmart_server::teleicu::SessionStatus::Ended);
    assert!(closed.ended_at.is_some());
    assert_eq!(
        dmart_server::teleicu::duration_minutes(
            &closed.started_at,
            closed.ended_at.as_deref().unwrap()
        ),
        Some(0)
    );

    let after = dmart_server::teleicu::find_active_session_for_patient(&db, &pid)
        .await
        .expect("find after close");
    assert!(
        after.is_none(),
        "no debe quedar sesión activa tras cerrarla"
    );
}

// ─── HTTP E2E ──────────────────────────────────────────────────────────

#[tokio::test]
async fn start_session_creates_active_and_rejects_duplicate() {
    let (db, _tmp) = make_test_db().await;
    let (app, token) = setup_authed_app(&db).await;
    let pid = create_patient(&db, "María", "González").await;

    let body = serde_json::json!({
        "patient_id": pid,
        "specialist_id": "dr-tele-1",
        "channel": "video"
    });
    let (status, json) = send(
        &app,
        Method::POST,
        "/teleicu/sessions",
        Some(&token),
        Some(body),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(json["success"], true);
    assert_eq!(json["data"]["status"], "active");
    assert!(!json["data"]["session_id"].as_str().unwrap().is_empty());

    let (status2, json2) = send(
        &app,
        Method::POST,
        "/teleicu/sessions",
        Some(&token),
        Some(serde_json::json!({
            "patient_id": pid,
            "specialist_id": "dr-tele-1",
            "channel": "chat"
        })),
    )
    .await;
    assert_eq!(status2, StatusCode::CONFLICT);
    assert_eq!(json2["success"], false);
}

#[tokio::test]
async fn start_session_requires_patient_and_valid_body() {
    let (db, _tmp) = make_test_db().await;
    let (app, token) = setup_authed_app(&db).await;

    let (status, _) = send(
        &app,
        Method::POST,
        "/teleicu/sessions",
        Some(&token),
        Some(serde_json::json!({ "channel": "video" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = send(
        &app,
        Method::POST,
        "/teleicu/sessions",
        Some(&token),
        Some(serde_json::json!({
            "patient_id": "no-existe-999",
            "specialist_id": "dr-x",
            "channel": "video"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn list_sessions_orders_active_first_then_recent() {
    let (db, _tmp) = make_test_db().await;
    let (app, token) = setup_authed_app(&db).await;
    let pid_a = create_patient(&db, "Luis", "Rodríguez").await;
    let pid_b = create_patient(&db, "Ana", "Torres").await;

    for (pid, channel) in [(&pid_a, "video"), (&pid_b, "chat")] {
        let (status, _) = send(
            &app,
            Method::POST,
            "/teleicu/sessions",
            Some(&token),
            Some(serde_json::json!({
                "patient_id": pid,
                "specialist_id": "dr-tele-1",
                "channel": channel
            })),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
    }

    // Finaliza la sesión de B para que quede como reciente.
    let (status, list) = send(&app, Method::GET, "/teleicu/sessions", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    let session_b = list["data"]["active"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["patient_id"] == pid_b)
        .expect("session B")
        .clone();
    let (status, _) = send(
        &app,
        Method::POST,
        &format!(
            "/teleicu/sessions/{}/end",
            session_b["session_id"].as_str().unwrap()
        ),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, json) = send(&app, Method::GET, "/teleicu/sessions", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    let active = json["data"]["active"].as_array().unwrap();
    let recent = json["data"]["recent"].as_array().unwrap();
    assert_eq!(active.len(), 1, "solo A sigue activa");
    assert_eq!(active[0]["patient_id"], pid_a);
    assert_eq!(recent.len(), 1, "B quedó como reciente finalizada");
    assert_eq!(recent[0]["patient_id"], pid_b);
    assert_eq!(recent[0]["status"], "ended");
}

#[tokio::test]
async fn end_session_sets_ended_at_and_duration() {
    let (db, _tmp) = make_test_db().await;
    let (app, token) = setup_authed_app(&db).await;
    let pid = create_patient(&db, "Carlos", "Martínez").await;

    let (status, json) = send(
        &app,
        Method::POST,
        "/teleicu/sessions",
        Some(&token),
        Some(serde_json::json!({
            "patient_id": pid,
            "specialist_id": "dr-tele-2",
            "channel": "video"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let session_id = json["data"]["session_id"].as_str().unwrap().to_string();

    let path = format!("/teleicu/sessions/{}/end", session_id);
    let (status, json) = send(&app, Method::POST, &path, Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["session"]["status"], "ended");
    assert!(json["data"]["session"]["ended_at"].is_string());
    assert!(json["data"]["duration_minutes"].as_i64().is_some());

    let (status2, _) = send(&app, Method::POST, &path, Some(&token), None).await;
    assert_eq!(status2, StatusCode::CONFLICT);

    let (status3, _) = send(
        &app,
        Method::POST,
        "/teleicu/sessions/sesion-inexistente/end",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status3, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn live_view_returns_patient_snapshot() {
    let (db, _tmp) = make_test_db().await;
    let (app, token) = setup_authed_app(&db).await;
    let pid = create_patient(&db, "Rosa", "Fernández").await;

    let (status, _) = send(
        &app,
        Method::POST,
        "/teleicu/sessions",
        Some(&token),
        Some(serde_json::json!({
            "patient_id": pid,
            "specialist_id": "dr-tele-3",
            "channel": "video"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let gcs = GcsData {
        apertura_ocular: 4,
        respuesta_verbal: 5,
        respuesta_motora: 6,
    };
    let m = Measurement::new(&pid, dmart_shared::models::ApacheIIData::default(), gcs);
    dmart_server::db::create_measurement(&db, m)
        .await
        .expect("measurement");

    for i in 0..12 {
        let event = dmart_server::patient_timeline::PatientEvent::new(
            pid.clone(),
            dmart_server::patient_timeline::EventType::VitalSigns,
            serde_json::json!({ "seq": i }),
            dmart_server::patient_timeline::EventSeverity::Info,
            "monitor".to_string(),
        );
        event.store(&db).await.expect("event stored");
    }

    let (status, json) = send(
        &app,
        Method::GET,
        &format!("/teleicu/live/{}", pid),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["patient"]["patient_id"], pid);
    assert_eq!(json["data"]["nombre_completo"], "Rosa Fernández");
    assert_eq!(json["data"]["cama_id"], "CAMA-01");
    assert_eq!(json["data"]["session"]["status"], "active");
    assert_eq!(json["data"]["last_measurement"]["patient_id"], pid);
    let events = json["data"]["timeline"]["events"].as_array().unwrap();
    assert_eq!(
        events.len(),
        10,
        "el snapshot limita el timeline a 10 eventos"
    );
}

#[tokio::test]
async fn live_view_404_missing_patient() {
    let (db, _tmp) = make_test_db().await;
    let (app, token) = setup_authed_app(&db).await;

    let (status, json) = send(
        &app,
        Method::GET,
        "/teleicu/live/paciente-inexistente",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["success"], false);
}

#[tokio::test]
async fn live_view_requires_active_session() {
    let (db, _tmp) = make_test_db().await;
    let (app, token) = setup_authed_app(&db).await;
    let pid = create_patient(&db, "Pedro", "Sánchez").await;

    // Sin sesión: 404
    let (status, _) = send(
        &app,
        Method::GET,
        &format!("/teleicu/live/{}", pid),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Con sesión ya finalizada: 404
    let (status, json) = send(
        &app,
        Method::POST,
        "/teleicu/sessions",
        Some(&token),
        Some(serde_json::json!({
            "patient_id": pid,
            "specialist_id": "dr-tele-4",
            "channel": "chat"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let session_id = json["data"]["session_id"].as_str().unwrap().to_string();
    let (status, _) = send(
        &app,
        Method::POST,
        &format!("/teleicu/sessions/{}/end", session_id),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = send(
        &app,
        Method::GET,
        &format!("/teleicu/live/{}", pid),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
