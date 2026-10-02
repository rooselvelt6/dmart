# SurrealDB Cluster — Alta Disponibilidad

> **Nota**: dMart **no incluye Docker**. El cluster de SurrealDB es infraestructura
> separada que debe desplegarse independientemente (VMs, Kubernetes, o bare metal).

---

## Opción A: Kubernetes (recomendado para producción)

El Helm chart en `helm/dmart/` incluye un StatefulSet para SurrealDB.

```yaml
# values.yaml
surrealdb:
  replicas: 3
  resources:
    limits:
      memory: "2Gi"
      cpu: "1000m"
  persistence:
    size: 20Gi
```

### Pod Disruption Budget

```yaml
# PDB para dmart-server (incluido en chart)
minAvailable: 1

# PDB adicional para SurrealDB cluster
apiVersion: policy/v1
kind: PodDisruptionBudget
metadata:
  name: surrealdb-pdb
spec:
  minAvailable: 2
  selector:
    matchLabels:
      app: surrealdb
```

---

## Opción B: Bare metal / VMs (sin Kubernetes)

Consultar la [documentación oficial de SurrealDB clustering](https://surrealdb.com/docs/deployment/clustering):

1. 3+ nodos con `surreal start --bind 0.0.0.0:8000 --cluster`
2. Configurar `RAFT` peers entre nodos
3. dMart se conecta via `SURREALDB_URL=ws://node1:8000,ws://node2:8000,ws://node3:8000`

---

## Opción C: Docker Compose (solo desarrollo/testing)

> Requiere Docker instalado — **no usar en producción** si evitas Docker.

```bash
# docker-compose.cluster.yml debe crearse manualmente
# Ver: https://surrealdb.com/docs/deployment/docker#clustering
```

---

## Verificación de salud

```bash
# Health check individual
curl -sf http://node1:8000/health && echo "node-1: OK"
curl -sf http://node2:8000/health && echo "node-2: OK"
curl -sf http://node3:8000/health && echo "node-3: OK"
```

## Failover

Al caer un nodo, los 2 restantes mantienen quorum y continúan sirviendo.
El líder fallido es re-electo automáticamente en < 30 segundos.

---

## Referencia

- Spec: `specs/023-surrealdb-cluster.md`
- Helm Chart: `helm/dmart/templates/statefulset-surrealdb.yaml`
- Docs oficiales: https://surrealdb.com/docs/deployment/clustering