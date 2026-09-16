# SurrealDB Cluster — Alta Disponibilidad

## Levantar el cluster localmente

```bash
docker compose -f docker-compose.cluster.yml up -d
```

Esto levanta 3 nodos SurrealDB en un cluster:

| Nodo | Puerto host | Rol inicial |
|------|-------------|-------------|
| surrealdb-node-1 | 8001 | Leader |
| surrealdb-node-2 | 8002 | Follower |
| surrealdb-node-3 | 8003 | Follower |

## Verificar salud del cluster

```bash
# Verificar que los 3 nodos estén corriendo
docker compose -f docker-compose.cluster.yml ps

# Health check individual
echo > /dev/tcp/localhost/8001 && echo "node-1: OK"
echo > /dev/tcp/localhost/8002 && echo "node-2: OK"
echo > /dev/tcp/localhost/8003 && echo "node-3: OK"
```

## Conexión

```bash
# Conectar a cualquier nodo
surreal sql --endpoint ws://localhost:8001 --username dmart --password changeme --namespace dmart --database dmart
```

## Failover

Al caer un nodo, los 2 restantes mantienen quorum y continúan sirviendo.
El líder fallido es re-electo automáticamente en < 30 segundos.

```bash
# Simular caída
docker stop surrealdb-node-1

# Verificar que node-2 o node-3 asumen liderazgo
docker logs surrealdb-node-2 2>&1 | tail -5

# Restaurar
docker start surrealdb-node-1
```

## En Kubernetes (Helm Chart)

El chart `helm/dmart/` incluye un StatefulSet para SurrealDB.
Para escalar a 3 nodos en K8s, ajustar `surrealdb.replicas: 3` en values.

### Pod Disruption Budget

El chart incluye un PDB con `minAvailable: 1` para dmart-server.
Para SurrealDB en cluster, configurar PDB adicional con `minAvailable: 2`.

```yaml
# En values.yaml override
surrealdb:
  replicas: 3
```

## Referencia

- Spec: `specs/023-surrealdb-cluster.md`
- Helm Chart: `helm/dmart/templates/statefulset-surrealdb.yaml`
- Docker Compose: `docker-compose.cluster.yml`
