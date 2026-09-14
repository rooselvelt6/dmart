//! Token Bucket Rate Limiter para ingest HL7
//! Thread-safe, serializable para persistencia en Valkey

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Token Bucket algorithm para rate limiting por device
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBucket {
    capacity: f64,    // burst máximo
    tokens: f64,      // tokens actuales
    refill_rate: f64, // tokens por segundo
    last_refill: f64, // timestamp Unix del último refill (segundos)
}

impl TokenBucket {
    /// Crea nuevo bucket con rate (tokens/seg) y burst (capacidad)
    pub fn new(rate_per_second: f64, burst: f64) -> Self {
        Self {
            capacity: burst.max(1.0),
            tokens: burst.max(1.0), // empieza lleno
            refill_rate: rate_per_second.max(0.1),
            last_refill: current_time_secs(),
        }
    }

    /// Intenta consumir `tokens` del bucket
    /// Returns true si había tokens suficientes
    pub fn try_consume(&mut self, tokens: f64) -> bool {
        self.refill();
        if self.tokens >= tokens {
            self.tokens -= tokens;
            true
        } else {
            false
        }
    }

    /// Consume tokens bloqueando hasta que estén disponibles (no usado en ingest)
    #[allow(dead_code)]
    pub async fn consume(&mut self, tokens: f64) {
        loop {
            self.refill();
            if self.tokens >= tokens {
                self.tokens -= tokens;
                return;
            }
            // Calcular tiempo hasta próximo token
            let needed = tokens - self.tokens;
            let wait_secs = needed / self.refill_rate;
            tokio::time::sleep(Duration::from_secs_f64(wait_secs.max(0.001))).await;
        }
    }

    /// Refill tokens basado en tiempo transcurrido
    fn refill(&mut self) {
        let now = current_time_secs();
        let elapsed = now - self.last_refill;
        if elapsed > 0.0 {
            self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.capacity);
            self.last_refill = now;
        }
    }

    /// Tokens disponibles actualmente (después de refill)
    pub fn available(&mut self) -> f64 {
        self.refill();
        self.tokens
    }

    /// Capacidad máxima (burst)
    pub fn capacity(&self) -> f64 {
        self.capacity
    }

    /// Rate de refill (tokens/seg)
    pub fn refill_rate(&self) -> f64 {
        self.refill_rate
    }
}

fn current_time_secs() -> f64 {
    use std::time::SystemTime;
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_bucket_allows_burst_then_throttles() {
        let mut bucket = TokenBucket::new(10.0, 20.0); // 10 rps, burst 20

        // Debe permitir burst completo
        for _ in 0..20 {
            assert!(bucket.try_consume(1.0), "should allow burst");
        }

        // El 21vo debe fallar
        assert!(!bucket.try_consume(1.0), "should throttle after burst");
    }

    #[test]
    fn test_token_bucket_refills_over_time() {
        let mut bucket = TokenBucket::new(10.0, 10.0); // 10 rps, burst 10

        // Consumir todo
        for _ in 0..10 {
            assert!(bucket.try_consume(1.0));
        }
        assert!(!bucket.try_consume(1.0));

        // Simular paso de tiempo (manipulando last_refill)
        bucket.last_refill = current_time_secs() - 1.0; // 1 segundo atrás
        bucket.refill();

        // Debe haber ~10 tokens de nuevo
        assert!(bucket.available() >= 9.0 && bucket.available() <= 10.0);
        assert!(bucket.try_consume(1.0));
    }

    #[test]
    fn test_token_bucket_never_exceeds_capacity() {
        let mut bucket = TokenBucket::new(100.0, 50.0); // rate alto, burst 50

        // Simular mucho tiempo sin uso
        bucket.last_refill = current_time_secs() - 1000.0;
        bucket.refill();

        assert_eq!(bucket.available(), bucket.capacity());
    }

    #[test]
    fn test_token_bucket_serialization() {
        let bucket = TokenBucket::new(10.0, 20.0);
        let json = serde_json::to_string(&bucket).unwrap();
        let deserialized: TokenBucket = serde_json::from_str(&json).unwrap();

        assert_eq!(bucket.capacity, deserialized.capacity);
        assert_eq!(bucket.refill_rate, deserialized.refill_rate);
        // tokens y last_refill pueden diferir ligeramente por tiempo
    }
}
