# SPEC-005: Prometheus `/metrics` + Grafana Dashboards

## Contexto
- **Problema**: Sistema tiene métricas básicas Prometheus (`/metrics` endpoint existe) pero faltan: métricas de negocio (KPIs clínicos), alertas configuradas, dashboards Grafana listos para producción, y exportación de métricas de ML/modelos.
- **Usuario objetivo**: DevOps / SRE / Clinical Operations
- **Métrica de éxito (KPI)**: 100% métricas críticas expuestas, 0 alertas falsas positivas/negativas en 30 días, MTTR < 15 min.

## Acceptance Criteria (Gherkin)

```gherkin
Feature: Prometheus Metrics + Grafana Dashboards
  As a DevOps Engineer
  I want comprehensive metrics and dashboards
  So that system health and clinical KPIs are observable

  Scenario: /metrics exposes all critical metrics
    Given server running
    When GET /metrics
    Then response includes:
      - HTTP: http_requests_total, http_request_duration_seconds (p50/p95/p99)
      - DB: surreal_query_duration_seconds, surreal_connection_pool
      - Auth: auth_login_total, auth_refresh_total, auth_failures_total
      - Business: patients_total, measurements_total, scales_calculated_total
      - ML: ml_model_load_duration_seconds, ml_predictions_total, ml_accuracy_gauge
      - System: process_cpu_seconds_total, process_resident_memory_bytes
      - Queue/Stream: sse_connections_active, hl7_messages_processed_total
      - Ingest/Data-quality (SPEC-031): ingest_gap_total, ingest_invalid_total, ingest_fault_devices, ingest_throttled_total, ingest_error_avg

  Scenario: Grafana dashboards load and show data
    Given Grafana connected to Prometheus
    When importing dashboard JSONs
    Then dashboards render without errors:
      - "dMart Overview" (RED metrics + system)
      - "Clinical KPIs" (pacientes, mortalidad, scores, occupancy)
      - "ML Models" (accuracy, latency, drift, predictions/day)
      - "Security" (auth failures, token refresh, RBAC denials)
      - "HL7/FHIR" (messages processed, latency, errors)
      - "Monitor Data-Quality" (gaps, sensor faults, throttling, degraded scores)

  Scenario: Alerting rules fire correctly
    Given Prometheus rules loaded
    When condition met (e.g., http 5xx > 1% for 5min)
    Then Alertmanager fires alert to webhook/email
    And alert includes: severity, runbook URL, affected service

  Scenario: ML model drift detection alert
    Given ML model serving predictions
    When prediction confidence drops > 20% vs baseline
    Then alert "ML Model Drift Detected" fires
    And includes model version, feature importance shift

  Scenario: Business KPI anomaly detection
    Given daily patient admissions
    When admissions drop > 50% vs 7-day avg
    Then alert "Clinical Volume Anomaly" fires
```

## API Contracts

### `/metrics` endpoint (ya existe, expandir)
```prometheus
# HELP http_requests_total Total HTTP requests
# TYPE http_requests_total counter
http_requests_total{method="GET",path="/api/patients",status="200"} 1234

# HELP http_request_duration_seconds HTTP request latency
# TYPE http_request_duration_seconds histogram
http_request_duration_seconds_bucket{method="POST",path="/api/scales/apache",le="0.1"} 567

# HELP patients_total Total patients in system
# TYPE patients_total gauge
patients_total{status="active"} 45

# HELP scales_calculated_total Total scales calculations
# TYPE scales_calculated_total counter
scales_calculated_total{scale="apache",result="success"} 890

# HELP ml_model_accuracy Current model accuracy
# TYPE ml_model_accuracy gauge
ml_model_accuracy{model="mortality_v1",version="1.0"} 0.87

# HELP ml_predictions_total Total ML predictions
# TYPE ml_predictions_total counter
ml_predictions_total{model="mortality_v1",outcome="survived"} 1500
```

### Grafana Dashboard JSONs (archivos en `grafana/dashboards/`)
```
grafana/dashboards/
├── dmart-overview.json          # RED + system health
├── clinical-kpis.json           # Pacientes, mortalidad, scores, camas
├── ml-models.json               # Accuracy, latency, drift, predictions
├── security.json                # Auth, RBAC, token refresh
├── hl7-fhir.json                # Interoperabilidad HL7/FHIR
├── monitor-data-quality.json    # Gaps de sensor, faults, throttling (SPEC-031)
└── datasource.yaml              # Prometheus datasource config
```

## Data Models

### Alerting Rules (`prometheus/rules/*.yml`)
```yaml
groups:
- name: dmart-critical
  rules:
  - alert: HighErrorRate
    expr: |
      sum(rate(http_requests_total{status=~"5.."}[5m])) 
      / sum(rate(http_requests_total[5m])) > 0.01
    for: 5m
    labels:
      severity: critical
    annotations:
      summary: "High 5xx error rate (>1%)"
      runbook_url: "https://wiki.dmart.io/runbooks/high-error-rate"

  - alert: DatabaseDown
    expr: up{job="surreal"} == 0
    for: 1m
    labels:
      severity: critical
    annotations:
      summary: "SurrealDB unreachable"

- name: dmart-business
  rules:
  - alert: ClinicalVolumeAnomaly
    expr: |
      patients_total{status="active"} 
      < (avg_over_time(patients_total{status="active"}[7d]) * 0.5)
    for: 15m
    labels:
      severity: warning
    annotations:
      summary: "Patient volume dropped >50% vs 7-day avg"

  - alert: MLModelDrift
    expr: |
      ml_model_accuracy{model="mortality_v1"} 
      < (ml_model_accuracy_baseline * 0.8)
    for: 1h
    labels:
      severity: warning
    annotations:
      summary: "ML model accuracy dropped >20%"

- name: dmart-ingest-quality
  rules:
  - alert: SensorGap
    expr: rate(ingest_gap_total[10m]) > 0
    for: 5m
    labels:
      severity: warning
    annotations:
      summary: "Monitor sin datos por >5min (sensor offline)"

  - alert: SensorFault
    expr: ingest_fault_devices > 0
    for: 10m
    labels:
      severity: critical
    annotations:
      summary: "Dispositivo marcado fault (valores inválidos)"

  - alert: IngestThrottling
    expr: rate(ingest_throttled_total[5m]) > 0
    for: 5m
    labels:
      severity: warning
    annotations:
      summary: "Ingest haciendo throttling (posible flood de monitor)"
```

## Edge Cases
| # | Caso | Comportamiento esperado |
|---|------|------------------------|
| 1 | Prometheus scrape timeout | Target marcado `down`, alerta `TargetDown` |
| 2 | Métricas de alta cardinalidad (user_id en labels) | Usar `label_limit` / `label_value_limit`, evitar cardinalidad |
| 3 | Histogram buckets insuficientes | Añadir buckets finos para latencias <10ms |
| 4 | Métricas ML no actualizadas | Alerta `MLMetricsStale` si `time() - ml_predictions_total timestamp > 1h` |

## Security Considerations
- **Metrics exposure**: `/metrics` solo accesible desde red interna (firewall) o con auth básica
- **No PHI en labels**: Nunca poner patient_id, MRN en labels de métricas
- **Alert routing**: Alertas críticas → PagerDuty/Slack, warning → email
- **Data-quality**: los events de gap/fault (SPEC-031) emiten `severity` y `daneez dev_type`, nunca datos del monitor (solo `patient_id` en canal interno SSE auditado)

## Testing Strategy

### Unit Tests
- [x] `test_metrics_endpoint_exposes_all()` — scrape `/metrics` → parse → verify keys
      (implementado como `test_metrics_endpoint_exposes_all_and_tracks_events` en
      `tests/api_tests.rs`, verifica las ~32 series del spec incl. data-quality)
- [x] `test_histogram_buckets_correct()` — buckets finos para latencias <10ms
      presentes en `/obs/metrics` (`le=0.0001..10` + `+Inf`, `_sum`, `_count`)
      — `test_metrics_histogram_buckets_fine_covered`
- [x] `test_business_metrics_updated()` — login success/failure, paciente creado
      y GCS incrementan contadores (dentro del E2E de métricas)
- [x] `test_ingest_quality_metrics()` — `ingest_gap_total`/`ingest_invalid_total`/
      `ingest_fault_devices`/`ingest_throttled_total`/`ingest_error_avg` expuestas
      (baseline 0 hasta SPEC-031; verificadas en el E2E de métricas)
- [x] Unit tests del feature flag `METRICS_EXTENDED` (parse) en `metrics.rs`

### Integration Tests
- [x] Prometheus scrape config works → `promtool check config` ✓ (job `dmart-server`
      → `/obs/metrics`, con `label_limit`/`sample_limit` para cardinalidad)
- [x] Alert rules evaluate correctly (`promtool test rules` ✓ — 12 escenarios)
- [x] Grafana dashboards import without errors → JSON válidos (schema 39) verificados
      con `jq`; import en staging pendiente (ver Rollout Plan)

### Load Test
- [x] k6 test `tests/load/metrics.js`: ramping a 1000 req/s sobre `/obs/metrics`
      con threshold `p95 < 100ms` (especificación del spec)
  - *Ejecución en local pendiente de entorno con server + DB reales.*

## Rollout Plan
- **Feature Flag**: `METRICS_EXTENDED=true` (default ON). Implementado en
  `metrics.rs::extended_enabled()` — si `false` solo se exponen métricas de
  sistema/HTTP/infra; KPIs clínicos, ML, HL7 e ingest quedan ocultos en scrape.
- **Dashboards**: JSONs listos; import en Grafana staging → validar → promover a
  prod es **dependencia operativa de SPEC-006/012** (staging no existe localmente).
- **Alerts**: dry-run 48h (solo log, no notificar) antes de activar notificaciones;
  el routing (AlertManager → PagerDuty/Slack/email) es **SPEC-008**.
- **Security**: `/obs/metrics` debe quedar restringido en prod: solo red interna
  (firewall/nginx allow) o `--web.config` con basic auth de Prometheus. Nunca PHI
  en labels (el server ya acota labels a method/status/route/scale/result/source).

## Definition of Done
- [x] Spec aprobada
- [x] `/metrics` expone todas las métricas listadas (incl. ingest data-quality)
- [x] 6 dashboards Grafana JSONs en `grafana/dashboards/`
- [x] 10+ alerting rules en `prometheus/rules/`
- [x] `promtool test rules` pasa
- [ ] Dashboards importados y validados en staging (depende de SPEC-006/012)
- [x] CHANGELOG.md actualizado