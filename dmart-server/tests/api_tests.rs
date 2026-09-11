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
