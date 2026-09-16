# SPEC-023: SurrealDB Cluster — HA Database (3+ nodos)

## Contexto

- **Problema a resolver**: SurrealDB corre como instancia single (SPEC-012/021), punto único de fallo. Para producción hospitalaria se necesita alta disponibilidad: replicación, failover automático, y tolerancia a la pérdida de 1 nodo sin downtime.
- **Usuario objetivo**: DevOps / SRE / Backend
- **Métrica de éxito (KPI)**: Cluster de 3 nodos sobrevive a la caída de 1 nodo; failover automático < 30s; 0 pérdida de datos; RPO=0, RTO<30s

## Acceptance Criteria (Gherkin)

```gherkin
Feature: SurrealDB Cluster HA
  As a SRE
  I want un cluster de SurrealDB con 3+ nodos y replicación
  So that la pérdida de 1 nodo no cause downtime ni pérdida de datos

  Scenario: Cluster de 3 nodos saludable
    Given 3 nodos SurrealDB configurados (node-1, node-2, node-3)
    When verifico el estado del cluster
    Then los 3 nodos reportan "healthy"
    And el líder (leader) es electo
    And la replicación está activa (data replicado a ≥ 2 nodos)

  Scenario: Failover automático por caída de nodo
    Given cluster con 3 nodos, node-1 es leader
    When node-1 cae (kill -9 o network partition)
    Then dentro de 30s un nuevo leader es electo (node-2 o node-3)
    And las operaciones de escritura continúan sin interrupción
    And las lecturas continúan (posible stale read < 5s)
    And no hay pérdida de datos confirmados

  Scenario: Recovery de nodo
    Given node-1 fue removido del cluster
    When node-1 vuelve a estar disponible
    Then node-1 se reincorpora al cluster automáticamente
    And sincroniza datos faltantes (catch-up replication)
    And vuelve a ser follower

  Scenario: Escritura bajo replicación
    Given cluster de 3 nodos saludable
    When escribo un registro con `CREATE patient`
    Then el registro es persistido en ≥ 2 nodos antes de confirmar
    And lectura desde cualquier nodo retorna el dato

  Scenario: Lectura desde follower
    Given cluster con 3 nodos, datos actualizados en leader
    When leo desde un follower (< 5s después de escritura)
    Then recibo el dato correcto (eventual consistency)
    When leo con `READONLY` consistency
    Then recibo el dato más reciente del leader

  Scenario: Split-brain protection
    Given 3 nodos con red particionada (2 vs 1)
    Then la partición con 2 nodos mantiene servicio
    And la partición con 1 nodo entra en "read-only mode"
    When la partición se resuelve
    Then todos los nodos convergen al mismo estado
```

## API Contracts

### SurrealDB Cluster Config

```bash
# node-1 (leader inicial)
surreal start \
  --cluster dmart-cluster \
  --node node-1 \
  --bind 0.0.0.0:8000 \
  --relationships /data \
  --strict \
  --log info

# node-2
surreal start \
  --cluster dmart-cluster \
  --node node-2 \
  --bind 0.0.0.0:8000 \
  --relationships /data \
  --strict \
  --log info

# node-3
surreal start \
  --cluster dmart-cluster \
  --node node-3 \
  --bind 0.0.0.0:8000 \
  --relationships /data \
  --strict \
  --log info
```

### Environment Variables

```bash
# dmart-server config
DMART_SURREALDB_URL="ws://surrealdb-svc:8000/rpc"  # ServiceMesh / headless DNS
DMART_SURREALDB_NS="dmart"
DMART_SURREALDB_DB="dmart"
DMART_SURREALDB_USER="dmart"
DMART_SURREALDB_PASS="${SURREALDB_PASSWORD}"

# Cluster config
DMART_SURREALDB_CLUSTER="dmart-cluster"
DMART_SURREALDB_NODE="auto"  # auto-detect desde hostname
```

### K8s StatefulSet

```yaml
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: surrealdb
  namespace: dmart
spec:
  serviceName: surrealdb-headless
  replicas: 3
  selector:
    matchLabels:
      app: surrealdb
  template:
    metadata:
      labels:
        app: surrealdb
    spec:
      containers:
        - name: surrealdb
          image: surrealdb/surrealdb:latest
          args:
            - start
            - --cluster
            - dmart-cluster
            - --node
            - $(POD_NAME)
            - --bind
            - 0.0.0.0:8000
            - --relationships
            - /data
            - --strict
          env:
            - name: POD_NAME
              valueFrom:
                fieldRef:
                  fieldPath: metadata.name
          ports:
            - containerPort: 8000
              name: rpc
          volumeMounts:
            - name: data
              mountPath: /data
          readinessProbe:
            tcpSocket:
              port: 8000
            initialDelaySeconds: 10
            periodSeconds: 5
          livenessProbe:
            tcpSocket:
              port: 8000
            initialDelaySeconds: 30
            periodSeconds: 10
  volumeClaimTemplates:
    - metadata:
        name: data
      spec:
        accessModes: ["ReadWriteOnce"]
        resources:
          requests:
            storage: 20Gi
```

### Headless Service

```yaml
apiVersion: v1
kind: Service
metadata:
  name: surrealdb-headless
  namespace: dmart
spec:
  clusterIP: None
  selector:
    app: surrealdb
  ports:
    - port: 8000
      name: rpc
```

## Data Models

### StatefulSet Topology

| Pod | Hostname | Rol Inicial | Persistencia |
|-----|----------|-------------|--------------|
| surrealdb-0 | node-1 | Leader | PVC 20Gi |
| surrealdb-1 | node-2 | Follower | PVC 20Gi |
| surrealdb-2 | node-3 | Follower | PVC 20Gi |

### Pod Disruption Budget

```yaml
apiVersion: policy/v1
kind: PodDisruptionBudget
metadata:
  name: surrealdb-pdb
  namespace: dmart
spec:
  minAvailable: 2
  selector:
    matchLabels:
      app: surrealdb
```

## Edge Cases

| # | Caso | Comportamiento esperado |
|---|------|------------------------|
| 1 | 2 nodos caen simultáneamente | 1 nodo restante → read-only; no write possible |
| 2 | Red particionada (1-1-1) | Quorum no alcanzable; servicio degradado |
| 3 | Líder term-sin renewal | Nuevo leader electo en < 30s |
| 4 | Disk full en 1 nodo | Nodo expulsado del cluster; PDB mantiene 2 |
| 5 | Versión mismatch entre nodos | Versión mínima compartida; upgrade rolling |
| 6 | PVC deleted | Nodo se reincorpora con data reconstruida de peers |
| 7 | Clock skew entre nodos | Reloj lógico (Lamport/Raft); sin dependencia NTP |

## Security Considerations

### Threat Model (STRIDE)

| Threat | Mitigación |
|--------|------------|
| Spoofing | Autenticación entre nodos (TLS mTLS interno) |
| Tampering | Raft log append-only; checksums en replicación |
| Repudiation | Audit log en cada nodo; correlación cross-node |
| Information Disclosure | Datos en-at-rest encryption (LUKS/dm-crypt) |
| Denial of Service | PDB (minAvailable=2); rate limit en conexiones |
| Elevation of Privilege | RBAC SurrealDB entre nodos; secrets en K8s Secrets |

### Network Encryption

```yaml
# TLS entre nodos (auto-signed interno)
surreal start \
  --tls-cert /certs/tls.crt \
  --tls-key /certs/tls.key \
  --tls-ca /certs/ca.crt
```

### Data Classification
- [x] PHI (Protected Health Information) — datos clínicos
- [x] Clinical Data — mediciones, scores, timeline
- [x] Operational/Metadata — configs, logs, métricas

## Testing Strategy

### Unit Tests
- [ ] StatefulSet genera 3 pods con hostnames correctos
- [ ] Headless Service resuelve a IPs de pods
- [ ] PDB permite max 1 disruption a la vez

### Integration Tests
- [ ] 3 nodos arrancan y forman cluster (kind)
- [ ] Escritura en nodo-1 visible en nodo-2
- [ ] Kill nodo-1 → failover en < 30s
- [ ] Recovery nodo-1 → sincronización automática
- [ ] Split-brain simulation (network partition)

### Chaos Tests
- [ ] `chaos-mesh`: kill pod random cada 5 min
- [ ] Network latency injection (100ms entre nodos)
- [ ] Disk pressure en 1 nodo

### Load Test
- [ ] 1000 writes/s distribuidos en 3 nodos
- [ ] Read scaling: 3x throughput con reads desde followers

## Rollout Plan

### Feature Flag
```yaml
surrealdb:
  cluster:
    enabled: true
    replicas: 3
```

### Canary Deployment
| Paso | Tráfico | Duración | Criterio go/no-go |
|------|---------|----------|-------------------|
| 1 | 1 nodo (single, como hoy) | — | Baseline |
| 2 | 2 nodos (replicación) | 24h | Replicación funciona |
| 3 | 3 nodos (cluster completo) | 48h | Failover probado |
| 4 | Chaos testing | 1 semana | 0 pérdida de datos |

### Rollback Procedure
1. Reducir a 1 nodo: `kubectl scale statefulset surrealdb --replicas=1`
2. Verificar datos intactos en nodo restante
3. Volver a modo single (SPEC-012)
4. Si datos corruptos: restore desde backup (SPEC-009)

## Definition of Done

- [ ] `specs/023-surrealdb-cluster.md` (este archivo)
- [ ] StatefulSet de 3 nodos en K8s
- [ ] Failover automático probado (< 30s)
- [ ] Recovery automático probado
- [ ] PDB: minAvailable=2
- [ ] TLS entre nodos
- [ ] Tests: unit + integration + chaos
- [ ] Documentación: `docs/SURREALDB_CLUSTER.md`
- [ ] ROADMAP, CHANGELOG actualizados

## Gate CI SPEC-023
```bash
kubectl get statefulset surrealdb -o jsonpath='{.status.readyReplicas}'  # = 3
kubectl get pods -l app=surrealdb --no-headers | wc -l                   # = 3
kubectl exec surrealdb-0 -- surreal version                             # OK
```

## Métricas
- Failover latency: < 30s (leader election)
- Replication lag: < 1s (p99)
- Data durability: RPO=0 (replicado antes de ACK)
- Cluster recovery: < 2 min (nodo vuelve a follower)
