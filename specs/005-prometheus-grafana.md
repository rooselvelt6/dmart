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

  Scenario: Grafana dashboards load and show data
    Given Grafana connected to Prometheus
    When importing dashboard JSONs
    Then dashboards render without errors:
      - "dMart Overview" (RED metrics + system)
      - "Clinical KPIs" (pacientes, mortalidad, scores, occupancy)
      - "ML Models" (accuracy, latency, drift, predictions/day)
      - "Security" (auth failures, token refresh, RBAC denials)
      - "HL7/FHIR" (messages processed, latency, errors)

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

## Testing Strategy

### Unit Tests
- [ ] `test_metrics_endpoint_exposes_all()` — scrape `/metrics` → parse → verify keys
- [ ] `test_histogram_buckets_correct()` — latencies fall in correct buckets
- [ ] `test_business_metrics_updated()` — simulate actions → verify counters increment

### Integration Tests
- [ ] Prometheus scrape config works → targets UP
- [ ] Alert rules evaluate correctly (Promtool test)
- [ ] Grafana dashboards import without errors

### Load Test
- [ ] k6 test: 1000 req/s → `/metrics` responds < 100ms

## Rollout Plan
- **Feature Flag**: `METRICS_EXTENDED=true` (default ON)
- **Dashboards**: Importar JSONs en Grafana staging → validar → promover a prod
- **Alerts**: Dry-run 48h (solo log, no notify) → luego activar notificaciones

## Definition of Done
- [ ] Spec aprobada
- [ ] `/metrics` expone todas las métricas listadas
- [ ] 5 dashboards Grafana JSONs en `grafana/dashboards/`
- [ ] 10+ alerting rules en `prometheus/rules/`
- [ ] `promtool test rules` pasa
- [ ] Dashboards importados y validados en staging
- [ ] CHANGELOG.md actualizado