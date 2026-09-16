# SPEC-035: Cost Optimization — Right-sizing, spot instances, report mensual

## Contexto

- **Problema a resolver**: dMart corre con recursos sobredimensionados por defecto (reservas conservadoras) y sin visibilidad de coste por ambiente/tenant/cama de UCI. Para sostenibilidad del piloto y del contrato hospitalario se necesita right-sizing de recursos, uso de instancias spot cuando es seguro, y un reporte mensual de costes con tendencia.
- **Usuario objetivo**: DevOps / FinOps / Administrador hospitalario
- **Métrica de éxito (KPI)**: Coste < $X/mes por cama UCI (X definido en contrato); ≥ 30% de reducción vs baseline; 0 impacto en latencia p95 ni en SLO; reporte mensual auto-generado

## Acceptance Criteria (Gherkin)

```gherkin
Feature: Cost Optimization
  As a FinOps
  I want right-size los recursos y reducir costes sin afectar el servicio
  So que el piloto sea viable económicamente y el contrato rentable

  Scenario: Baseline de coste
    Given dMart corriendo en staging/prod
    When paso el primer mes
    Then se genera reporte de coste baseline por ambiente
    And se desglosa por: CPU, memory, storage, network, DB
    And se normaliza a coste/cama UCI/mes

  Scenario: Right-sizing de pods
    Given pods dmart-server con requests 256Mi/250m
    When analizo métricas Prometheus (30 días)
    Then detecto sobre-provisioning (>80% utilización < 20% del tiempo)
    And ajusto requests a p99 real (e.g. 128Mi/125m)
    And el coste de compute baja sin afectar latencia p95

  Scenario: HPA + autoscaling vertical
    Given carga variable (día/noche, fin de semana)
    When configuro HPA con CPU=70% y VPA sugerencias
    Then pods reducen fuera de horas pico
    And coste de compute se reduce en horas de baja carga
    And 0 downtime en escala horizontal

  Scenario: Storage tiering
    Given measurements raw retenidos 30 días (SPEC-030)
    When configuro storage tiers
    Then hot storage (NVMe) solo para datos recientes (< 7 días)
    And cold/archive (S3/obs) para downsampling y backups
    And se mide coste de storage por tier

  Scenario: Spot instances para carga no crítica
    Given jobs de retención/downsampling y CI runner
    When migro a spot/preemptible instances
    Then el coste baja ≥ 50% en esos workloads
    And jobs se reanudan si la instancia es reclaimada (checkpoint)
    And datos críticos NUNCA en spot (surrealDB, API)

  Scenario: Idle resource detection
    Given ambientes de prueba (staging, dev, e2e)
    When detecto recursos idle > 7 días
    Then se envían alertas y auto-suspend si configurado
    And el ahorro se reporta en el reporte mensual

  Scenario: Reporte mensual con tendencia
    Given configuración de reporte `cost-report`
    When pasa el fin de mes
    Then se genera `docs/cost/reports/2026-09.md`
    And contiene: coste por ambiente, tendencia 12m, forecasts, anomalías
    And métrica cost_dmart_total_mensual expuesta
```

## API Contracts

### Scripts de Cost

```bash
# Análisis de metrics 30 días → sugerencias right-size
./scripts/cost_rightsize.sh \
  --prometheus http://prometheus:9090 \
  --namespace dmart \
  --output /tmp/rightsize_$(date +%Y%m%d).json

# Reporte mensual
./scripts/cost_report.sh \
  --month 2026-09 \
  --output docs/cost/reports/2026-09.md

# Detección de recursos idle
./scripts/cost_idle_detect.sh \
  --threshold-days 7 \
  --dry-run

# Forecast
./scripts/cost_forecast.sh \
  --months 6 \
  --output docs/cost/reports/forecast.md
```

### Reporte de coste (formato)

```markdown
# Reporte de Coste — Septiembre 2026

## Resumen
| Ambiente | Coste | Δ vs mes anterior | Coste/cama/mes |
|----------|-------|--------------------|----------------|
| Producción | $1,240.00 | +3.2% | $24.80 |
| Staging | $96.00 | -12.1% | — |
| CI/Testing | $32.50 | -28.3% | — |

## Desglose por recurso
| Recurso | Coste | % del total | Tendencia |
|---------|-------|-------------|-----------|
| Compute (pods) | $640.00 | 46% | ↓ |
| Storage (DB/backups) | $390.00 | 28% | → |
| Network | $128.00 | 9% | ↑ |
| Observabilidad | $82.00 | 6% | → |

## Right-size aplicado
- dmart-server: requests 256Mi/250m → 128Mi/125m (★ -$112.00/mes)
- Retención job en spot: -$24.00/mes
- Storage tiering raw→cold: -$58.00/mes

## Anomalías
- 2026-09-12: staging activo 3 días → idle detectado, auto-suspend
```

### Configuración

```bash
# Budget / alertas
DMART_COST_BUDGET_MONTHLY=1500
DMART_COST_ALERT_THRESHOLD_PCT=80
DMART_COST_PER_BED_TARGET=30

# Right-size
DMART_COST_RIGHTSIZE_ENABLED=true
DMART_COST_RIGHTSIZE_PERCENTILE=99
DMART_COST_MIN_OVERRIDE=true

# Idle
DMART_COST_IDLE_THRESHOLD_DAYS=7
DMART_COST_IDLE_AUTO_SUSPEND=false

# Storage
DMART_COST_STORAGE_HOT_DAYS=7
DMART_COST_STORAGE_COLD_CLASS="standard-ia"
```

## Data Models

### Métricas Prometheus (Cost)

```prometheus
# Cost de recursos
cost_compute_bytes{namespace,resource}      # Gauge: request/limit en cores pero coste "eur"
cost_compute_vcpu{namespace}                # Gauge: vCPU total reservado
cost_memory_bytes{namespace}                # Gauge: mem total reservada
cost_storage_bytes{namespace,tier}          # Gauge: storage por tier
cost_network_egress_bytes{namespace}        # Counter: tráfico saliente

# Utilización (para right-size)
utilization_cpu_p95{namespace,deployment}   # Gauge: p95 de uso CPU
utilization_memory_p95{namespace,deployment}# Gauge

# Budget
cost_dmart_budget_monthly                    # Gauge: budget configurado
cost_dmart_total_mensual                     # Gauge: coste real del mes
cost_budget_alert{level="warning|critical"} # Gauge: 1 si alerta
```

### Right-Size Suggestion (JSON)

```json
{
  "deployment": "dmart-server",
  "current": { "cpu": "250m", "memory": "256Mi", "replicas": 3 },
  "suggested": { "cpu": "125m", "memory": "128Mi", "replicas": 3 },
  "utilization_p95": { "cpu": 42.3, "memory": 51.0 },
  "utilization_p99": { "cpu": 58.1, "memory": 63.4 },
  "savings_monthly": 112.0,
  "risk": "low",
  "confidence": 0.95,
  "applied": false
}
```

## Edge Cases

| # | Caso | Comportamiento esperado |
|---|------|------------------------|
| 1 | Pico de carga puntual justo tras right-size | HPA lo absorbe (búfer); alerta si sostenido |
| 2 | Spot instance reclaimada durante job retención | Checkpoint/reanudación; nunca pierde datos confirmados |
| 3 | Storage tiering sin datos cold aún | Espera el cutoff; sin error |
| 4 | Budget excedido | Alerta finance; no auto-apaga (clínico)
| 5 | Idle detection falso positivo (staging usado de noche) | Whitelist por label; minimun uptime check |
| 6 | Múltiples tenants con coste compartido | Cost attribution por tenant labels (SPEC-025) |
| 7 | Cambio de precio del cloud provider | Forecast usa últimos prices; reporte nota el cambio |

## Security Considerations

### Threat Model (STRIDE)

| Threat | Mitigación |
|--------|------------|
| Spoofing | Cost scripts autenticados (kubectl impersonation) |
| Tampering | Reportes versionados en git; checksums |
| Repudiation | Audit log de right-size aplicado (quién/cuándo) |
| Information Disclosure | Reportes NO incluyen PHI; solo coste/recursos + tenant id |
| Denial of Service | right-size no reduce bajo mínimo seguro (min replicas 2) |
| Elevation of Privilege | Solo DevOps/FinOps pueden aplicar right-size |

### Datos sensibles
- Los reportes de coste NO mencionan pacientes ni datos clínicos
- Cost attribution por tenant usa slug anonimizado (tenant_id), no nombre hospital

### Data Classification
- [ ] PHI
- [x] Operational/Metadata (coste, recursos, utilización)

## Testing Strategy

### Unit Tests
- [ ] Análisis de métricas genera JSON valido de right-size
- [ ] Cálculo de coste por tier correcto
- [ ] Forecast 12m: mínimo 2 modelos validados vs histórico
- [ ] Idle detection respeta whitelist

### Integration Tests
- [ ] Reporte mensual generado en staging
- [ ] right-size aplicado en staging sin impacto latencia
- [ ] Spot/reclaim simulado en job de retención (reanuda)

### Load Test
- [ ] After right-size: latencia p95 idéntica al baseline (< 5% diff)
- [ ] HPA funciona con requests reducidos

## Rollout Plan

### Feature Flag
```yaml
cost:
  enabled: true
  rightsize: true
  idle_detection: true
  report: "monthly@00:30"
  budget: 1500
```

### Canary Deployment
| Paso | Tráfico | Duración | Criterio go/no-go |
|------|---------|----------|-------------------|
| 1 | Baseline medido (1 mes) | 30 días | Coste/cama baseline documentado |
| 2 | Reporte mensual + métricas | 15 días | Reporte correcto y alertas funcionando |
| 3 | Right-size solo staging | 7 días | 0 impacto latencia/errores |
| 4 | Right-size production (≤30% reductión) | 7 días | SLO intactos |
| 5 | Spot para jobs + storage tiering | 14 días | Ahorro ≥ 30% vs baseline |

### Rollback Procedure
1. Revertir requests a valores anteriores (`helm upgrade --reuse-values`)
2. Verificar utilización y latencia vuelven a baseline
3. Deshabilitar auto-suspend si idle detection causó problemas
4. Budget alert crítico → revisión manual (jamás auto-apagado clínico)

## Definition of Done

- [ ] `specs/035-cost-optimization.md` (este archivo)
- [ ] Baseline de coste medido y documentado
- [ ] `scripts/cost_rightsize.sh` funcional
- [ ] `scripts/cost_report.sh` funcional (mensual)
- [ ] `scripts/cost_idle_detect.sh` funcional
- [ ] `scripts/cost_forecast.sh` funcional
- [ ] Métricas `cost_*` en Prometheus
- [ ] Alertas de budget (warning 80%, critical 100%)
- [ ] Right-size aplicado en staging y prod
- [ ] Storage tiering activo
- [ ] Spot instances para jobs retención/CI (con checkpoint)
- [ ] Reporte mensual en `docs/cost/reports/`
- [ ] Tests: unit + integration + load
- [ ] ROADMAP, CHANGELOG actualizados

## Gate CI SPEC-035
```bash
./scripts/cost_report.sh --month $(date +%Y-%m) --output /tmp/cost.md  # exit 0
test -f /tmp/cost.md && grep -q "Coste/cama/mes" /tmp/cost.md
./scripts/cost_rightsize.sh --prometheus http://prometheus:9090 --namespace dmart --output /tmp/rs.json
[ "$(cat /tmp/cost.md | grep -c 'Right-size aplicado')" -ge 1 ]
```

## Métricas
- Coste por cama UCI/mes: < $30 (target contrato)
- Reducción vs baseline: ≥ 30%
- Utilización CPU media: ≥ 60% (post-right-size)
- Impacto en latencia p95: < 5% variación
- Budget: 0 mensuales excedidos