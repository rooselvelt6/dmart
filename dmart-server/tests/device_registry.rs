//! SPEC-017 — Device Registry conformance.
//! Gate: registro, get, heartbeat (ventana offline), filtro por estado y
//! summary GROUP BY, sobre HTTP y sobre el store.
//! clippy -D 0/0; reusa device_registry module.

use axum::body::Body;
use axum::http::{Method, StatusCode, header};
use dmart_server::db;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::sync::Arc;
use surrealdb::Surreal;
use surrealdb::engine::local::Db;
use tempfile::TempDir;
use tower::ServiceExt;

type TestDb = Surreal<Db>;

use dmart_server::device_registry::{
    ClinicalDevice, DEFAULT_HEARTBEAT_INTERVAL_SECS, RegisterDeviceInput, stale_window,
};

async fn make_test_db() -> (Arc<db::Database>, TempDir) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.db");
    let db = db::connect(path.to_str().unwrap()).await.unwrap();
    (Arc::new(db), dir)
}

fn device_id(device: &ClinicalDevice) -> String {
    device.id.clone()
}

// ─── Store ────────────────────────────────────────────────────────────

#[tokio::test]
async fn store_register_and_get() {
    let (db, _tmp) = make_test_db().await;
    let input = RegisterDeviceInput {
        id: None,
        device_type: "monitor".into(),
        fabricante: "Philips".into(),
        modelo: "IntelliVue MX450".into(),
        firmware: "4.1.2".into(),
        serial: "SN-MX450-0001".into(),
        cama_id: Some("cama-01".into()),
        ubicacion: None,
        estado: None,
        heartbeat_interval_secs: None,
    };
    let created = dmart_server::device_registry::register(&db, input)
        .await
        .unwrap();
    assert_eq!(created.estado, "online");
    assert_eq!(
        created.heartbeat_interval_secs,
        DEFAULT_HEARTBEAT_INTERVAL_SECS
    );
    assert!(created.last_seen_at.is_none());
    assert!(!device_id(&created).is_empty());

    let fetched = dmart_server::device_registry::get(&db, &device_id(&created))
        .await
        .unwrap()
        .expect("device exists");
    assert_eq!(fetched.serial, "SN-MX450-0001");
    assert_eq!(fetched.cama_id.as_deref(), Some("cama-01"));
}

#[tokio::test]
async fn store_get_unknown_returns_none() {
    let (db, _tmp) = make_test_db().await;
    let fetched = dmart_server::device_registry::get(&db, "no-existe")
        .await
        .unwrap();
    assert!(fetched.is_none());
}

#[tokio::test]
async fn store_heartbeat_updates_last_seen_and_online() {
    let (db, _tmp) = make_test_db().await;
    let input = RegisterDeviceInput {
        id: Some("vent-001".into()),
        device_type: "ventilador".into(),
        fabricante: "Draeger".into(),
        modelo: "Evita V300".into(),
        firmware: "1.0".into(),
        serial: "SN-V300-0001".into(),
        cama_id: None,
        ubicacion: Some("UCI-02 Modulo B".into()),
        estado: Some("offline".into()),
        heartbeat_interval_secs: None,
    };
    dmart_server::device_registry::register(&db, input)
        .await
        .unwrap();

    let before = dmart_server::device_registry::get(&db, "vent-001")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(before.estado, "offline");

    let after = dmart_server::device_registry::heartbeat(&db, "vent-001")
        .await
        .unwrap()
        .expect("device exists");
    assert_eq!(after.estado, "online");
    assert!(
        after
            .last_seen_at
            .is_some_and(|t| t >= before.registered_at)
    );
}

#[tokio::test]
async fn store_heartbeat_unknown_returns_none() {
    let (db, _tmp) = make_test_db().await;
    let after = dmart_server::device_registry::heartbeat(&db, "fantasma")
        .await
        .unwrap();
    assert!(after.is_none());
}

#[tokio::test]
async fn store_stale_devices_marked_offline() {
    let (db, _tmp) = make_test_db().await;
    let input = RegisterDeviceInput {
        id: Some("mon-001".into()),
        device_type: "monitor".into(),
        fabricante: "Philips".into(),
        modelo: "M8010".into(),
        firmware: "2.0".into(),
        serial: "SN-M8010-0001".into(),
        cama_id: None,
        ubicacion: None,
        estado: Some("online".into()),
        heartbeat_interval_secs: Some(20),
    };
    dmart_server::device_registry::register(&db, input)
        .await
        .unwrap();
    // Envejecer artificialmente el heartbeat más allá de la ventana (20*3=60s).
    let viejo = chrono::Utc::now().timestamp_millis() - 10 * 60 * 1000;
    db.query("UPDATE device_registry SET last_seen_at = $ts WHERE meta::id(id) = $id")
        .bind(("ts", viejo))
        .bind(("id", "mon-001"))
        .await
        .unwrap();

    let marcados = dmart_server::device_registry::reconcile_stale_devices(&db)
        .await
        .unwrap();
    assert_eq!(marcados, 1);

    let dev = dmart_server::device_registry::get(&db, "mon-001")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(dev.estado, "offline");
}

#[tokio::test]
async fn store_stale_preserves_mantenimiento() {
    let (db, _tmp) = make_test_db().await;
    let input = RegisterDeviceInput {
        id: Some("bomba-001".into()),
        device_type: "bomba".into(),
        fabricante: "B.Braun".into(),
        modelo: "Perfusor".into(),
        firmware: "3.3".into(),
        serial: "SN-PF-0001".into(),
        cama_id: None,
        ubicacion: None,
        estado: Some("mantenimiento".into()),
        heartbeat_interval_secs: Some(5),
    };
    dmart_server::device_registry::register(&db, input)
        .await
        .unwrap();
    let viejo = chrono::Utc::now().timestamp_millis() - 10 * 60 * 1000;
    db.query("UPDATE device_registry SET last_seen_at = $ts WHERE meta::id(id) = $id")
        .bind(("ts", viejo))
        .bind(("id", "bomba-001"))
        .await
        .unwrap();

    let marcados = dmart_server::device_registry::reconcile_stale_devices(&db)
        .await
        .unwrap();
    assert_eq!(marcados, 0);

    let dev = dmart_server::device_registry::get(&db, "bomba-001")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(dev.estado, "mantenimiento");
}

#[tokio::test]
async fn store_heartbeat_preserves_mantenimiento() {
    let (db, _tmp) = make_test_db().await;
    let input = RegisterDeviceInput {
        id: Some("bomba-002".into()),
        device_type: "bomba".into(),
        fabricante: "B.Braun".into(),
        modelo: "Perfusor".into(),
        firmware: "3.3".into(),
        serial: "SN-PF-0002".into(),
        cama_id: None,
        ubicacion: None,
        estado: Some("mantenimiento".into()),
        heartbeat_interval_secs: None,
    };
    dmart_server::device_registry::register(&db, input)
        .await
        .unwrap();

    let after = dmart_server::device_registry::heartbeat(&db, "bomba-002")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.estado, "mantenimiento");
    assert!(after.last_seen_at.is_some());
}

#[tokio::test]
async fn store_list_filters_by_estado_and_fulfills_summary() {
    let (db, _tmp) = make_test_db().await;
    for (id, tipo, estado) in [
        ("mon-a", "monitor", "online"),
        ("vent-b", "ventilador", "offline"),
        ("bomba-c", "bomba", "mantenimiento"),
        ("mon-d", "monitor", "online"),
    ] {
        dmart_server::device_registry::register(
            &db,
            RegisterDeviceInput {
                id: Some(id.into()),
                device_type: tipo.into(),
                fabricante: "Test".into(),
                modelo: tipo.into(),
                firmware: "1.0".into(),
                serial: id.into(),
                cama_id: None,
                ubicacion: None,
                estado: Some(estado.into()),
                heartbeat_interval_secs: None,
            },
        )
        .await
        .unwrap();
    }

    let online = dmart_server::device_registry::list(&db, Some("online"))
        .await
        .unwrap();
    assert_eq!(online.len(), 2);
    assert!(online.iter().all(|d| d.estado == "online"));

    let todos = dmart_server::device_registry::list(&db, None)
        .await
        .unwrap();
    assert_eq!(todos.len(), 4);

    let summary = dmart_server::device_registry::status_summary(&db)
        .await
        .unwrap();
    assert_eq!(summary.total, 4);
    let estado_total: u64 = summary.por_estado.iter().map(|c| c.total).sum();
    assert_eq!(estado_total, 4);
    let por_tipo: std::collections::HashMap<String, u64> = summary
        .por_tipo
        .iter()
        .map(|c| (c.device_type.clone(), c.total))
        .collect();
    assert_eq!(por_tipo.get("monitor"), Some(&2));
    assert_eq!(por_tipo.get("ventilador"), Some(&1));
}

#[tokio::test]
async fn stale_window_floor_is_60s() {
    assert_eq!(stale_window(1), 60);
    assert_eq!(stale_window(30), 90);
    assert_eq!(stale_window(60), 180);
}

// ─── HTTP (router real + auth Bearer) ─────────────────────────────────

async fn seed_user(
    db: &TestDb,
    username: &str,
    password: &str,
    rol: dmart_shared::models::UserRole,
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
        tenant_id: "default".into(),
    };
    dmart_server::db::create_user(db, user)
        .await
        .expect("seed user failed");
}

async fn build_app(db: &db::Database) -> axum::Router {
    let raw: &Surreal<Db> = db;
    let auth_service = dmart_server::auth::AuthService::new(raw.clone());
    let auth_config = dmart_server::middleware::auth_mod::AuthMiddlewareConfig::new(auth_service);
    let security_state = dmart_server::security::create_security_state();
    dmart_server::api::build_api_router(db.clone(), auth_config, security_state).layer(
        axum::extract::connect_info::MockConnectInfo("127.0.0.1:0".parse::<SocketAddr>().unwrap()),
    )
}

async fn send(
    app: &axum::Router,
    method: Method,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let app = app.clone();
    let mut builder = axum::http::Request::builder()
        .method(method)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(t) = token {
        builder = builder.header("authorization", format!("Bearer {t}"));
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
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

async fn login_token(app: &axum::Router) -> String {
    let (status, json) = send(
        app,
        Method::POST,
        "/auth/login",
        None,
        Some(json!({ "username": "devops", "password": "s3cret-test" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "login failed: {json}");
    json["data"]["token"].as_str().expect("token").to_string()
}

async fn http_app() -> (axum::Router, String, TempDir) {
    let (db, tmp) = make_test_db().await;
    seed_user(
        &db,
        "devops",
        "s3cret-test",
        dmart_shared::models::UserRole::Admin,
    )
    .await;
    let app = build_app(&db).await;
    let token = login_token(&app).await;
    (app, token, tmp)
}

#[tokio::test]
async fn http_register_list_get_heartbeat_flow() {
    let (app, token, _tmp) = http_app().await;

    let (status, json) = send(
        &app,
        Method::POST,
        "/devices",
        Some(&token),
        Some(json!({
            "device_type": "monitor",
            "fabricante": "Philips",
            "modelo": "IntelliVue MX450",
            "firmware": "4.1.2",
            "serial": "SN-API-0001",
            "cama_id": "cama-07"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "register: {json}");
    let device = &json["data"];
    assert_eq!(device["estado"], "online");
    let id = device["id"].as_str().expect("device id").to_string();

    let (status, json) = send(&app, Method::GET, "/devices", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    let items = json["data"].as_array().expect("list array");
    assert_eq!(items.len(), 1);

    let (status, json) = send(
        &app,
        Method::GET,
        &format!("/devices/{id}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "get: {json}");
    assert_eq!(json["data"]["serial"], "SN-API-0001");

    let (status, json) = send(
        &app,
        Method::POST,
        &format!("/devices/{id}/heartbeat"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "heartbeat: {json}");
    assert_eq!(json["data"]["estado"], "online");
    assert!(json["data"]["last_seen_at"].is_i64());
}

#[tokio::test]
async fn http_register_generates_id_when_absent() {
    let (app, token, _tmp) = http_app().await;
    let (status, json) = send(
        &app,
        Method::POST,
        "/devices",
        Some(&token),
        Some(json!({
            "device_type": "bomba",
            "fabricante": "B.Braun",
            "modelo": "Perfusor SP",
            "firmware": "2.1",
            "serial": "SN-API-0002"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{json}");
    assert!(json["data"]["id"].as_str().is_some());
}

#[tokio::test]
async fn http_validation_errors() {
    let (app, token, _tmp) = http_app().await;

    // Estado fuera del enum
    let (status, json) = send(
        &app,
        Method::POST,
        "/devices",
        Some(&token),
        Some(json!({
            "device_type": "monitor",
            "fabricante": "X",
            "modelo": "Y",
            "serial": "SN-BAD-1",
            "estado": "apagado_magico"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{json}");
    assert_eq!(json["success"], false);

    // Campo requerido ausente
    let (status, json) = send(
        &app,
        Method::POST,
        "/devices",
        Some(&token),
        Some(json!({ "modelo": "Y" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{json}");
}

#[tokio::test]
async fn http_get_and_heartbeat_unknown_return_404() {
    let (app, token, _tmp) = http_app().await;
    let (status, json) = send(&app, Method::GET, "/devices/no-existe", Some(&token), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{json}");

    let (status, json) = send(
        &app,
        Method::POST,
        "/devices/no-existe/heartbeat",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{json}");
}

#[tokio::test]
async fn http_list_filters_by_estado_and_summary_aggregates() {
    let (app, token, _tmp) = http_app().await;
    for (n, tipo, estado) in [
        ("SN-S1", "monitor", "online"),
        ("SN-S2", "ventilador", "offline"),
        ("SN-S3", "bomba", "mantenimiento"),
        ("SN-S4", "monitor", "online"),
    ] {
        let (status, json) = send(
            &app,
            Method::POST,
            "/devices",
            Some(&token),
            Some(json!({
                "device_type": tipo,
                "fabricante": "Test",
                "modelo": tipo,
                "firmware": "1.0",
                "serial": n,
                "estado": estado
            })),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{json}");
    }

    let (status, json) = send(
        &app,
        Method::GET,
        "/devices?estado=online",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let online = json["data"].as_array().unwrap();
    assert_eq!(online.len(), 2);
    assert!(online.iter().all(|d| d["estado"] == "online"));

    let (status, json) = send(&app, Method::GET, "/devices/status", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    let summary = &json["data"];
    assert_eq!(summary["total"], 4);
    let por_estado = summary["por_estado"].as_array().unwrap();
    let por_estado_total: i64 = por_estado
        .iter()
        .map(|c| c["total"].as_i64().unwrap())
        .sum();
    assert_eq!(por_estado_total, 4);

    let por_tipo = summary["por_tipo"].as_array().unwrap();
    let monitor_count = por_tipo
        .iter()
        .find(|c| c["device_type"] == "monitor")
        .map(|c| c["total"].as_i64().unwrap())
        .unwrap();
    assert_eq!(monitor_count, 2);
}

#[tokio::test]
async fn http_requires_auth() {
    let (app, _token, _tmp) = http_app().await;
    let (status, json) = send(&app, Method::GET, "/devices", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{json}");
}
