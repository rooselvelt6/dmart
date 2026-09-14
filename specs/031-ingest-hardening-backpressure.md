# SPEC-031: Hardening Ingest HL7 (rate-limit, circuit breaker, data-quality)

## Contexto

- **Problema a resolver**: El ingest HL7 (SPEC-003) acepta tráfico ilimitado sin protecciones. Un monitor rebotando (flapping) o enviando datos basura puede saturar CPU, memoria, DB y red, afectando al resto de la UCI. No hay visibilidad de calidad de datos (gaps, faults, throttling).
- **Usuario objetivo**: Ingeniería clínica / DevOps / On-call
- **Métrica de éxito (KPI)**: 0 eventos de OOM/CPU spike por ingest; gap/fault detection < 30s; alertas SPEC-008 disparan correctamente; latencia p99 ingest < 500ms bajo carga normal

## Acceptance Criteria (Gherkin)

```gherkin
Feature: Hardening Ingest HL7
  As a operador de UCI
  I want el pipeline HL7 sea resiliente a monitores mal comportados
  So that un dispositivo roto no tumbe el sistema ni pierda datos de otros

  Scenario: Token bucket rate limit por device
    Given device "MON-001" envía HL7 vía MLLP
    When excede 10 msg/s (burst 20) sostenido
    Then mensajes excedentes reciben NAK (AR) con error "throttled"
    And métrica ingest_throttled_total{device="MON-001"} incrementa
    And otros dispositivos no afectados

  Scenario: Circuit breaker por device
    Given device "MON-002" tiene > 50% errores parseo en 1 min
    Then circuit breaker abre para ese device
    And conexiones MLLP nuevas de ese device rechazadas con NAK "circuit_open"
    And métrica ingest_fault_devices incrementa
    After 5 min → half-open: permite 1 msg de prueba
    If éxito → cierra breaker; si falla → vuelve a abrir

  Scenario: Gap detection (secuencia HL7)
    Given device "MON-003" envía ORU^R01 con MSH.13 (sequence number)
    When sequence number salta > 1 (gap detectado)
    Then métrica ingest_gap_total{device="MON-003"} incrementa
    And log WARNING con device, expected_seq, received_seq

  Scenario: Invalid sample detection (vitales fuera de rango)
    Given HL7 con OBX valor HR=9999 (físicamente imposible)
    When parser procesa
    Then muestra descartada (no persiste en BD)
    And métrica ingest_invalid_total{device, vital="HR"} incrementa
    And métrica ingest_error_avg actualizada

  Scenario: Ingest error rate average (rolling window)
    Given ventana 5 min
    When error_rate = (parse_errors + validation_errors + throttled) / total_msgs > 0.1
    Then alerta HL7IngestDegraded (SPEC-008) dispara

  Scenario: MLLP frame size limit
    Given mensaje > 1 MiB (configurable)
    Then conexión cerrada con NAK "frame_too_large"
    And métrica ingest_invalid_total incrementa

  Scenario: Graceful degradation bajo carga extrema
    Given 100 dispositivos enviando simultáneo (load test)
    When CPU > 80% o memoria > 90%
    Then rate limits se vuelven más agresivos (auto-scale down)
    And healthcheck /obs/health sigue respondiendo "healthy"
    And métricas Prometheus siguen expuestas
```

## API Contracts

### Endpoints modificados
- `POST /hl7/mllp` (MLLP TCP) — comportamiento interno, sin cambios en API HTTP

### Métricas nuevas (expuestas en `/metrics` / `/obs/metrics`)
```prometheus
# Rate limiting
ingest_throttled_total{device}                    # Counter: mensajes throttled por device
ingest_rate_limit_current{device}                 # Gauge: tokens disponibles en bucket

# Circuit breaker
ingest_fault_devices                              # Gauge: dispositivos en estado fault
ingest_circuit_state{device, state="closed|open|half_open"}  # Gauge: 1 si en ese estado

# Data quality
ingest_gap_total{device}                          # Counter: gaps de secuencia detectados
ingest_invalid_total{device, vital, reason}       # Counter: muestras inválidas (out_of_range, malformed, missing_required)
ingest_error_avg                                  # Gauge: error rate rolling 5m (0.0-1.0)

# Throughput
ingest_messages_total{device, result="ok|parse_error|validation_error|throttled|circuit_open"}  # Counter
ingest_message_size_bytes{device}                 # Histogram: tamaño mensajes
```

### Configuración (env vars)
```bash
# Rate limit (token bucket)
DMART_INGEST_RATE_LIMIT_RPS=10        # requests/segundo por device
DMART_INGEST_RATE_LIMIT_BURST=20      # burst tokens

# Circuit breaker
DMART_INGEST_CB_ERROR_THRESHOLD=0.5   # 50% errores abre breaker
DMART_INGEST_CB_WINDOW_SECONDS=60     # ventana evaluación
DMART_INGEST_CB_HALF_OPEN_SECONDS=300 # tiempo antes de half-open

# Gap detection
DMART_INGEST_GAP_DETECTION=true       # habilitar detección gaps

# Frame limits
DMART_INGEST_MAX_FRAME_BYTES=1048576  # 1 MiB

# Auto-throttle bajo presión
DMART_INGEST_AUTO_THROTTLE=true       # habilitar auto-throttle
DMART_INGEST_CPU_THRESHOLD=80         # % CPU para activar
DMART_INGEST_MEM_THRESHOLD=90         # % memoria para activar
```

## Data Models

### Estado por device (en memoria + Valkey para persistencia cross-restart)
```rust
struct DeviceIngestState {
    device_id: String,
    // Token bucket
    tokens: f64,
    last_refill: Instant,
    // Circuit breaker
    cb_state: CircuitState,  // Closed | Open | HalfOpen
    cb_error_count: u64,
    cb_total_count: u64,
    cb_last_failure: Option<Instant>,
    // Gap detection
    last_sequence: Option<u32>,
    // Metrics
    throttled_count: u64,
    gap_count: u64,
    invalid_count: u64,
}
```

### Valkey keys (TTL 24h)
```
ingest:rate_limit:{device_id}       -> {tokens, last_refill}
ingest:circuit_breaker:{device_id}  -> {state, error_count, total_count, last_failure_ts}
ingest:sequence:{device_id}         -> last_sequence (u32)
```

## Edge Cases

| # | Caso | Comportamiento esperado |
|---|------|------------------------|
| 1 | Device nuevo (sin estado) | Bucket lleno, CB closed, sequence=0 |
| 2 | Reinicio servidor | Estado restaurado desde Valkey (TTL 24h) |
| 3 | MSH.13 no numérico / ausente | Gap detection deshabilitado para ese msg; log DEBUG |
| 4 | Múltiples conexiones mismo device | Estado compartido via Valkey (atomic ops) |
| 5 | Clock drift entre instancias | Token bucket usa tiempo monotónico local; Valkey como fuente verdad |
| 6 | Dispositivo legítimo con burst alto | Burst configurable; alerta si sostenido > threshold |
| 7 | CB half-open con éxito parcial | Requiere N éxitos consecutivos (configurable) para cerrar |

## Security Considerations

### Threat Model (STRIDE)

| Threat | Mitigación |
|--------|------------|
| Spoofing | Device ID validado contra registry (futuro); por ahora MSH.3/MSH.4 |
| Tampering | Checksum HL7 (MSH.18) validado; estado en Valkey con ACL |
| Repudiation | Audit log inmutable de throttling/CB actions |
| Information Disclosure | No PHI en métricas; solo device_id + contadores |
| Denial of Service | Rate limit + CB + frame limit + auto-throttle = defensa en profundidad |
| Elevation of Privilege | Ingest path sin auth (MLLP plano); aislamiento red (VLAN monitores) |

### Data Classification
- [ ] PHI
- [ ] PII
- [x] Clinical Data (vital signs raw)
- [x] Operational/Metadata (device_id, sequence, rates)

### Auth/Autz Requirements
- MLLP ingest: network-level trust (VLAN segregada)
- Métricas/alertas: SPEC-005/008 RBAC

## Testing Strategy

### Unit Tests
- [ ] `test_token_bucket_allows_burst_then_throttles`
- [ ] `test_token_bucket_refills_over_time`
- [ ] `test_circuit_breaker_opens_on_error_threshold`
- [ ] `test_circuit_breaker_half_open_success_closes`
- [ ] `test_circuit_breaker_half_open_failure_reopens`
- [ ] `test_gap_detection_increments_on_sequence_jump`
- [ ] `test_gap_detection_ignores_wraparound`
- [ ] `test_invalid_vital_discarded_and_counted`
- [ ] `test_frame_size_limit_rejects_large_messages`

### Property-Based Tests (proptest)
- [ ] Token bucket invariant: tokens nunca > capacity, nunca < 0
- [ ] Circuit breaker state machine: closed→open→half_open→closed/open
- [ ] Gap detection: monotonically increasing sequence (mod 2^16)

### Fuzzing Targets (cargo-fuzz)
- [ ] MLLP frame parsing con size limits
- [ ] HL7 parser con datos maliciosos + rate limit

### Integration Tests
- [ ] MLLP server + token bucket + CB end-to-end
- [ ] Multi-device isolation (device A throttled no afecta B)
- [ ] Estado persistente tras reinicio (Valkey)

### Load Test (k6)
- [ ] 50 devices × 20 msg/s = 1000 msg/s sostenido
- [ ] 1 device flapping (100 msg/s) → throttled, otros OK
- [ ] 1 device bad data (50% errors) → CB opens, alerta

## Rollout Plan

### Feature Flag
```rust
// config.rs
ingest_hardening: {
    enabled: true,
    rate_limit: true,
    circuit_breaker: true,
    gap_detection: true,
    auto_throttle: true,
}
```

### Canary Deployment
| Paso | Tráfico | Duración | Criterio go/no-go |
|------|---------|----------|-------------------|
| 1 | Staging (dispositivos simulados) | 48h | 0 falsos positivos CB; rate limit preciso |
| 2 | Piloto 1 dispositivo real | 1 semana | Gap detection válido; alertas SPEC-008 OK |
| 3 | Todos los dispositivos piloto | — | Métricas estables; 0 incidentes |

### Rollback Procedure
1. `ingest_hardening.enabled = false` (config reload instantáneo)
2. Verificar ingest normalizado
3. Métricas `ingest_*` caen a 0

## Definition of Done

- [ ] Spec aprobada en PR (1+ reviewer)
- [ ] `dmart-server/src/ingest/` module creado (rate_limit.rs, circuit_breaker.rs, gap_detector.rs, quality.rs)
- [ ] MLLP server integra token bucket + CB + gap detection
- [ ] Métricas `ingest_*` expuestas en `/metrics` (extendidas con `METRICS_EXTENDED=true`)
- [ ] Valkey persistence para estado cross-restart
- [ ] Config via env vars (ver API Contracts)
- [ ] Tests: unit + prop + fuzz + integration + k6 load
- [ ] `promtool test rules` pasa (alertas SPEC-008 usan nuevas métricas)
- [ ] Documentación: `docs/INGEST_HARDENING.md`
- [ ] ROADMAP, CHANGELOG actualizados
- [ ] Deploy staging verificado con simulador 50 devices