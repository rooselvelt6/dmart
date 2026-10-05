//! Test distribuido para rate limiting: dos SecurityState con el mismo
//! backend compartido deben compartir la cuota (comportamiento global).
//! La clave debe incluir tenant e IP.

use axum::{
    Router,
    body::Body,
    extract::State,
    http::{Request, StatusCode, header},
    middleware,
    routing::get,
};
use dmart_server::{
    rate_limit_store::{InMemoryFailingStore, RateLimitStore},
    security::{SecurityState, rate_limit_middleware},
};
use std::sync::Arc;
use tower::ServiceExt;

async fn handler() -> &'static str {
    "ok"
}

fn build_app(store: Arc<dyn RateLimitStore>) -> Router {
    let max_rpm: u32 = std::env::var("DMART_TEST_RPM")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2);
    let rl = Arc::new(dmart_server::security::RateLimiter::with_store(
        store.clone(),
        max_rpm,
        60,
    ));
    let lt = Arc::new(dmart_server::security::LoginThrottle::with_store(
        store.clone(),
        5,
        300,
    ));
    let mfa = Arc::new(dmart_server::security::LoginThrottle::with_store(
        store.clone(),
        3,
        300,
    ));
    let state = SecurityState {
        rate_limiter: rl,
        login_throttle: lt,
        mfa_throttle: mfa,
        rate_limit_store: store,
    };
    Router::new()
        .route("/test", get(handler))
        .layer(middleware::from_fn_with_state(state, rate_limit_middleware))
}

#[tokio::test]
async fn test_shared_store_global_quota() {
    let store: Arc<dyn RateLimitStore> = Arc::new(InMemoryFailingStore::new());
    let app1 = build_app(store.clone());
    let app2 = build_app(store.clone());

    // Primera petición desde app1
    let req = Request::builder()
        .uri("/test")
        .header("x-forwarded-for", "1.1.1.1")
        .body(Body::empty())
        .unwrap();
    let resp = app1.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let limit = resp
        .headers()
        .get(header::HeaderName::from_static("x-ratelimit-limit"))
        .unwrap()
        .to_str()
        .unwrap();
    assert_eq!(limit, "2");

    // Segunda desde app2 (misma store compartida) debe consumir cuota
    let req = Request::builder()
        .uri("/test")
        .header("x-forwarded-for", "1.1.1.1")
        .body(Body::empty())
        .unwrap();
    let resp = app2.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let rem = resp
        .headers()
        .get(header::HeaderName::from_static("x-ratelimit-remaining"))
        .unwrap()
        .to_str()
        .unwrap();
    assert_eq!(rem, "0");

    // Tercera debe dar 429 con headers reales
    let req = Request::builder()
        .uri("/test")
        .header("x-forwarded-for", "1.1.1.1")
        .body(Body::empty())
        .unwrap();
    let resp = app2.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(resp.headers().get(header::RETRY_AFTER).is_some());
    assert!(
        resp.headers()
            .get(header::HeaderName::from_static("x-ratelimit-limit"))
            .is_some()
    );
    assert!(
        resp.headers()
            .get(header::HeaderName::from_static("x-ratelimit-remaining"))
            .is_some()
    );
    assert!(
        resp.headers()
            .get(header::HeaderName::from_static("x-ratelimit-reset"))
            .is_some()
    );
}

#[tokio::test]
async fn test_tenant_key_isolates_quota() {
    let store: Arc<dyn RateLimitStore> = Arc::new(InMemoryFailingStore::new());
    let app = build_app(store);

    // tenant A
    let req = Request::builder()
        .uri("/test")
        .header("x-forwarded-for", "2.2.2.2")
        .header(
            "authorization",
            "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJ0ZXN0IiwiYW5kIjoie30iLCJ0ZW5hbnRfaWQiOiJ0ZW5hbnRfYSJ9.invalid",
        )
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // tenant B con misma IP
    let req = Request::builder()
        .uri("/test")
        .header("x-forwarded-for", "2.2.2.2")
        .header(
            "authorization",
            "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJ0ZXN0IiwiYW5kIjoie30iLCJ0ZW5hbnRfaWQiOiJ0ZW5hbnRfYiJ9.invalid",
        )
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // tenant A consume segunda
    let req = Request::builder()
        .uri("/test")
        .header("x-forwarded-for", "2.2.2.2")
        .header(
            "authorization",
            "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJ0ZXN0IiwiYW5kIjoie30iLCJ0ZW5hbnRfaWQiOiJ0ZW5hbnRfYSJ9.invalid",
        )
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK); // rem 0
    let rem = resp
        .headers()
        .get(header::HeaderName::from_static("x-ratelimit-remaining"))
        .unwrap()
        .to_str()
        .unwrap();
    assert_eq!(rem, "0");
}
