//! Security Middleware - Rate Limiting & Protection
//!
//! Implements protection against:
//! - DDoS attacks (rate limiting)
//! - Brute force (login throttling)
//! - Clickjacking (X-Frame-Options)

use axum::{
    Json,
    extract::{ConnectInfo, Request, State},
    http::{HeaderValue, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use dmart_shared::models::ApiResponse;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Combined security state for middleware
#[derive(Clone)]
pub struct SecurityState {
    pub rate_limiter: Arc<RateLimiter>,
    pub login_throttle: Arc<LoginThrottle>,
    /// Dedicated throttle for the MFA challenge flow (`/auth/mfa/verify`).
    /// Stricter than the generic login throttle to prevent TOTP brute-force.
    pub mfa_throttle: Arc<LoginThrottle>,
}

/// Rate Limiter using in-memory sliding window (backup for Valkey)
pub struct RateLimiter {
    requests: Arc<RwLock<HashMap<String, Vec<Instant>>>>,
    max_requests: u32,
    window_secs: u64,
}

impl RateLimiter {
    pub fn new(max_requests: u32, window_secs: u64) -> Self {
        RateLimiter {
            requests: Arc::new(RwLock::new(HashMap::new())),
            max_requests,
            window_secs,
        }
    }

    pub async fn check(&self, key: &str) -> bool {
        let now = Instant::now();
        let window = Duration::from_secs(self.window_secs);
        let mut requests = self.requests.write().await;

        requests.retain(|_, times| times.iter().any(|t| now.duration_since(*t) < window));

        let entry = requests.entry(key.to_string()).or_insert_with(Vec::new);

        entry.retain(|t| now.duration_since(*t) < window);

        if entry.len() >= self.max_requests as usize {
            return false;
        }

        entry.push(now);
        true
    }
}

/// Login throttle tracker (brute force protection)
type AttemptMap = HashMap<String, (u32, Option<Instant>)>;

pub struct LoginThrottle {
    attempts: Arc<RwLock<AttemptMap>>,
    max_attempts: u32,
    lockout_secs: u64,
}

impl LoginThrottle {
    pub fn new(max_attempts: u32, lockout_secs: u64) -> Self {
        LoginThrottle {
            attempts: Arc::new(RwLock::new(HashMap::new())),
            max_attempts,
            lockout_secs,
        }
    }

    pub async fn record_failure(&self, key: &str) -> bool {
        let mut attempts = self.attempts.write().await;
        let count = attempts
            .entry(key.to_string())
            .or_insert_with(|| (0u32, None));
        count.0 += 1;

        if count.0 >= self.max_attempts {
            count.1 = Some(Instant::now());
            return true;
        }
        false
    }

    #[allow(dead_code)]
    pub async fn record_success(&self, key: &str) {
        let mut attempts = self.attempts.write().await;
        attempts.remove(key);
    }

    pub async fn is_locked(&self, key: &str) -> Option<u64> {
        let attempts = self.attempts.read().await;
        if let Some((_, Some(locked_at))) = attempts.get(key) {
            let elapsed = locked_at.elapsed().as_secs();
            if elapsed < self.lockout_secs {
                return Some(self.lockout_secs - elapsed);
            }
        }
        None
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
                connect-src 'self' https://fonts.googleapis.com; \
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

    if state.rate_limiter.check(&key).await {
        let mut res = next.run(req).await;
        res.headers_mut().insert(
            header::HeaderName::from_static("x-ratelimit-remaining"),
            HeaderValue::from_static("1"),
        );
        res
    } else {
        (
            StatusCode::TOO_MANY_REQUESTS,
            [(header::RETRY_AFTER, "60")],
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
    client_ip(req).unwrap_or_else(|| "unknown".to_string())
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

/// Key del throttle: IP + (para login) username del body, (para MFA) subject
/// del reto. Así un bot distribuido no puede esquivar el lockout rotando IPs y
/// un mismo usuario no bloquea a toda la NAT.
fn throttle_key(req: &Request, path: &str, ip: &str, body: &[u8]) -> String {
    if path.ends_with("/auth/login") {
        if let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) {
            if let Some(u) = value.get("username").and_then(|v| v.as_str()) {
                let u = u.trim().to_lowercase();
                if !u.is_empty() {
                    return format!("{}|{}", ip, u);
                }
            }
        }
    } else if path.ends_with("/auth/mfa/verify") {
        if let Some(sub) = bearer_sub(req) {
            return format!("{}|{}", ip, sub);
        }
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
pub fn create_security_state() -> SecurityState {
    // 100 requests per minute for general endpoints
    let rate_limiter = Arc::new(RateLimiter::new(100, 60));
    // 5 failed logins before 5 minute lockout
    let login_throttle = Arc::new(LoginThrottle::new(5, 300));
    // 3 failed TOTP codes before 5 minute lockout (brute-force protection)
    let mfa_throttle = Arc::new(LoginThrottle::new(3, 300));

    SecurityState {
        rate_limiter,
        login_throttle,
        mfa_throttle,
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
        let limiter = RateLimiter::new(3, 60);

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
        let throttle = LoginThrottle::new(3, 60);

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

    #[test]
    fn test_mfa_throttle_is_stricter_than_login() {
        let state = create_security_state();
        // 3 failed attempts allowed for the MFA challenge flow
        assert_eq!(state.mfa_throttle.max_attempts, 3);
        // while the generic login throttle allows 5
        assert_eq!(state.login_throttle.max_attempts, 5);
    }
}
