# SPEC-021: Kubernetes Helm Chart — Deploy HA en producción

## Contexto

- **Problema a resolver**: dMart se despliega vía `docker compose` (SPEC-012), válido para staging y piloto de 5 camas. Para escalar a producción hospitalaria con alta disponibilidad se necesita Kubernetes: auto-scaling, self-healing, rolling updates, y gestión declarativa de recursos.
- **Usuario objetivo**: DevOps / SRE / Ingeniería clínica
- **Métrica de éxito (KPI)**: `helm install dmart` despliega el sistema completo en < 3 min; 0 downtime en rolling update; auto-scaling de 2→8 pods bajo carga; healthcheck K8s pasa en < 10s

## Acceptance Criteria (Gherkin)

```gherkin
Feature: Kubernetes Helm Chart
  As a operador de producción
  I want desplegar dMart en K8s con un solo comando
  So that el sistema sea resiliente, escalable y con zero-downtime

  Scenario: Instalación limpia
    Given un cluster K8s 1.28+ disponible
    When ejecuto `helm install dmart ./helm/dmart --namespace dmart --create-namespace`
    Then los pods dmart-server (≥2 replicas) pasan healthcheck en < 60s
    And el endpoint /obs/health retorna 200
    And SurrealDB (StatefulSet) está corriendo con PVC de 10Gi
    And el servicio interno dmart-server:8080 es alcanzable

  Scenario: Rolling update sin downtime
    Given dmart desplegado con 3 réplicas
    When actualizo la imagen a nueva versión (`helm upgrade`)
    Then el endpoint /obs/health nunca retorna 5xx durante el upgrade
    And al menos 2 réplicas activas en todo momento
    And los pods viejos terminan graceful (terminationGracePeriodSeconds=30)

  Scenario: Horizontal Pod Autoscaler
    Given HPA configurado (min=2, max=8, target CPU=70%)
    When la carga sube a 80% CPU sostenido 2 min
    Then HPA escala a ≥ 4 pods
    When la carga baja a 20% CPU sostenido 5 min
    Then HPA reduce a 2 pods

  Scenario: Resource limits
    Given pods con requests=256Mi/250m, limits=512Mi/500m
    When un pod excede memory limit
    Then el pod es OOMKilled y recreado automáticamente
    And laقلق se restringe al pod afectado

  Scenario: Ingress con TLS
    Given Ingress configurado con TLS (cert-manager)
    When accedo a `https://dmart.hospital.cu/`
    Then recibo 200 con certificado TLS válido
    And HTTP redirect a HTTPS funciona
```

## API Contracts

### Helm Chart values.yaml (principales)

```yaml
replicaCount: 2

image:
  repository: ghcr.io/ucigtm/dmart-server
  tag: "latest"
  pullPolicy: IfNotPresent

resources:
  requests:
    cpu: 250m
    memory: 256Mi
  limits:
    cpu: 500m
    memory: 512Mi

autoscaling:
  enabled: true
  minReplicas: 2
  maxReplicas: 8
  targetCPUUtilizationPercentage: 70

surrealdb:
  enabled: true
  persistence:
    enabled: true
    size: 10Gi
  replicas: 1  # SPEC-023 escala a 3+

ingress:
  enabled: true
  className: nginx
  annotations:
    cert-manager.io/cluster-issuer: letsencrypt-prod
  hosts:
    - host: dmart.hospital.cu
      paths:
        - path: /
          pathType: Prefix
  tls:
    - secretName: dmart-tls
      hosts:
        - dmart.hospital.cu

healthcheck:
  liveness:
    path: /obs/health
    initialDelaySeconds: 10
    periodSeconds: 15
  readiness:
    path: /obs/health
    initialDelaySeconds: 5
    periodSeconds: 10
```

### Helm Chart structure

```
helm/dmart/
├── Chart.yaml
├── values.yaml
├── templates/
│   ├── _helpers.tpl
│   ├── deployment-server.yaml
│   ├── statefulset-surrealdb.yaml
│   ├── service-server.yaml
│   ├── service-surrealdb.yaml
│   ├── ingress.yaml
│   ├── hpa.yaml
│   ├── configmap.yaml
│   ├── secret.yaml
│   ├── serviceaccount.yaml
│   └── networkpolicy.yaml
└── .helmignore
```

### ConfigMap keys

```yaml
DMART_SURREALDB_URL: "ws://surrealdb-svc:8000/rpc"
DMART_SURREALDB_NS: "dmart"
DMART_SURREALDB_DB: "dmart"
DMART_LOG_LEVEL: "info"
DMART_METRICS_ENABLED: "true"
```

## Data Models

### Kubernetes Resources

| Resource | Kind | Nombre | Réplicas |
|----------|------|--------|----------|
| dmart-server | Deployment | dmart-server | 2 (HPA: 2-8) |
| surrealdb | StatefulSet | surrealdb | 1 (SPEC-023 → 3) |
| dmart-server-svc | Service | dmart-server | ClusterIP |
| surrealdb-svc | Service | surrealdb | ClusterIP |
| dmart-ingress | Ingress | dmart | 1 |
| dmart-hpa | HorizontalPodAutoscaler | dmart-server | — |
| dmart-config | ConfigMap | dmart | 1 |
| dmart-secret | Secret | dmart | 1 |

### NetworkPolicy

```yaml
# Solo dmart-server puede acceder a SurrealDB
ingress:
  - from:
      - podSelector:
          matchLabels:
            app.kubernetes.io/name: dmart-server
    ports:
      - port: 8000
```

## Edge Cases

| # | Caso | Comportamiento esperado |
|---|------|------------------------|
| 1 | Cluster sin cert-manager | Ingress sin TLS; WARNING en helm install |
| 2 | PVC no disponible | StatefulSet Pending; server funciona sin DB (read-only mode) |
| 3 | Nodos sin recursos | Pods en Pending; HPA no escala; alerta SPEC-008 |
| 4 | Namespace ya existe | `--create-namespace` es idempotente |
| 5 | Rollback de helm | `helm rollback dmart N` restaura versión previa |
| 6 | Secret inexistente | Pods en CrashLoopBackOff; evento K8s descriptivo |
| 7 | NetworkPolicy incorrecto | SurrealDB inaccesible; server en degraded mode |

## Security Considerations

### Threat Model (STRIDE)

| Threat | Mitigación |
|--------|------------|
| Spoofing | NetworkPolicy: solo dmart-server accede a SurrealDB |
| Tampering | Secret en K8s Secret (no en ConfigMap); RBAC SA dedicado |
| Repudiation | Audit log en stdout → collected by Fluentd/FluentBit |
| Information Disclosure | SecurityContext: readOnlyRootFilesystem, noPrivilegeEscalation |
| Denial of Service | HPA + resource limits + PDB (minAvailable=1) |
| Elevation of Privilege | PodSecurityPolicy: no root, no hostNetwork, drop ALL caps |

### Pod Security

```yaml
securityContext:
  runAsNonRoot: true
  runAsUser: 1000
  fsGroup: 1000
  readOnlyRootFilesystem: true
  allowPrivilegeEscalation: false
  capabilities:
    drop: ["ALL"]
```

### Secrets Management

- credenciales SurrealDB en K8s Secret (no hardcodeadas)
- Opción未来: External Secrets Operator + Vault

## Testing Strategy

### Unit Tests (Helm)
- [ ] `helm template dmart ./helm/dmart` genera YAML válido
- [ ] Valores por defecto producen despliegue funcional
- [ ] Override de values produce cambios esperados

### Integration Tests
- [ ] `kind create cluster` + `helm install` → pods healthy
- [ ] Rolling update con tráfico simultáneo (k6)
- [ ] HPA responde a carga (curl + watch pods)

### Conformance Tests
- [ ] `kubeconform` / `kube-linter` contra chart generado
- [ ] `ct lint` (chart-testing) pasa sin warnings

### Load Test (k6)
- [ ] 100 VUs, 5 min, p95 < 500ms en K8s
- [ ] Scale-up/down observado en métricas HPA

## Rollout Plan

### Feature Flag
```yaml
# values.yaml
enabled: true
```

### Canary Deployment
| Paso | Tráfico | Duración | Criterio go/no-go |
|------|---------|----------|-------------------|
| 1 | 1 réplica + 0 tráfico | 10 min | Pods healthy, healthcheck OK |
| 2 | 2 réplicas + staging traffic | 1h | 0 errores, latencia estable |
| 3 | HPA activado + carga sintética | 30 min | Scale-up/down funciona |
| 4 | Producción + Ingress | — | TLS funciona, 0 downtime |

### Rollback Procedure
1. `helm rollback dmart <previous-revision>`
2. Verificar pods healthy
3. Verificar endpoint /obs/health 200
4. Verificar SurrealDB conectado

## Definition of Done

- [ ] `specs/021-kubernetes-helm-chart.md` (este archivo)
- [ ] Helm chart en `helm/dmart/` con Chart.yaml, values.yaml, templates/
- [ ] `helm install` funciona en cluster limpio (kind o real)
- [ ] Rolling update sin downtime verificado
- [ ] HPA funciona bajo carga
- [ ] NetworkPolicy aísla SurrealDB
- [ ] PodSecurityPolicy: no root, no privilege escalation
- [ ] Tests: helm template + kubeconform + kind e2e
- [ ] Documentación: `docs/HELM.md` con valores y personalización
- [ ] ROADMAP, CHANGELOG actualizados
- [ ] `cargo clippy -D warnings` = 0/0 (no regression)

## Gate CI SPEC-021
```bash
helm template dmart ./helm/dmart > /dev/null 2>&1  # template válido
kubeconform -summary helm/dmart/templates/          # K8s YAML válido
helm lint ./helm/dmart                               # sin warnings
```

## Métricas
- Tiempo de instalación: < 3 min (cluster limpio)
- Tiempo de rolling update: < 2 min (sin downtime)
- HPA scale-up latency: < 60s
- PDB: ≥ 1 pod disponible en todo momento
