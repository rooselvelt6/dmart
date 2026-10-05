//! Security Middleware - Rate Limiting & Protection
//!
//! Implements protection against:
//! - DDoS attacks (rate limiting)
//! - Brute force (login throttling)
//! - Clickjacking (X-Frame-Options)

use crate::rate_limit_store::{RateLimitStore, create_rate_limit_store};
use axum::{
    Json,
    extract::{ConnectInfo, Request, State},
    http::{HeaderValue, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use base64::Engine;
use dmart_shared::models::ApiResponse;
use std::net::SocketAddr;
use std::sync::Arc;

/// Combined security state for middleware
#[derive(Clone)]
pub struct SecurityState {
    pub rate_limiter: Arc<RateLimiter>,
    pub login_throttle: Arc<LoginThrottle>,
    /// Dedicated throttle for the MFA challenge flow (`/auth/mfa/verify`).
    /// Stricter than the generic login throttle to prevent TOTP brute-force.
    pub mfa_throttle: Arc<LoginThrottle>,
    /// Distributed rate limit store (Valkey/Redis or in-memory for tests)
    pub rate_limit_store: Arc<dyn RateLimitStore>,
}

/// Rate Limiter using distributed store (Valkey/Redis or in-memory)
pub struct RateLimiter {
    store: Arc<dyn RateLimitStore>,
    max_requests: u32,
    window_secs: u64,
}

impl RateLimiter {
    pub fn new(max_requests: u32, window_secs: u64) -> Self {
        // Legacy constructor for backward compatibility (in-memory only)
        Self::with_store(
            Arc::new(crate::rate_limit_store::InMemoryFailingStore::new()),
            max_requests,
            window_secs,
        )
    }

    pub fn with_store(store: Arc<dyn RateLimitStore>, max_requests: u32, window_secs: u64) -> Self {
        RateLimiter {
            store,
            max_requests,
            window_secs,
        }
    }

    pub async fn check(&self, key: &str) -> bool {
        match self.store.increment(key, self.window_secs).await {
            Ok((count, _)) => count <= self.max_requests,
            Err(_) => false, // Fail closed
        }
    }
}

/// Login throttle tracker (brute force protection) using distributed store
pub struct LoginThrottle {
    store: Arc<dyn RateLimitStore>,
    max_attempts: u32,
    lockout_secs: u64,
}

impl LoginThrottle {
    pub fn new(max_attempts: u32, lockout_secs: u64) -> Self {
        Self::with_store(
            Arc::new(crate::rate_limit_store::InMemoryFailingStore::new()),
            max_attempts,
            lockout_secs,
        )
    }

    pub fn with_store(
        store: Arc<dyn RateLimitStore>,
        max_attempts: u32,
        lockout_secs: u64,
    ) -> Self {
        LoginThrottle {
            store,
            max_attempts,
            lockout_secs,
        }
    }

    pub async fn record_failure(&self, key: &str) -> bool {
        match self.store.increment(key, self.lockout_secs).await {
            Ok((count, _)) => count >= self.max_attempts,
            Err(_) => false,
        }
    }

    #[allow(dead_code)]
    pub async fn record_success(&self, key: &str) {
        let _ = self.store.reset(key).await;
    }

    pub async fn is_locked(&self, key: &str) -> Option<u64> {
        match self.store.get(key).await {
            Ok(Some((count, ttl_remaining))) if count >= self.max_attempts => Some(ttl_remaining),
            _ => None,
        }
    }
}

/// All security headers as key-value pairs
pub fn security_headers() -> Vec<(header::HeaderName, HeaderValue)> {
    let mut headers = vec![
        (header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY")),
        (
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ),
        (
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(
                "default-src 'self'; \
                script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval' https://cdnjs.cloudflare.com; \
                style-src 'self' 'unsafe-inline' https://fonts.googleapis.com https://cdnjs.cloudflare.com; \
                font-src 'self' https://fonts.gstatic.com https://cdnjs.cloudflare.com data:; \
                img-src 'self' data:; \
                connect-src 'self' https://fonts.googleapis.com ws: wss: chrome-extension:; \
                frame-ancestors 'none';",
            ),
        ),
        (
            header::HeaderName::from_static("x-xss-protection"),
            HeaderValue::from_static("1; mode=block"),
        ),
        (
            header::HeaderName::from_static("referrer-policy"),
            HeaderValue::from_static("strict-origin-when-cross-origin"),
        ),
        (
            header::HeaderName::from_static("permissions-policy"),
            HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
        ),
        (
            header::HeaderName::from_static("cross-origin-opener-policy"),
            HeaderValue::from_static("same-origin"),
        ),
    ];

    // Strict-Transport-Security only makes sense over HTTPS; disable with DMART_ENABLE_HSTS=false
    let hsts_enabled = std::env::var("DMART_ENABLE_HSTS")
        .map(|v| v != "false")
        .unwrap_or(true);
    if hsts_enabled {
        headers.push((
            header::STRICT_TRANSPORT_SECURITY,
            HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        ));
    }

    headers
}

/// Middleware that applies all security headers to every response
pub async fn security_headers_middleware(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = security_headers();
    for (name, value) in headers {
        response.headers_mut().insert(name, value);
    }
    response
}

/// Middleware to apply rate limiting
pub async fn rate_limit_middleware(
    State(state): State<SecurityState>,
    req: Request,
    next: Next,
) -> Response {
    let key = get_client_key(&req);

    // Get current count from store to return proper headers
    let (allowed, limit, remaining, reset) = match state
        .rate_limit_store
        .increment(&key, state.rate_limiter.window_secs)
        .await
    {
        Ok((count, ttl_remaining)) => {
            let allowed = count <= state.rate_limiter.max_requests;
            let remaining = state.rate_limiter.max_requests.saturating_sub(count);
            (
                allowed,
                state.rate_limiter.max_requests,
                remaining,
                ttl_remaining,
            )
        }
        Err(_) => (false, state.rate_limiter.max_requests, 0, 60), // Fail closed
    };

    if allowed {
        let mut res = next.run(req).await;
        res.headers_mut().insert(
            header::HeaderName::from_static("x-ratelimit-limit"),
            HeaderValue::from_str(&limit.to_string()).unwrap(),
        );
        res.headers_mut().insert(
            header::HeaderName::from_static("x-ratelimit-remaining"),
            HeaderValue::from_str(&remaining.to_string()).unwrap(),
        );
        res.headers_mut().insert(
            header::HeaderName::from_static("x-ratelimit-reset"),
            HeaderValue::from_str(&reset.to_string()).unwrap(),
        );
        res
    } else {
        (
            StatusCode::TOO_MANY_REQUESTS,
            [
                (header::RETRY_AFTER, HeaderValue::from_static("60")),
                (
                    header::HeaderName::from_static("x-ratelimit-limit"),
                    HeaderValue::from_str(&limit.to_string()).unwrap(),
                ),
                (
                    header::HeaderName::from_static("x-ratelimit-remaining"),
                    HeaderValue::from_static("0"),
                ),
                (
                    header::HeaderName::from_static("x-ratelimit-reset"),
                    HeaderValue::from_str(&reset.to_string()).unwrap(),
                ),
            ],
            "Rate limit exceeded. Try again later.",
        )
            .into_response()
    }
}

/// Determina si `peer` (IP origen del socket) es un proxy de confianza.
///
/// El header `X-Forwarded-For` SOLO se honra cuando:
/// - `DMART_TRUST_PROXY=true`, y
/// - la conexión directa proviene de un peer en `DMART_TRUSTED_PROXIES` (CSV)
///   o, si tal variable no existe, de loopback (proxy en el mismo host).
///
/// Sin esto, cualquier cliente que alcance el servidor directamente podría
/// espoofear `X-Forwarded-For` para evadir el rate limiter y el login throttle.
fn peer_is_trusted_proxy(peer: Option<&str>) -> bool {
    let trust_proxy = std::env::var("DMART_TRUST_PROXY")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);
    if !trust_proxy {
        return false;
    }
    let configured: std::collections::HashSet<String> = std::env::var("DMART_TRUSTED_PROXIES")
        .map(|v| {
            v.split(',')
                .map(|s: &str| s.trim().to_string())
                .filter(|s: &String| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();
    if configured.is_empty() {
        // Fail-safe: sin lista explícita solo se confía en loopback.
        return peer.is_some_and(|p| ["127.0.0.1", "::1", "localhost"].contains(&p));
    }
    peer.is_some_and(|p| configured.contains(p))
}

/// Real client IP from the socket, unless the request arrives from a trusted
/// reverse proxy (`DMART_TRUST_PROXY=true` + peer en `DMART_TRUSTED_PROXIES` o
/// loopback) that overwrites `X-Forwarded-For`. Never trusts the header without
/// trusting the direct peer, otherwise the rate limiter can be bypassed.
fn client_ip(req: &Request) -> Option<String> {
    let peer = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ci| ci.0.ip().to_string());

    if peer_is_trusted_proxy(peer.as_deref())
        && let Some(forwarded) = req
            .headers()
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
    {
        return Some(
            forwarded
                .split(',')
                .next()
                .unwrap_or(forwarded)
                .trim()
                .to_string(),
        );
    }

    peer
}

fn get_client_key(req: &Request) -> String {
    let ip = client_ip(req).unwrap_or_else(|| "unknown".to_string());
    // Si hay un tenant en el token, usarlo para aislar la cuota por tenant
    if let Some(tenant) = bearer_tenant(req) {
        format!("{}|{}", tenant, ip)
    } else {
        ip
    }
}

/// Decodifica el `sub` (user_id) de un token Bearer sin tocar la BD, para
/// granular el throttle MFA por usuario (además de por IP).
fn bearer_sub(req: &Request) -> Option<String> {
    let header = req.headers().get("authorization")?.to_str().ok()?;
    let token = crate::auth::extract_token_from_header(header)?;
    match jsonwebtoken::decode::<crate::auth::Claims>(
        token,
        &jsonwebtoken::DecodingKey::from_secret(crate::auth::jwt_secret_bytes()),
        &jsonwebtoken::Validation::default(),
    ) {
        Ok(data) => Some(data.claims.sub),
        Err(_) => None,
    }
}

/// Extrae el `tenant_id` del token Bearer (claim `tenant_id`).
/// No verifica la firma: solo extrae el claim para rate limiting.
fn bearer_tenant(req: &Request) -> Option<String> {
    let header = req.headers().get("authorization")?.to_str().ok()?;
    let token = crate::auth::extract_token_from_header(header)?;
    // Decode without verification for rate limit key extraction
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    let payload = parts[1];
    // Add padding if needed
    let payload = payload.to_string() + &"=".repeat((4 - payload.len() % 4) % 4);
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .ok()?;
    let claims: serde_json::Value = serde_json::from_slice(&decoded).ok()?;
    let tid = claims.get("tenant_id")?.as_str()?;
    if tid == "default" || tid.is_empty() {
        None
    } else {
        Some(tid.to_string())
    }
}

/// Key del throttle: IP + (para login) username del body, (para MFA) subject
/// del reto. Así un bot distribuido no puede esquivar el lockout rotando IPs y
/// un mismo usuario no bloquea a toda la NAT.
fn throttle_key(req: &Request, path: &str, ip: &str, body: &[u8]) -> String {
    if path.ends_with("/auth/login") {
        let username = serde_json::from_slice::<serde_json::Value>(body)
            .ok()
            .and_then(|v| {
                v.get("username")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned)
            });
        if let Some(u) = username {
            let u = u.trim().to_lowercase();
            if !u.is_empty() {
                return format!("{}|{}", ip, u);
            }
        }
    } else if path.ends_with("/auth/mfa/verify")
        && let Some(sub) = bearer_sub(req)
    {
        return format!("{}|{}", ip, sub);
    }
    ip.to_string()
}

/// Middleware for login throttling
///
/// Solo se cuenta un fallo (401) y únicamente en las rutas `/auth/*`; si una
/// cuenta se bloquea, el lockout afecta a esa combinación IP+usuario (login) o
/// IP+user_id (MFA), no a toda la NAT. El flujo MFA (`/auth/mfa/verify`) tiene
/// su propio throttle más estricto (3 intentos / 5 min) para que un código TOTP
/// no pueda brute-forcearse ni repitiendo el reto.
pub async fn login_throttle_middleware(
    State(state): State<SecurityState>,
    req: Request,
    next: Next,
) -> Response {
    let path = req.uri().path().to_string();
    let is_mfa_verify = path.ends_with("/auth/mfa/verify");
    let is_auth_path = path.starts_with("/auth/");
    let throttle = if is_mfa_verify {
        &state.mfa_throttle
    } else {
        &state.login_throttle
    };

    // Leer el body de una vez para poder granular la key por username (login).
    let (parts, body) = req.into_parts();
    let bytes = axum::body::to_bytes(body, 16 * 1024)
        .await
        .unwrap_or_default();
    let req = Request::from_parts(parts, axum::body::Body::from(bytes.clone()));

    let key = {
        let ip = get_client_key(&req);
        if is_auth_path {
            throttle_key(&req, &path, &ip, &bytes)
        } else {
            ip
        }
    };

    if let Some(remaining) = throttle.is_locked(&key).await {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            [(header::CONTENT_TYPE, "application/json")],
            Json(ApiResponse::<()>::err(format!(
                "Account temporarily locked. Try again in {} seconds.",
                remaining
            ))),
        )
            .into_response();
    }

    let res = next.run(req).await;

    // Solo cuentan los 401 reales de autenticación (`/auth/*`). Un 401 generado
    // por otro endpoint no debe poder quebrar la IP de la víctima.
    if is_auth_path
        && res.status() == StatusCode::UNAUTHORIZED
        && throttle.record_failure(&key).await
    {
        tracing::warn!("Login throttle triggered for {}", key);
        return (
            StatusCode::TOO_MANY_REQUESTS,
            [(header::CONTENT_TYPE, "application/json")],
            Json(ApiResponse::<()>::err(
                "Too many failed attempts. Account locked for 5 minutes.",
            )),
        )
            .into_response();
    }

    if is_mfa_verify && res.status().is_success() {
        throttle.record_success(&key).await;
        state.login_throttle.record_success(&key).await;
    }

    res
}

/// Sanitize user input - prevent header/log injection
#[allow(dead_code)]
pub fn sanitize_input(input: &str) -> String {
    input
        .chars()
        .filter(|c| {
            c.is_alphanumeric()
                || c.is_whitespace()
                || matches!(c, '.' | ',' | '-' | '_' | '@' | '/' | ':')
        })
        .collect()
}

/// Sanitize for SQL-like injection (extra safety for SurrealDB)
#[allow(dead_code)]
pub fn sanitize_for_query(input: &str) -> String {
    input
        .replace('\'', "\\'")
        .replace('"', "\\\"")
        .replace([';', '\\'], "")
}

/// Escape HTML entities to prevent XSS in user-provided strings
#[allow(dead_code)]
pub fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

/// Mensaje genérico para errores internos (base de datos, SurrealDB internals,
/// etc.). El detalle real se registra en los logs y NO se expone al cliente
/// HTTP para no filtrar el modelo de datos ni la infraestructura.
pub fn sanitize_internal_error(detail: &dyn std::fmt::Display) -> String {
    tracing::error!("Internal error (ocultado al cliente): {}", detail);
    "Error interno del servidor".to_string()
}

/// Create global security state
pub async fn create_security_state() -> SecurityState {
    // Crear store distribuido (Valkey/Redis o en memoria)
    let rate_limit_store = create_rate_limit_store().await;

    // Requests por ventana para endpoints generales. El valor por defecto es
    // conservador (100 rpm) y solo se sube explícitamente: el job de load test
    // necesita declarar el sobre que mide, y con el límite de producción
    // cualquier carga devolvería 429 en vez de medir nada.
    let max_requests: u32 = std::env::var("DMART_RATE_LIMIT_RPM")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(100);
    let window_secs: u64 = std::env::var("DMART_RATE_LIMIT_WINDOW_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60);

    // Disable rate limiter in dev mode if explicitly set
    let disable_rate_limit = std::env::var("DMART_DISABLE_RATE_LIMIT")
        .ok()
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(false);
    let rate_limiter = if disable_rate_limit {
        Arc::new(RateLimiter::with_store(
            rate_limit_store.clone(),
            u32::MAX,
            1,
        ))
    } else {
        Arc::new(RateLimiter::with_store(
            rate_limit_store.clone(),
            max_requests,
            window_secs,
        ))
    };

    // Login throttle configuration
    // DMART_DISABLE_LOGIN_THROTTLE=true -> disables login throttle entirely
    // DMART_LOGIN_THROTTLE_MAX_ATTEMPTS=N -> max failed attempts before lockout (default 5, 0 = disabled)
    // DMART_LOGIN_THROTTLE_LOCKOUT_SECS=N -> lockout duration in seconds (default 300)
    let disable_login_throttle = std::env::var("DMART_DISABLE_LOGIN_THROTTLE")
        .ok()
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(false);
    let login_max_attempts: u32 = std::env::var("DMART_LOGIN_THROTTLE_MAX_ATTEMPTS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);
    let login_lockout_secs: u64 = std::env::var("DMART_LOGIN_THROTTLE_LOCKOUT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(300);

    let login_throttle = if disable_login_throttle || login_max_attempts == 0 {
        Arc::new(LoginThrottle::with_store(
            rate_limit_store.clone(),
            u32::MAX,
            1,
        ))
    } else {
        Arc::new(LoginThrottle::with_store(
            rate_limit_store.clone(),
            login_max_attempts,
            login_lockout_secs,
        ))
    };

    // MFA throttle configuration
    // DMART_DISABLE_MFA_THROTTLE=true -> disables MFA throttle
    // DMART_MFA_THROTTLE_MAX_ATTEMPTS=N -> max failed attempts (default 3, 0 = disabled)
    // DMART_MFA_THROTTLE_LOCKOUT_SECS=N -> lockout duration (default 300)
    let disable_mfa_throttle = std::env::var("DMART_DISABLE_MFA_THROTTLE")
        .ok()
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(false);
    let mfa_max_attempts: u32 = std::env::var("DMART_MFA_THROTTLE_MAX_ATTEMPTS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3);
    let mfa_lockout_secs: u64 = std::env::var("DMART_MFA_THROTTLE_LOCKOUT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(300);

    let mfa_throttle = if disable_mfa_throttle || mfa_max_attempts == 0 {
        Arc::new(LoginThrottle::with_store(
            rate_limit_store.clone(),
            u32::MAX,
            1,
        ))
    } else {
        Arc::new(LoginThrottle::with_store(
            rate_limit_store.clone(),
            mfa_max_attempts,
            mfa_lockout_secs,
        ))
    };

    SecurityState {
        rate_limiter,
        login_throttle,
        mfa_throttle,
        rate_limit_store,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_input() {
        assert_eq!(sanitize_input("John Doe"), "John Doe");
        assert_eq!(sanitize_input("'; DROP TABLE--"), " DROP TABLE--");
        assert_eq!(sanitize_input("test@test.com"), "test@test.com");
    }

    #[test]
    fn test_escape_html() {
        assert_eq!(
            escape_html("<script>alert(1)</script>"),
            "&lt;script&gt;alert(1)&lt;/script&gt;"
        );
        assert_eq!(escape_html("A & B"), "A &amp; B");
    }

    #[tokio::test]
    async fn test_rate_limiter() {
        let store = Arc::new(crate::rate_limit_store::InMemoryFailingStore::new());
        let limiter = RateLimiter::with_store(store, 3, 60);

        // First 3 should pass
        assert!(limiter.check("test_ip").await);
        assert!(limiter.check("test_ip").await);
        assert!(limiter.check("test_ip").await);

        // 4th should fail
        assert!(!limiter.check("test_ip").await);

        // Different IP should pass
        assert!(limiter.check("other_ip").await);
    }

    #[tokio::test]
    async fn test_login_throttle() {
        let store = Arc::new(crate::rate_limit_store::InMemoryFailingStore::new());
        let throttle = LoginThrottle::with_store(store, 3, 60);

        // Record 2 failures
        assert!(!throttle.record_failure("user1").await);
        assert!(!throttle.record_failure("user1").await);

        // 3rd should trigger lockout
        assert!(throttle.record_failure("user1").await);
        assert!(throttle.is_locked("user1").await.is_some());

        // Success clears
        throttle.record_success("user1").await;
        assert!(throttle.is_locked("user1").await.is_none());
    }

    #[tokio::test]
    async fn test_mfa_throttle_is_stricter_than_login() {
        // Test with default values
        // SAFETY: el binario de test es el único que fija estas variables de
        // entorno, siempre al mismo valor, así que no hay carrera observable
        // con los tests paralelos que leen la configuración.
        unsafe {
            std::env::set_var("DMART_DISABLE_LOGIN_THROTTLE", "false");
            std::env::set_var("DMART_LOGIN_THROTTLE_MAX_ATTEMPTS", "5");
            std::env::set_var("DMART_LOGIN_THROTTLE_LOCKOUT_SECS", "300");
            std::env::set_var("DMART_DISABLE_MFA_THROTTLE", "false");
            std::env::set_var("DMART_MFA_THROTTLE_MAX_ATTEMPTS", "3");
            std::env::set_var("DMART_MFA_THROTTLE_LOCKOUT_SECS", "300");
        }

        let state = create_security_state().await;
        // 3 failed attempts allowed for the MFA challenge flow
        assert_eq!(state.mfa_throttle.max_attempts, 3);
        // while the generic login throttle allows 5
        assert_eq!(state.login_throttle.max_attempts, 5);
    }
}
