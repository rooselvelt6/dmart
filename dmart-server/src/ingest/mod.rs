//! Ingest Hardening — SPEC-031
//! Rate limiting, circuit breaker, gap detection, data quality for HL7 MLLP ingest

pub mod circuit_breaker;
pub mod gap_detector;
pub mod quality;
pub mod rate_limit;

use std::sync::Arc;
use tokio::sync::RwLock;

use crate::cache::{cache_available, cache_get, cache_set};

/// Configuración global de hardening ingest (desde env vars)
#[derive(Debug, Clone)]
pub struct IngestConfig {
    // Rate limit (token bucket)
    pub rate_limit_rps: f64,
    pub rate_limit_burst: f64,
    // Circuit breaker
    pub cb_error_threshold: f64,
    pub cb_window_seconds: u64,
    pub cb_half_open_seconds: u64,
    // Gap detection
    pub gap_detection_enabled: bool,
    // Frame limits
    pub max_frame_bytes: usize,
    // Auto-throttle
    pub auto_throttle_enabled: bool,
    pub cpu_threshold: f64,
    pub mem_threshold: f64,
}

impl Default for IngestConfig {
    fn default() -> Self {
        Self {
            rate_limit_rps: std::env::var("DMART_INGEST_RATE_LIMIT_RPS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10.0),
            rate_limit_burst: std::env::var("DMART_INGEST_RATE_LIMIT_BURST")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(20.0),
            cb_error_threshold: std::env::var("DMART_INGEST_CB_ERROR_THRESHOLD")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.5),
            cb_window_seconds: std::env::var("DMART_INGEST_CB_WINDOW_SECONDS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(60),
            cb_half_open_seconds: std::env::var("DMART_INGEST_CB_HALF_OPEN_SECONDS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(300),
            gap_detection_enabled: std::env::var("DMART_INGEST_GAP_DETECTION")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
            max_frame_bytes: std::env::var("DMART_INGEST_MAX_FRAME_BYTES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1_048_576),
            auto_throttle_enabled: std::env::var("DMART_INGEST_AUTO_THROTTLE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
            cpu_threshold: std::env::var("DMART_INGEST_CPU_THRESHOLD")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(80.0),
            mem_threshold: std::env::var("DMART_INGEST_MEM_THRESHOLD")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(90.0),
        }
    }
}

/// Estado agregado de ingest para métricas
#[derive(Debug, Default, Clone)]
pub struct IngestMetrics {
    pub throttled_total: u64,
    pub gaps_total: u64,
    pub invalid_total: u64,
    pub error_avg: f64,
    pub fault_devices: u64,
}

/// Estado global de ingest (compartido entre conexiones MLLP)
#[derive(Debug)]
pub struct IngestState {
    config: IngestConfig,
    metrics: Arc<RwLock<IngestMetrics>>,
    // Device states stored in Valkey with local cache
    // Local cache: device_id -> DeviceState
    pub devices: Arc<RwLock<dashmap::DashMap<String, DeviceState>>>,
}

#[derive(Debug, Clone)]
pub struct DeviceState {
    pub rate_limiter: rate_limit::TokenBucket,
    pub circuit_breaker: circuit_breaker::CircuitBreaker,
    pub gap_detector: gap_detector::GapDetector,
    pub last_seen: std::time::Instant,
}

impl IngestState {
    pub fn new(config: IngestConfig) -> Self {
        Self {
            config,
            metrics: Arc::new(RwLock::new(IngestMetrics::default())),
            devices: Arc::new(RwLock::new(dashmap::DashMap::new())),
        }
    }

    pub fn config(&self) -> &IngestConfig {
        &self.config
    }

    pub async fn metrics(&self) -> IngestMetrics {
        self.metrics.read().await.clone()
    }

    /// Obtiene o crea estado para un device (con carga desde Valkey)
    pub async fn get_device_state(&self, device_id: &str) -> DeviceState {
        let devices = self.devices.read().await;
        if let Some(state) = devices.get(device_id) {
            return state.clone();
        }
        drop(devices);

        // Cargar desde Valkey
        let state = self
            .load_from_valkey(device_id)
            .await
            .unwrap_or_else(|| DeviceState::new(&self.config));

        // Guardar en cache local
        let devices = self.devices.read().await;
        devices.insert(device_id.to_string(), state.clone());
        state
    }

    async fn load_from_valkey(&self, device_id: &str) -> Option<DeviceState> {
        if !cache_available() {
            return None;
        }

        // Cargar rate limiter
        let rl_key = format!("ingest:rate_limit:{}", device_id);
        let cb_key = format!("ingest:circuit_breaker:{}", device_id);
        let seq_key = format!("ingest:sequence:{}", device_id);

        let rl_data = cache_get(&rl_key).await?;
        let cb_data = cache_get(&cb_key).await?;
        let seq_data = cache_get(&seq_key).await?;

        // Deserializar (simplificado: JSON)
        let rate_limiter: rate_limit::TokenBucket = serde_json::from_str(&rl_data).ok()?;
        let circuit_breaker: circuit_breaker::CircuitBreaker =
            serde_json::from_str(&cb_data).ok()?;
        let last_sequence: u32 = serde_json::from_str(&seq_data).ok()?;

        Some(DeviceState {
            rate_limiter,
            circuit_breaker,
            gap_detector: gap_detector::GapDetector::with_sequence(last_sequence),
            last_seen: std::time::Instant::now(),
        })
    }

    /// Persiste estado de un device a Valkey
    pub async fn persist_device_state(&self, device_id: &str, state: &DeviceState) {
        if !cache_available() {
            return;
        }

        let rl_key = format!("ingest:rate_limit:{}", device_id);
        let cb_key = format!("ingest:circuit_breaker:{}", device_id);
        let seq_key = format!("ingest:sequence:{}", device_id);

        let _ = cache_set(
            &rl_key,
            &serde_json::to_string(&state.rate_limiter).unwrap(),
            86400,
        )
        .await;
        let _ = cache_set(
            &cb_key,
            &serde_json::to_string(&state.circuit_breaker).unwrap(),
            86400,
        )
        .await;
        let _ = cache_set(
            &seq_key,
            &serde_json::to_string(&state.gap_detector.last_sequence()).unwrap(),
            86400,
        )
        .await;
    }

    /// Procesa un mensaje HL7 entrante, aplica hardening
    /// Returns: (allowed: bool, reason: Option<String>, metrics_update: MetricsUpdate)
    pub async fn process_message(
        &self,
        device_id: &str,
        frame_size: usize,
        sequence: Option<u32>,
        parse_result: &Result<(), String>,
    ) -> (bool, Option<String>, MetricsUpdate) {
        let mut metrics_update = MetricsUpdate::default();

        // 1. Frame size limit
        if frame_size > self.config.max_frame_bytes {
            metrics_update.throttled = true;
            metrics_update.throttled_count = 1;
            self.increment_throttled().await;
            return (false, Some("frame_too_large".to_string()), metrics_update);
        }

        // 2. Auto-throttle bajo presión sistémica
        if self.config.auto_throttle_enabled && self.system_under_pressure().await {
            // Reducir rate limit efectivo a la mitad
            metrics_update.auto_throttled = true;
        }

        // 3. Obtener estado del device
        let mut state = self.get_device_state(device_id).await;

        // 4. Circuit breaker check
        if matches!(
            state.circuit_breaker.state(),
            circuit_breaker::CircuitState::Open
        ) {
            if state.circuit_breaker.should_attempt_reset() {
                state.circuit_breaker.transition_to_half_open();
            } else {
                metrics_update.circuit_open = true;
                metrics_update.circuit_open_count = 1;
                self.increment_fault_device(device_id, true).await;
                self.persist_device_state(device_id, &state).await;
                return (false, Some("circuit_open".to_string()), metrics_update);
            }
        }

        // 5. Rate limit (token bucket)
        let tokens_needed = 1.0;
        if !state.rate_limiter.try_consume(tokens_needed) {
            metrics_update.throttled = true;
            metrics_update.throttled_count = 1;
            self.increment_throttled().await;
            self.persist_device_state(device_id, &state).await;
            return (false, Some("throttled".to_string()), metrics_update);
        }

        // 6. Gap detection (si hay sequence number)
        if self.config.gap_detection_enabled
            && let Some(seq) = sequence
            && let Some(gap) = state.gap_detector.check_gap(seq)
        {
            metrics_update.gaps = true;
            metrics_update.gaps_count = gap.into();
            self.increment_gaps(gap.into()).await;
        }

        // 7. Parse/validation result handling
        let (parse_ok, validation_ok) = match parse_result {
            Ok(()) => (true, true),
            Err(e) if e.contains("validation") => (true, false),
            Err(_) => (false, false),
        };

        // 8. Circuit breaker: registrar resultado
        state
            .circuit_breaker
            .record_result(parse_ok && validation_ok);

        // Actualizar métricas de error
        if !parse_ok || !validation_ok {
            metrics_update.errors = true;
            metrics_update.errors_count = 1;
            self.update_error_rate(device_id, parse_ok, validation_ok)
                .await;
        }

        // 9. Si CB pasó a Open, marcar fault device
        if matches!(
            state.circuit_breaker.state(),
            circuit_breaker::CircuitState::Open
        ) {
            self.increment_fault_device(device_id, true).await;
        }

        // 10. Persistir estado actualizado
        self.persist_device_state(device_id, &state).await;

        // 11. Actualizar última vista
        state.last_seen = std::time::Instant::now();
        let devices = self.devices.write().await;
        devices.insert(device_id.to_string(), state);

        (true, None, metrics_update)
    }

    async fn system_under_pressure(&self) -> bool {
        // Leer métricas de sistema (CPU, memoria)
        // Por simplicidad: check básico
        false // TODO: implementar lectura real de /proc o metrics crate
    }

    async fn increment_throttled(&self) {
        let mut m = self.metrics.write().await;
        m.throttled_total += 1;
    }

    async fn increment_gaps(&self, count: u64) {
        let mut m = self.metrics.write().await;
        m.gaps_total += count;
    }

    async fn increment_fault_device(&self, _device_id: &str, _is_fault: bool) {
        let mut m = self.metrics.write().await;
        // Recalcular fault devices count desde states
        let devices = self.devices.read().await;
        m.fault_devices = devices
            .iter()
            .filter(|d| {
                matches!(
                    d.circuit_breaker.state(),
                    circuit_breaker::CircuitState::Open
                )
            })
            .count() as u64;
    }

    async fn update_error_rate(&self, _device_id: &str, parse_ok: bool, validation_ok: bool) {
        let mut m = self.metrics.write().await;
        m.invalid_total += 1;
        // Recalcular error_avg (rolling window simplificado)
        // TODO: implementar ventana deslizante real
        m.error_avg =
            (m.error_avg * 0.9) + (if parse_ok && validation_ok { 0.0 } else { 1.0 }) * 0.1;
    }
}

#[derive(Debug, Default, Clone)]
pub struct MetricsUpdate {
    pub throttled: bool,
    pub throttled_count: u64,
    pub circuit_open: bool,
    pub circuit_open_count: u64,
    pub gaps: bool,
    pub gaps_count: u64,
    pub errors: bool,
    pub errors_count: u64,
    pub auto_throttled: bool,
}

impl DeviceState {
    fn new(config: &IngestConfig) -> Self {
        Self {
            rate_limiter: rate_limit::TokenBucket::new(
                config.rate_limit_rps,
                config.rate_limit_burst,
            ),
            circuit_breaker: circuit_breaker::CircuitBreaker::new(
                config.cb_error_threshold,
                config.cb_window_seconds,
                config.cb_half_open_seconds,
            ),
            gap_detector: gap_detector::GapDetector::new(),
            last_seen: std::time::Instant::now(),
        }
    }
}

/// Inicializa métricas Prometheus para ingest (llamado desde metrics::register)
pub fn register_ingest_metrics() {
    use metrics::{Unit, describe_counter, describe_gauge, describe_histogram};

    // Rate limiting
    describe_counter!(
        "ingest_throttled_total",
        Unit::Count,
        "Messages throttled by rate limiter per device"
    );
    describe_gauge!(
        "ingest_rate_limit_current",
        Unit::Count,
        "Current tokens available in rate limit bucket per device"
    );

    // Circuit breaker
    describe_gauge!(
        "ingest_fault_devices",
        Unit::Count,
        "Devices in fault state (circuit breaker open)"
    );
    describe_gauge!(
        "ingest_circuit_state",
        Unit::Count,
        "Circuit breaker state per device (1=closed, 2=open, 3=half_open)"
    );

    // Data quality
    describe_counter!(
        "ingest_gap_total",
        Unit::Count,
        "Monitor data gaps (sequence number jumps) per device"
    );
    describe_counter!(
        "ingest_invalid_total",
        Unit::Count,
        "Invalid monitor samples per device/vital/reason"
    );
    describe_gauge!(
        "ingest_error_avg",
        Unit::Count,
        "Average ingest error rate (rolling 5m)"
    );

    // Throughput
    describe_counter!(
        "ingest_messages_total",
        Unit::Count,
        "Total HL7 messages ingested per device/result"
    );
    describe_histogram!(
        "ingest_message_size_bytes",
        Unit::Bytes,
        "HL7 message frame size distribution"
    );
}
