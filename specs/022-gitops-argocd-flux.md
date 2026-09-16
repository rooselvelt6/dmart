# SPEC-022: GitOps con ArgoCD/Flux — Sync automático main→prod

## Contexto

- **Problema a resolver**: El deploy manual vía `helm upgrade` es propenso a errores y no tiene trazabilidad. Se necesita GitOps: el estado deseado vive en Git, y un controlador sincroniza automáticamente el cluster con el repositorio. Esto da auditabilidad, rollback instantáneo, y doors-bells entre ambientes.
- **Usuario objetivo**: DevOps / SRE
- **Métrica de éxito (KPI)**: Push a `main` refleja en producción en < 5 min; rollback = `git revert`; 0 deployments manuales en 30 días

## Acceptance Criteria (Gherkin)

```gherkin
Feature: GitOps con ArgoCD/Flux
  As a SRE
  I want que push a main sincronice automáticamente el cluster
  So that el despliegue sea auditable, reproducible y sin intervención manual

  Scenario: Sync automático post-push
    Given ArgoCD/Flux configurado apuntando a `helm/dmart/` en main
    When hago push a main con cambio en image.tag
    Then el controlador detecta el drift en < 2 min
    And ejecuta sync automático
    And los pods se actualizan (rolling update)
    And el estado en Git = estado del cluster

  Scenario: Rollback por git revert
    Given producción en versión v1.2.0 (commit abc123)
    When hago `git revert abc123` y push
    Then el controlador detecta el cambio en < 2 min
    And rollback automático a v1.1.0
    And 0 downtime durante rollback

  Scenario: Drift detection
    Given estadoGit=estadoCluster
    When alguien hace `kubectl edit deployment` manualmente
    Then el controlador detecta drift en < 5 min
    And revierte el cambio (self-healing)
    And notificación en canal Slack/Teams

  Scenario: Health check de ArgoCD
    Given ArgoCD desplegado en namespace `argocd`
    When accedo a `https://argocd.hospital.cu/`
    Then recibo 200 con UI funcional
    And el app de dMart muestra "Synced"

  Scenario: Ambientes separados
    Given repo con estructura `envs/staging/` y `envs/prod/`
    When push a `develop` → sync staging
    And merge a `main` → sync prod
    Then staging y prod tienen configuraciones independientes
```

## API Contracts

### Estructura del repositorio (GitOps)

```
dmart-gitops/
├── envs/
│   ├── staging/
│   │   ├── kustomization.yaml
│   │   ├── namespace.yaml
│   │   └── dmart-values.yaml
│   └── prod/
│       ├── kustomization.yaml
│       ├── namespace.yaml
│       └── dmart-values.yaml
├── base/
│   ├── helmrelease.yaml    # Flux CRD
│   ├── hr-argocd.yaml      # ArgoCD Application
│   └── ...
└── scripts/
    └── sync-check.sh
```

### ArgoCD Application

```yaml
apiVersion: argoproj.io/v1alpha1
kind: Application
metadata:
  name: dmart-prod
  namespace: argocd
spec:
  project: default
  source:
    repoURL: https://github.com/ucigtm/dmart-gitops.git
    targetRevision: main
    path: envs/prod
  destination:
    server: https://kubernetes.default.svc
    namespace: dmart
  syncPolicy:
    automated:
      prune: true
      selfHeal: true
    syncOptions:
      - CreateNamespace=true
    retry:
      limit: 3
      backoff:
        duration: 30s
        factor: 2
        maxDuration: 5m
```

### Flux HelmRelease (alternativa)

```yaml
apiVersion: helm.toolkit.fluxcd.io/v2beta2
kind: HelmRelease
metadata:
  name: dmart
  namespace: dmart
spec:
  interval: 5m
  chart:
    spec:
      chart: ./helm/dmart
      sourceRef:
        kind: GitRepository
        name: dmart-repo
  values:
    replicaCount: 3
    image:
      tag: "v1.2.0"
  install:
    remediation:
      retries: 3
  upgrade:
    remediation:
      retries: 3
      remediateLastFailure: true
```

### Monitoreo

```yaml
# ServiceMonitor para Prometheus (SPEC-005)
apiVersion: monitoring.coreos.com/v1
kind: ServiceMonitor
metadata:
  name: argocd-metrics
  namespace: argocd
spec:
  selector:
    matchLabels:
      app.kubernetes.io/name: argocd-metrics
  endpoints:
    - port: metrics
      interval: 30s
```

## Data Models

### Flux CRDs

| Resource | Kind | Namespace | Propósito |
|----------|------|-----------|-----------|
| dmart-repo | GitRepository | flux-system | Fuente Git |
| dmart | HelmRelease | dmart | Deploy Helm |
| dmart | Kustomization | flux-system | Sync config |

### ArgoCD Resources

| Resource | Kind | Namespace | Propósito |
|----------|------|-----------|-----------|
| dmart-prod | Application | argocd | Sync prod |
| dmart-staging | Application | argocd | Sync staging |
| dmart-project | AppProject | argocd | Permisos |

## Edge Cases

| # | Caso | Comportamiento esperado |
|---|------|------------------------|
| 1 | Repo inaccesible | Reintentos con backoff; alerta tras 5 min |
| 2 | Values inválido | Sync falla; estado "Degraded" en UI |
| 3 | Namespace no existe | Auto-creación (CreateNamespace=true) |
| 4 | CRD no instalado | Helm install falla con mensaje claro |
| 5 | Multi-cluster | Un ArgoCD gestiona N clusters (futuro) |
| 6 | Secret rotación | External Secrets renueva; Flux re-sincroniza |
| 7 | Conflicto staging/prod | Branch protection + PR review obligatorio |

## Security Considerations

### Threat Model (STRIDE)

| Threat | Mitigación |
|--------|------------|
| Spoofing | Git commit signing (GPG/SSH); ArgoCD SSO (OIDC) |
| Tampering | Branch protection; required reviews; signed commits |
| Repudiation | Git log inmutable; ArgoCD audit events |
| Information Disclosure | Secrets en Vault/SealedSecrets, no en Git |
| Denial of Service | ArgoCD RBAC; rate limit syncs; resource quotas |
| Elevation of Privilege | ArgoCD projects con restricto de repos/namespaces |

### Secret Management

```yaml
# SealedSecrets (Bitnami) — seguro en Git
apiVersion: bitnami.com/v1alpha1
kind: SealedSecret
metadata:
  name: dmart-secrets
spec:
  encryptedData:
    DMART_SURREALDB_USER: AgBy3i4OJSWK+..."
    DMART_SURREALDB_PASS: AgBy3i4OJSWK+..."
```

### RBAC ArgoCD

```yaml
policy.default: role:readonly
policy.csv:
  - p, role:devops, applications, get, dmart/*, allow
  - p, role:devops, applications, sync, dmart/*, allow
  - g, devops-team, role:devops
```

## Testing Strategy

### Unit Tests
- [ ] `helm template` genera YAML válido para cada env
- [ ] Flux CRDs son válidos contra schema
- [ ] ArgoCD Application genera diff correcto

### Integration Tests
- [ ] Kind cluster + ArgoCD → sync automático funciona
- [ ] Git push → pod update en < 5 min
- [ ] Git revert → rollback automático
- [ ] Drift manual → self-healing

### Conformance Tests
- [ ] `kubeconform` contra todos los manifests
- [ ] `flux diff kustomization` muestra cambios esperados

## Rollout Plan

### Feature Flag
```yaml
# Solo se activa con GitOps habilitado
gitops:
  enabled: true
  tool: argocd  # | flux
```

### Canary Deployment
| Paso | Tráfico | Duración | Criterio go/no-go |
|------|---------|----------|-------------------|
| 1 | ArgoCD instalado + app "OutOfSync" | 10 min | App detecta drift |
| 2 | Sync manual desde UI | 5 min | Pods actualizados |
| 3 | Sync automático habilitado | 24h | 0 intervenciones manuales |
| 4 | Prod GitOps completo | — | Rollback por git revert funciona |

### Rollback Procedure
1. `git revert <commit>` + push
2. ArgoCD/Flux sincroniza automáticamente
3. Verificar pods actualizados
4. Si ArgoCD roto: `helm rollback dmart <rev>` (escape hatch)

## Definition of Done

- [ ] `specs/022-gitops-argocd-flux.md` (este archivo)
- [ ] ArgoCD o Flux instalado en cluster staging
- [ ] Application/CRD apunta a repo GitOps
- [ ] Sync automático funciona (push → deploy)
- [ ] Self-healing funciona (drift revertido)
- [ ] Rollback por git revert funciona
- [ ] Branch protection + commit signing
- [ ] SealedSecrets para secrets en Git
- [ ] Tests: kind e2e + diff validation
- [ ] Documentación: `docs/GITOPS.md`
- [ ] ROADMAP, CHANGELOG actualizados

## Gate CI SPEC-022
```bash
kubectl get application dmart-prod -o jsonpath='{.status.sync.status}'  # = "Synced"
kubectl get application dmart-prod -o jsonpath='{.status.health.status}'  # = "Healthy"
```

## Métricas
- Sync latency: < 5 min (push a pod actualizado)
- Rollback latency: < 5 min (git revert a pod funcionando)
- Self-healing latency: < 10 min (drift detectado → revertido)
- Deployment frequency target: ≥ 1/día (continuous)
