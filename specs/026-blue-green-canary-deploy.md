# SPEC-026: Blue/Green + Canary Deploy — Zero-downtime deployments

## Contexto

- **Problema a resolver**: El deploy actual (docker compose / K8s rolling update) tiene ventanas de downtime durante la transición. Para producción hospitalaria se necesita zero-downtime absoluto: blue/green para releases grandes y canary para rollout gradual con validación de métricas antes de comprometer todo el tráfico.
- **Usuario objetivo**: DevOps / SRE
- **Métrica de éxito (KPI)**: 0 downtime en deploy; rollback en < 60s; canary con validación automática de métricas; 0 error spike durante deploy

## Acceptance Criteria (Gherkin)

```gherkin
Feature: Blue/Green + Canary Deploy
  As a SRE
  I want deploy sin downtime con validación gradual
  So que la producción nunca se vea afectada por un deploy

  Scenario: Blue/Gendeploy completo
    Given producción en "blue" (v1.2.0) sirviendo tráfico
    When deployo v1.3.0 en "green"
    Then green se despliega y pasa healthcheck
    And el tráfico se commuta de blue→green instantáneamente
    And 0 requests fallan durante la transición
    And blue queda standby por 30 min

  Scenario: Canary 5% → 25% → 100%
    Given nueva versión desplegada en 1 pod canary
    When el canary recibe 5% del tráfico
    Then métricas del canary se monitorean (error rate, latency p95)
    After 10 min sin errores → scale a 25%
    After 30 min sin errores → scale a 100%
    And blue se desactiva

  Scenario: Canary failure → automatic rollback
    Given canary en 5% del tráfico
    When error rate del canary > 1% o p95 > 500ms
    Then rollback automático: canary removido
    And tráfico vuelve 100% a blue
    And alerta CanaryFailure dispara (SPEC-008)
    And 0 impacto al tráfico principal

  Scenario: Manual rollback instantáneo
    Given producción en green (v1.3.0)
    When ejecuto `./scripts/deploy_rollback.sh`
    Then tráfico vuelve a blue (v1.2.0) en < 60s
    And green se mantiene standby
    And healthcheck en blue OK

  Scenario: Deploy con DB migration
    Given migrations compatibles con v1.2 y v1.3
    When deployo v1.3.0 blue/green
    Then la migración se ejecuta antes del traffic switch
    And ambos blue y green pueden leer la DB migrada
    And el switch es instantáneo

  Scenario: Concurrent deploys
    Given deploy v1.3 en curso (canary 5%)
    When se necesita hotfix v1.3.1
    Then el deploy actual se cancela (abort canary)
    And v1.3.1 se despliega como nuevo canary
```

## API Contracts

### Deploy Scripts

```bash
# Blue/Green deploy
./scripts/deploy_blue_green.sh \
  --image ghcr.io/ucigtm/dmart-server:v1.3.0 \
  --namespace dmart \
  --healthcheck-url /obs/health \
  --switch-timeout 300 \
  --rollback-window 1800

# Canary deploy
./scripts/deploy_canary.sh \
  --image ghcr.io/ucigtm/dmart-server:v1.3.0 \
  --namespace dmart \
  --canary-weight 5 \
  --prometheus-url http://prometheus:9090 \
  --error-threshold 0.01 \
  --latency-threshold 500 \
  --auto-increment 10m,25m,100m

# Rollback
./scripts/deploy_rollback.sh \
  --namespace dmart \
  --target blue \
  --timeout 60

# Status
./scripts/deploy_status.sh --namespace dmart
```

### K8s Resources para Blue/Green

```yaml
# Deployment Blue (v1.2.0 - actual)
apiVersion: apps/v1
kind: Deployment
metadata:
  name: dmart-blue
  labels:
    app: dmart
    slot: blue
spec:
  replicas: 3
  selector:
    matchLabels:
      app: dmart
      slot: blue
  template:
    metadata:
      labels:
        app: dmart
        slot: blue
    spec:
      containers:
        - name: dmart
          image: ghcr.io/ucigtm/dmart-server:v1.2.0
---
# Deployment Green (v1.3.0 - staging)
apiVersion: apps/v1
kind: Deployment
metadata:
  name: dmart-green
  labels:
    app: dmart
    slot: green
spec:
  replicas: 3
  selector:
    matchLabels:
      app: dmart
      slot: green
  template:
    metadata:
      labels:
        app: dmart
        slot: green
    spec:
      containers:
        - name: dmart
          image: ghcr.io/ucigtm/dmart-server:v1.3.0
---
# Service selector apunta al slot activo
apiVersion: v1
kind: Service
metadata:
  name: dmart
spec:
  selector:
    app: dmart
    slot: blue  # se cambia a green durante switch
  ports:
    - port: 80
      targetPort: 8080
```

### Canary Routing (Istio/Nginx)

```yaml
# Nginx canary annotations
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: dmart-canary
  annotations:
    nginx.ingress.kubernetes.io/canary: "true"
    nginx.ingress.kubernetes.io/canary-weight: "5"
spec:
  rules:
    - host: dmart.hospital.cu
      http:
        paths:
          - path: /
            pathType: Prefix
            backend:
              service:
                name: dmart-canary
                port:
                  number: 80
```

### Metrics Validation Script

```bash
# Canary metrics check (used by deploy_canary.sh)
PROMETHEUS_URL="http://prometheus:9090"

# Error rate del canary
ERROR_RATE=$(curl -s "$PROMETHEUS_URL/api/v1/query" \
  --data-urlencode "query=sum(rate(http_requests_total{slot=\"canary\",code=~\"5..\"}[2m])) / sum(rate(http_requests_total{slot=\"canary\"}[2m]))" \
  | jq '.data.result[0].value[1]' -r)

# Latency p95 del canary
LATENCY_P95=$(curl -s "$PROMETHEUS_URL/api/v1/query" \
  --data-urlencode "query=histogram_quantile(0.95, sum(rate(http_request_duration_seconds_bucket{slot=\"canary\"}[2m])) by (le))" \
  | jq '.data.result[0].value[1]' -r)
```

### Métricas de Deploy

```prometheus
# Deploy status
deploy_active_slot{slot="blue|green"}                    # Gauge: 1 si activo
deploy_canary_weight                                      # Gauge: 0-100
deploy_last_switch_timestamp                              # Gauge: Unix timestamp
deploy_last_rollback_timestamp                            # Gauge: Unix timestamp

# Canary health
deploy_canary_error_rate                                  # Gauge: 0.0-1.0
deploy_canary_latency_p95_seconds                         # Gauge
deploy_canary_health{status="healthy|unhealthy"}          # Gauge
```

## Data Models

### Deploy State

```json
{
  "active_slot": "blue",
  "blue": {
    "version": "v1.2.0",
    "replicas": 3,
    "status": "serving",
    "deployed_at": "2026-09-15T10:00:00Z"
  },
  "green": {
    "version": "v1.3.0",
    "replicas": 3,
    "status": "standby",
    "deployed_at": "2026-09-15T14:00:00Z"
  },
  "canary": {
    "enabled": false,
    "weight": 0,
    "version": null,
    "error_rate": 0.0,
    "latency_p95_ms": 0
  }
}
```

### Rollback History

```json
{
  "rollbacks": [
    {
      "timestamp": "2026-09-15T15:30:00Z",
      "from": "green-v1.3.0",
      "to": "blue-v1.2.0",
      "reason": "canary_failure",
      "duration_seconds": 45
    }
  ]
}
```

## Edge Cases

| # | Caso | Comportamiento esperado |
|---|------|------------------------|
| 1 | Green healthcheck falla | No se hace switch; green eliminado; alerta |
| 2 | Múltiples deploys simultáneos | Solo 1 deploy activo; segundo en cola |
| 3 | Rollback durante canary | Canary eliminado; tráfico vuelve a blue |
| 4 | DB migration incompatible | Pre-flight check falla; deploy abortado |
| 5 | Network partition durante switch | Timeout → rollback automático |
| 6 | Metrics unavailable durante canary | Deploy pausado; alerta |
| 7 | Rollback a versión con DB migration | Requiere backward-compatible migration |

## Security Considerations

### Threat Model (STRIDE)

| Threat | Mitigación |
|--------|------------|
| Spoofing | Deploy solo desde CI/CD autenticado; image signing |
| Tampering | Image digest verification; no tag overrides |
| Repudiation | Deploy audit log con who/what/when |
| Information Disclosure | Canary metrics no exponen PHI |
| Denial of Service | Deploy rate limit; max 1 deploy/hora |
| Elevation of Privilege | Deploy scripts requieren CI token con scope limitado |

### Image Security

```yaml
# Image signature verification
spec:
  containers:
    - name: dmart
      image: ghcr.io/ucigtm/dmart-server@sha256:abc123...  # digest, no tag
  # Cosign verification en CI pipeline
```

## Testing Strategy

### Unit Tests
- [ ] Service selector switch funciona (blue→green)
- [ ] Canary weight routing funciona
- [ ] Rollback script cambia selector correctamente

### Integration Tests
- [ ] Blue/Green deploy completo en kind
- [ ] Canary incrementa tráfico gradualmente
- [ ] Canary failure → rollback automático
- [ ] Deploy abortado por healthcheck failure

### Chaos Tests
- [ ] Deploy durante network partition
- [ ] Deploy durante alta carga (k6)
- [ ] Rollback bajo presión

### Load Test (k6)
- [ ] Zero-downtime durante blue/green switch (0 errores)
- [ ] Canary con tráfico real no afecta latencia
- [ ] Rollback no causa error spike

## Rollout Plan

### Feature Flag
```yaml
deploy:
  strategy: "blue_green"  # | canary | rolling
  canary:
    initial_weight: 5
    increments: [25, 100]
    interval_minutes: 10
    error_threshold: 0.01
    latency_threshold_ms: 500
```

### Canary Deployment
| Paso | Tráfico | Duración | Criterio go/no-go |
|------|---------|----------|-------------------|
| 1 | 0% (green desplegado) | 2 min | Healthcheck OK |
| 2 | 5% (canary) | 10 min | error rate < 1%, p95 < 500ms |
| 3 | 25% | 30 min | Métricas estables |
| 4 | 100% | — | Deploy completo; blue standby |

### Rollback Procedure
1. `./scripts/deploy_rollback.sh --target blue`
2. Service selector vuelve a `slot=blue` (K8s aplica en < 5s)
3. Verificar healthcheck en blue
4. Green se mantiene 30 min para re-deploy rápido

## Definition of Done

- [ ] `specs/026-blue-green-canary.md` (este archivo)
- [ ] Blue/Green deploy funcional en K8s
- [ ] Canary con validación automática de métricas
- [ ] Rollback automático por métricas
- [ ] Rollback manual en < 60s
- [ ] Zero-downtime verificado (k6 sin errores)
- [ ] Métricas de deploy en Prometheus
- [ ] Tests: unit + integration + chaos + load
- [ ] Documentación: `docs/DEPLOY_STRATEGY.md`
- [ ] ROADMAP, CHANGELOG actualizados

## Gate CI SPEC-026
```bash
kubectl get deployment dmart-blue -o jsonpath='{.status.readyReplicas}'  # ≥ 1
kubectl get deployment dmart-green -o jsonpath='{.status.readyReplicas}'  # ≥ 1
kubectl get svc dmart -o jsonpath='{.spec.selector.slot}'                # = "blue" o "green"
```

## Métricas
- Deploy downtime: 0s (zero-downtime)
- Rollback latency: < 60s
- Canary validation: 100% automática
- Error rate during deploy: < 0.1%
