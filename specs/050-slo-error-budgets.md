# SPEC-050: SLOs y error budgets (SLI/SLO + alertamiento)

## Contexto
- **Problema a resolver**: el roadmap exige publicar y medir SLOs (99.5 %
  disponibilidad, p95 < 100 ms, freshness SSE < 1–5 s) con error budgets y
  ruteo de alertas; hoy solo hay métricas crudas y targets documentales.
- **Usuario objetivo**: SRE / operaciones (Fase 3.5) y Admin.
- **Métrica de éxito (KPI)**: `GET /obs/slo` devuelve disponibilidad, p95 y
  freshness con `overall_ok`; error budget por SLO en Prometheus; alertas
  fast-burn/exhausted en `alerts-slo.yml`.

## Acceptance Criteria (Gherkin)

```gherkin
Feature: SLOs y error budgets
  As an SRE
  I want measurable SLOs with error budgets and alerts
  So that I can act before the service violates its objectives

  Scenario: Reporte de SLO
    Given el servidor en ejecución
    When GET /obs/slo
    Then responde 200 con availability, latency_p95_ms, sse_freshness_seconds
    And una lista de 3 SloItem con ok y error_budget_remaining
    And overall_ok=true si los 3 cumplen

  Scenario: Error budget se agota con 5xx
    Given requests registradas con 5xx por encima del 0.5 %
    When se consulta el snapshot
    Then el error budget de disponibilidad es 0.0 y ok=false

  Scenario: Latencia p95 dentro de objetivo
    Given peticiones con latencia <= 100 ms
    When se consulta el snapshot
    Then latency_p95_ms <= 100 y el item latency_p95 tiene ok=true

  Scenario: Frescura SSE
    Given una publicación SSE reciente
    When se consulta el snapshot
    Then sse_freshness_seconds < 5 y el item sse_freshness tiene ok=true

  Scenario: Export a Prometheus
    Given un scrape de /metrics
    Then se exponen slo_availability_ratio, slo_latency_p95_ms,
         slo_sse_freshness_seconds y slo_error_budget_remaining{slo}
```

## API Contracts

| Método | Path | Auth | Response |
|--------|------|------|----------|
| GET | `/obs/slo` | pública (observabilidad) | `200 SloReport` |

Métricas Prometheus nuevas:

| Métrica | Tipo | Descripción |
|---------|------|-------------|
| `slo_availability_ratio` | gauge | SLI disponibilidad (1 - 5xx/total) |
| `slo_requests_total` | gauge | Requests considerados |
| `slo_requests_5xx_total` | gauge | Requests 5xx |
| `slo_latency_p95_ms` | gauge | Latencia p95 (ms) |
| `slo_sse_freshness_seconds` | gauge | Segundos desde último evento SSE |
| `slo_error_budget_remaining{slo}` | gauge | Budget restante (1=lleno) |

## Data Models
No aplica (estado en memoria; sin migración).

## Edge Cases
| # | Caso | Comportamiento esperado |
|---|------|------------------------|
| 1 | Sin requests | availability=1.0, p95=0.0 → ok |
| 2 | Sin publicaciones SSE | freshness=0.0 (inactivo) → ok |
| 3 | allowed_bad_ratio=0 | budget 1.0 si no hay fallos, 0.0 si hay |
| 4 | p95 en el borde | bucket `<=100 ms` cuenta como bueno |

## Security Considerations
- Ruta `/obs/slo` no expone PHI ni secretos (solo agregados).
- Sin escritura; sin cambios de RBAC.

## Testing Strategy
- [x] Unit `error_budget_full_and_exhausted`
- [x] Unit `snapshot_has_three_items_and_bounded_values`
- [x] Unit `sse_freshness_updates_on_publish`
- [x] `cargo test -p dmart-server --lib`

## Rollout / DoD
- [x] Dashboard Grafana `dmart-slo.json`
- [x] Reglas Prometheus `alerts-slo.yml` (fast burn + exhausted)
- [x] CHANGELOG actualizado
