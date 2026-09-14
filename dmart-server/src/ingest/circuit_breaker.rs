//! Circuit Breaker para ingest HL7
//! Estado: Closed → Open → HalfOpen → (Closed | Open)

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

impl CircuitState {
    pub fn as_u8(&self) -> u8 {
        match self {
            CircuitState::Closed => 1,
            CircuitState::Open => 2,
            CircuitState::HalfOpen => 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreaker {
    state: CircuitState,
    error_count: u64,
    total_count: u64,
    last_failure: Option<f64>, // Unix timestamp
    // Config
    error_threshold: f64, // 0.0-1.0
    window_seconds: u64,
    half_open_seconds: u64,
    success_threshold: u64, // éxitos consecutivos para cerrar desde half-open
    #[serde(default)]
    consecutive_successes: u64,
}

impl CircuitBreaker {
    /// Crea nuevo circuit breaker
    pub fn new(error_threshold: f64, window_seconds: u64, half_open_seconds: u64) -> Self {
        Self {
            state: CircuitState::Closed,
            error_count: 0,
            total_count: 0,
            last_failure: None,
            error_threshold: error_threshold.clamp(0.0, 1.0),
            window_seconds: window_seconds.max(1),
            half_open_seconds: half_open_seconds.max(1),
            success_threshold: 3, // 3 éxitos consecutivos para cerrar
            consecutive_successes: 0,
        }
    }

    /// Registra resultado de una operación (true = éxito, false = error)
    pub fn record_result(&mut self, success: bool) {
        let now = current_time_secs();

        // Limpiar contadores fuera de ventana
        if let Some(last) = self.last_failure
            && now - last > self.window_seconds as f64
        {
            self.error_count = 0;
            self.total_count = 0;
        }

        self.total_count += 1;
        if !success {
            self.error_count += 1;
            self.last_failure = Some(now);
        }

        // Evaluar transición de estado
        self.evaluate_state(now, success);
    }

    fn evaluate_state(&mut self, now: f64, success: bool) {
        match self.state {
            CircuitState::Closed => {
                if self.total_count >= 10 {
                    // mínimo muestras
                    let error_rate = self.error_count as f64 / self.total_count as f64;
                    if error_rate >= self.error_threshold {
                        self.state = CircuitState::Open;
                    }
                }
            }
            CircuitState::Open => {
                // Verificar si debe pasar a half-open
                if let Some(last) = self.last_failure
                    && now - last >= self.half_open_seconds as f64
                {
                    self.state = CircuitState::HalfOpen;
                }
            }
            CircuitState::HalfOpen => {
                if success {
                    self.consecutive_successes += 1;
                    if self.consecutive_successes >= self.success_threshold {
                        self.state = CircuitState::Closed;
                        self.error_count = 0;
                        self.total_count = 0;
                        self.consecutive_successes = 0;
                    }
                } else {
                    self.state = CircuitState::Open;
                    self.consecutive_successes = 0;
                }
            }
        }
    }

    /// Estado actual
    pub fn state(&self) -> CircuitState {
        self.state
    }

    /// Verifica si debe intentar reset (closed → half-open)
    pub fn should_attempt_reset(&self) -> bool {
        matches!(self.state, CircuitState::Open)
            && self
                .last_failure
                .is_some_and(|last| current_time_secs() - last >= self.half_open_seconds as f64)
    }

    /// Transición manual a half-open (llamado externamente)
    pub fn transition_to_half_open(&mut self) {
        if self.state == CircuitState::Open {
            self.state = CircuitState::HalfOpen;
        }
    }

    /// Métricas para exportación
    pub fn metrics(&self) -> CircuitBreakerMetrics {
        CircuitBreakerMetrics {
            state: self.state,
            error_count: self.error_count,
            total_count: self.total_count,
            error_rate: if self.total_count > 0 {
                self.error_count as f64 / self.total_count as f64
            } else {
                0.0
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct CircuitBreakerMetrics {
    pub state: CircuitState,
    pub error_count: u64,
    pub total_count: u64,
    pub error_rate: f64,
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
    fn test_circuit_breaker_opens_on_error_threshold() {
        let mut cb = CircuitBreaker::new(0.5, 60, 300); // 50% threshold

        // 10 requests, 6 errores = 60% > 50%
        for _ in 0..4 {
            cb.record_result(true); // éxito
        }
        for _ in 0..6 {
            cb.record_result(false); // error
        }

        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[test]
    fn test_circuit_breaker_stays_closed_below_threshold() {
        let mut cb = CircuitBreaker::new(0.5, 60, 300);

        // 10 requests, 4 errores = 40% < 50%
        for _ in 0..6 {
            cb.record_result(true);
        }
        for _ in 0..4 {
            cb.record_result(false);
        }

        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_circuit_breaker_half_open_after_timeout() {
        let mut cb = CircuitBreaker::new(0.5, 60, 1); // half-open después de 1s

        // Forzar open
        for _ in 0..10 {
            cb.record_result(false);
        }
        assert_eq!(cb.state(), CircuitState::Open);

        // Manipular tiempo para simular timeout
        cb.last_failure = Some(current_time_secs() - 2.0);

        // should_attempt_reset debe ser true
        assert!(cb.should_attempt_reset());

        // Transición manual
        cb.transition_to_half_open();
        assert_eq!(cb.state(), CircuitState::HalfOpen);
    }

    #[test]
    fn test_circuit_breaker_half_open_success_closes() {
        let mut cb = CircuitBreaker::new(0.5, 60, 1);

        // Forzar open
        for _ in 0..10 {
            cb.record_result(false);
        }
        cb.last_failure = Some(current_time_secs() - 2.0);
        cb.transition_to_half_open();

        // Éxito en half-open debe cerrar tras success_threshold éxitos consecutivos
        cb.record_result(true);
        assert_eq!(cb.state(), CircuitState::HalfOpen); // 1 de 3
        cb.record_result(true);
        assert_eq!(cb.state(), CircuitState::HalfOpen); // 2 de 3
        cb.record_result(true);
        assert_eq!(cb.state(), CircuitState::Closed); // 3 de 3 → cierra
    }

    #[test]
    fn test_circuit_breaker_half_open_failure_reopens() {
        let mut cb = CircuitBreaker::new(0.5, 60, 1);

        // Forzar open
        for _ in 0..10 {
            cb.record_result(false);
        }
        cb.last_failure = Some(current_time_secs() - 2.0);
        cb.transition_to_half_open();

        // Fallo en half-open debe reabrir
        cb.record_result(false);
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[test]
    fn test_circuit_breaker_serialization() {
        let cb = CircuitBreaker::new(0.5, 60, 300);
        let json = serde_json::to_string(&cb).unwrap();
        let deserialized: CircuitBreaker = serde_json::from_str(&json).unwrap();

        assert_eq!(cb.state, deserialized.state);
        assert_eq!(cb.error_threshold, deserialized.error_threshold);
        assert_eq!(cb.window_seconds, deserialized.window_seconds);
        assert_eq!(cb.half_open_seconds, deserialized.half_open_seconds);
    }

    #[test]
    fn test_circuit_breaker_window_expiry_resets_counters() {
        let mut cb = CircuitBreaker::new(0.5, 1, 300); // ventana 1 segundo

        // Generar errores
        for _ in 0..10 {
            cb.record_result(false);
        }
        assert_eq!(cb.state(), CircuitState::Open);

        // Simular expiración de ventana
        cb.last_failure = Some(current_time_secs() - 2.0);

        // Nuevo éxito debe resetear contadores (ventana expirada)
        cb.record_result(true);
        // Estado puede seguir Open por last_failure, pero contadores reseteados
        assert_eq!(cb.total_count, 1);
        assert_eq!(cb.error_count, 0);
    }
}
