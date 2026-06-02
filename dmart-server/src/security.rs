//! Security Middleware - Rate Limiting & Protection
//! 
//! Implements protection against:
//! - DDoS attacks (rate limiting)
//! - Brute force (login throttling)
//! - Clickjacking (X-Frame-Options)

use axum::{
    extract::{Request, State},
    http::{HeaderValue, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use std::collections::HashMap;

/// Combined security state for middleware
#[derive(Clone)]
pub struct SecurityState {
    pub rate_limiter: Arc<RateLimiter>,
    pub login_throttle: Arc<LoginThrottle>,
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
        
        requests.retain(|_, times| {
            times.iter().any(|t| now.duration_since(*t) < window)
        });
        
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
pub struct LoginThrottle {
    attempts: Arc<RwLock<HashMap<String, (u32, Option<Instant>)>>>,
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
        let count = attempts.entry(key.to_string()).or_insert_with(|| (0u32, None));
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
    vec![
        (header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY")),
        (header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff")),
        (
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(
                "default-src 'self'; \
                script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval' https://cdnjs.cloudflare.com; \
                style-src 'self' 'unsafe-inline' https://fonts.googleapis.com https://cdnjs.cloudflare.com; \
                font-src 'self' https://fonts.gstatic.com https://cdnjs.cloudflare.com data:; \
                img-src 'self' data:; \
                connect-src 'self' https://fonts.googleapis.com; \
                frame-ancestors 'none';"
            ),
        ),
        (header::HeaderName::from_static("x-xss-protection"), HeaderValue::from_static("1; mode=block")),
        (header::HeaderName::from_static("referrer-policy"), HeaderValue::from_static("strict-origin-when-cross-origin")),
        (header::HeaderName::from_static("permissions-policy"), HeaderValue::from_static("camera=(), microphone=(), geolocation=()")),
        (header::HeaderName::from_static("cross-origin-opener-policy"), HeaderValue::from_static("same-origin")),
    ]
}

/// Middleware that applies all security headers to every response
pub async fn security_headers_middleware(
    request: Request,
    next: Next,
) -> Response {
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
        ).into_response()
    }
}

fn get_client_key(req: &Request) -> String {
    req.headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or(s).to_string())
        .or_else(|| {
            req.headers()
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "unknown".to_string())
}

/// Middleware for login throttling
pub async fn login_throttle_middleware(
    State(state): State<SecurityState>,
    req: Request,
    next: Next,
) -> Response {
    let key = get_client_key(&req);
    
    if let Some(remaining) = state.login_throttle.is_locked(&key).await {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            [(header::RETRY_AFTER, &remaining.to_string())],
            format!("Account temporarily locked. Try again in {} seconds.", remaining),
        ).into_response();
    }
    
    let res = next.run(req).await;
    
    if res.status() == StatusCode::UNAUTHORIZED {
        if state.login_throttle.record_failure(&key).await {
            tracing::warn!("Login throttle triggered for IP: {}", key);
            return (
                StatusCode::TOO_MANY_REQUESTS,
                [(header::RETRY_AFTER, "300")],
                "Too many failed attempts. Account locked for 5 minutes.",
            ).into_response();
        }
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
        .replace(';', "")
        .replace('\\', "")
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

/// Create global security state
pub fn create_security_state() -> SecurityState {
    // 100 requests per minute for general endpoints
    let rate_limiter = Arc::new(RateLimiter::new(100, 60));
    // 5 failed logins before 5 minute lockout
    let login_throttle = Arc::new(LoginThrottle::new(5, 300));
    
    SecurityState { rate_limiter, login_throttle }
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
        assert_eq!(escape_html("<script>alert(1)</script>"), "&lt;script&gt;alert(1)&lt;/script&gt;");
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
}