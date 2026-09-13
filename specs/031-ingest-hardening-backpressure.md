# SPEC-031: Hardening del Ingest HL7/MLLP (Backpressure, Rate-Limit, Data-Quality)

## Contexto
- **Problema**: el ingest ha recibido monitor flood, frames corruptos y gaps. Un monitor puede entrar en "rebote" (DOP/APN stop esperando ack) y saturar el filósofo; un dispositivo averiado puede emitir valores inválidos sin que nadie lo sepa. Hoy se validan los frames de forma individual, pero no hay throttling ni monitoreo de calidad del flujo.
- **Usuario objetivo**: Operador clínico / Ops / Backend
- **Métrica de éxito (KPI)**: 0 hormiga de cola MLLP en condiciones de rebote; gap detection < 2 min (p95); alertas de data-quality ≤ 1 h tras desviación.

## Acceptance Criteria (Gherkin)

```gherkin
Feature: Ingest MLLP resiliente y observable
  As a operador de la UCI
  I want que el ingest aguante abusos y avise cuando un sensor miente
  So that ningún monitor degrada el sistema y los gaps se detectan

  Scenario: Rate-limit por fuente TCP
    Given un cliente que supera RM_INGEST_MAX_FRAMES_PER_SEC=50
    When envía frames en serie
    Then los sobrantes reciben ACK negativo local (log rate)
    And se cuenta `ingest.throttled_total` en métricas

  Scenario: Circuit breaker por error rate
    Given error rate de parsing > 10% en ventana de 60 s (EWMA)
    When llegan más frames
    Then se corta la ingestión de esa fuente durante 30 s (backoff) con log + alerta
    And tras el cooldown se rehabilita solo si la tasa recupera < 5%

  Scenario: Detección de gap del sensor
    Given una cama sin frames durante DMART_GAP_MINUTES=5
    When el job de data-quality corre (cada 2 min)
    Then se emite un gap event para esa cama (patient_id, since)
    And Grafana (SPEC-005) lo muestra como sensor offline

  Scenario: Sensor con valores inválidos evolutivos
    Given una cama que acumula > 20 frames inválidos en 10 min
    When el job de data-quality corre
    Then marca `device_status=fault` en la cama
    And los scores de esa cama se marcan metadata `data_quality=degraded`

  Scenario: Alertas prometheus
    Given los eventos anteriores
    Then `ingest.gap_total`, `ingest.invalid_total`, `ingest.fault_devices` se exponen
```

## Design / Algoritmos

### Sliding-window rate limit (por fuente TCP)
- `RateLimiter` token bucket por dirección IP: capacidad `INGEST_BUCKET_CAPACITY=500`, refill `INGEST_REFILL_PER_SEC=50`, burst inicial acotado. Implementación lock-free con `AtomicU64` + cas.
```rust
struct TokenBucket { tokens: AtomicU64, last_refill: AtomicU64 }
fn try_acquire(&self) -> bool  // refill proporcional a Δt, satura a capacity
```

### Circuit breaker (EWMA error rate + hysteresis)
- EWMA por fuente: `err_avg = α·err_avg + (1-α)·(err?1:0)`, α = 0.1 (ventana ~10 mensajes).
- Estados `Closed → Open (acciones 60 s) → HalfOpen (prueba 1 frame) → Closed`; se abre si `err_avg > 0.10`, se cierra si la probe pasa y `err_avg < 0.05`.
- MLLP: en `Open`, se rechaza el `START/END` bloque (ACK negativo) sin parsear; el cliente TCP recibe backpressure real por la capa del stream.

### Gap / data-quality job
- Consulta por cama: `max(t)` por paciente vía índice (0.1 s, batch por día). Gap si `now - max(t) > DMART_GAP_MINUTES`.
- `fault` si `invalid/total > 0.2` en ventana 10 min → marca `Device.fault_reason`; los docs de score incluyen `data_quality`.

## API Contracts

| Método | Path | Cambio |
|--------|------|--------|
| GET | `/api/admin/ingest/status` | nuevo (admin): por fuente (rate, err_avg, estado breaker, gap) |
| POST | `/api/admin/ingest/throttle` | nuevo (admin): override temporal de rate para una fuente |

Evento interno (no HTTP): `GapEvent { patient_id, since, until?, severity }` → emitido a SSE (estado `device_events`) y a Prometheus via endpoint `/metrics` (SPEC-005).

## Data Models / Migraciones
```sql
-- migrations/XXX_device_data_quality.surql
DEFINE FIELD fault_reason ON Device TYPE string?;
DEFINE FIELD last_ok_at ON Device TYPE datetime?;
DEFINE TABLE gap_event SCHEMAFULL;
DEFINE FIELD patient_id ON gap_event TYPE record<Patient>;
DEFINE FIELD since / until / severity TYPE datetime / datetime / int;
DEFINE TABLE throttle_rule SCHEMAFULL;
DEFINE FIELD source_ip / frames_per_sec / ends_at TYPE ...;
```

## Edge Cases
| # | Caso | Comportamiento |
|---|------|----------------|
| 1 | Monitoreo simultáneo + medición en el mismo segundo | rate limiter por fuente; mediciones triviales (~1.7 msg/s) sin impacto |
| 2 | Breaker abre por ruido real (reden: monitor válido) | ventana 60 s y probe en half-open evita falsos permanentes |
| 3 | Gap por apagado programado del equipo | `maintenance window` en Device evita alertas |
| 4 | Cliente lento pero válido | ACK del stream preserva backpressure TCP; nunca se pierde frame |
| 5 | Reinicio del servidor | breaker/rate state vuelve a cero (solo ventana corta), gap job se recalcula en < 2 min |
| 6 | Dos fuentes en misma IP | rate por par (IP, port) para no penalizar repartidores |

## Security Considerations
- **DoS**: límites de tasa/backoff evitan inundación de un nodo comprometido en LAN; respetar tiempo de pausa MLLP *(CS timeout)*.
- Rate-limit admin-only; métricas nuevas no exponen PHI (solo `patient_id` en eventos internos, no en `/metrics`).
- No loggear tramas MLP enteras (pueden contener PHI): log de `frame#`, `length`, `error kind`.

## Testing Strategy
- [ ] Token bucket: burst tilde a capacidad, refill lineal, 0-race con threads
- [ ] Circuit breaker: transiciones Closed→Open→HalfOpen→Closed
- [ ] Integration (TCP real, `dmart-server/tests/hl7_integration.rs`): flood de 200 frames/s → throttled no acked; luego descansa
- [ ] Gap job: fixture con hueco de 6 min → `gap_event` + `fault_reason`
- [ ] k6: 100 VUs de números → ingest no degrada p95
- [ ] Load: APACHE II con `data_quality=degraded` persiste metadata

## Rollout Plan
- Fases: (A) rate-limit + breaker con defaults conservadores (B) gap/fault events (C) integración Grafana (con SPEC-005).
- Config: `DMART_INGEST_MAX_FRAMES_PER_SEC`, `DMART_GAP_MINUTES`, `DMART_BREAKER_WINDOW_SECS`.

## Definition of Done
- [ ] Spec aprobada
- [ ] TokenBucket + CircuitBreaker con tests de concurrencia
- [ ] Endpoints admin ingest status/throttle
- [ ] Gap/fault job + tabla `gap_event`
- [ ] Métricas Prometheus del ingest
- [ ] Documentos de score con `data_quality`
- [ ] CHANGELOG.md y ROADMAP actualizados