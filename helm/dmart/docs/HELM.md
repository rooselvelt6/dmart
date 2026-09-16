# Helm Chart — dMart

## Quick Start

```bash
# Instalación limpia
helm install dmart ./helm/dmart --namespace dmart --create-namespace

# Verificar pods
kubectl get pods -n dmart

# Verificar health
kubectl port-forward svc/dmart-server-svc 3000:3000 -n dmart
curl http://localhost:3000/obs/health
```

## Custom Values

Crear un archivo `my-values.yaml` con overrides:

```yaml
replicaCount: 3

image:
  tag: "v1.2.0"

autoscaling:
  enabled: true
  minReplicas: 3
  maxReplicas: 10

surrealdb:
  persistence:
    size: 50Gi

ingress:
  hosts:
    - host: dmart.mi-hospital.cu
      paths:
        - path: /
          pathType: Prefix
  tls:
    - secretName: dmart-tls-prod
      hosts:
        - dmart.mi-hospital.cu
```

```bash
helm install dmart ./helm/dmart -f my-values.yaml --namespace dmart --create-namespace
```

## Upgrade

```bash
helm upgrade dmart ./helm/dmart --namespace dmart -f my-values.yaml
```

## Rollback

```bash
# Ver historial
helm history dmart -n dmart

# Rollback a revisión específica
helm rollback dmart <revision> -n dmart

# Verificar post-rollback
kubectl get pods -n dmart
curl http://localhost:3000/obs/health
```

## Uninstall

```bash
helm uninstall dmart -n dmart
kubectl delete pvc -l app.kubernetes.io/instance=dmart -n dmart
```

## Configuration Reference

| Parameter | Description | Default |
|---|---|---|
| `replicaCount` | Number of dmart-server replicas | `2` |
| `image.repository` | Server image repository | `ghcr.io/ucigtm/dmart-server` |
| `image.tag` | Server image tag | `latest` |
| `resources.requests.cpu` | CPU request | `250m` |
| `resources.requests.memory` | Memory request | `256Mi` |
| `resources.limits.cpu` | CPU limit | `500m` |
| `resources.limits.memory` | Memory limit | `512Mi` |
| `autoscaling.enabled` | Enable HPA | `true` |
| `autoscaling.minReplicas` | HPA min replicas | `2` |
| `autoscaling.maxReplicas` | HPA max replicas | `8` |
| `autoscaling.targetCPUUtilizationPercentage` | Target CPU % | `70` |
| `surrealdb.enabled` | Deploy SurrealDB StatefulSet | `true` |
| `surrealdb.persistence.size` | PVC size | `10Gi` |
| `surrealdb.replicas` | SurrealDB replicas | `1` |
| `ingress.enabled` | Enable Ingress | `true` |
| `ingress.hosts[0].host` | Ingress hostname | `dmart.hospital.cu` |

## Secrets

Secrets are stored in a Kubernetes Secret. **Do not commit real values to Git.**

```bash
kubectl create secret generic dmart-secret \
  --from-literal=DMART_MASTER_KEY=your-real-key \
  --from-literal=DMART_SURREALDB_USER=dmart \
  --from-literal=DMART_SURREALDB_PASS=your-real-password \
  -n dmart
```

Or override in `my-values.yaml`:

```yaml
secretEnv:
  DMART_MASTER_KEY: "your-real-key"
  DMART_SURREALDB_USER: "dmart"
  DMART_SURREALDB_PASS: "your-real-password"
```
