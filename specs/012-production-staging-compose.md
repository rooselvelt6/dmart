# SPEC-012: Producción/Staging Compose (`docker-compose.prod.yml`)

## Contexto
- **Problema a resolver**: el `docker-compose.prod.yml` heredado es un borrador que
  **no arranca** (heredó los bugs de SPEC-006 antes del fix): healthcheck a
  `/api/health` (inexistente), Valkey en crash-loop por falta de caps, sin `tmpfs`
  ni `read_only`, y un "backup" vía `cp` que copia la base de datos **viva**
  (snapshot inconsistente). Además el `Caddyfile` fuerza `redir https`, lo que rompe
  el piloto en una LAN de hospital sin dominio público ni certificado.
- **Usuario objetivo**: DevOps / Release Engineer (piloto: UCI de 5 camas en LAN).
- **Métrica de éxito (KPI)**: `docker compose -f docker-compose.prod.yml up -d`
  reproducible en staging; todos los servicios `healthy`; frontend accesible vía
  Caddy; persistencia tras restart. Cubre el criterio **R4** del ROADMAP
  (deploy/rollback reproducible, imagen ≈50MB).

## Decisión de arquitectura
- **Sin nginx**: el frontend lo sirve el server (`ServeDir`), como en SPEC-006.
  Caddy actúa solo de reverse proxy + TLS opcional.
- **TLS opcional y parametrizado** (`SITE_ADDRESS`): por defecto `:80` (HTTP en
  LAN del piloto). Con `SITE_ADDRESS=dmart.dominio-hospitalero@example.net` se
  activa auto-HTTPS de Caddy (Let's Encrypt) sin tocar más nada.
- **Sin servicio `backup`**: el `cp` sobre BD viva es inconsistent and peligroso.
  El backup real es SPEC-009 (Backup automático SurrealKV). Aquí solo se dejan los
  volúmenes named (`dmart-data`, `valkey-data`) para que 009 los respalde.
- **Server sin puerto publicado directo**: solo Caddy publica 80/443 en prod
  (producción-like); el debug directo queda para staging compose (SPEC-006).
- **Refuerzo de contenedores**: `read_only`, `cap_drop: ALL` + `cap_add` mínimo
  (Valkey necesita `SETUID/SETGID/CHOWN/FOWNER/DAC_OVERRIDE` para `setpriv`;
  Caddy necesita `NET_BIND_SERVICE` para puertos <1024), `no-new-privileges`,
  `tmpfs /tmp`, `logging` con rotación, límites de memoria/CPU, healthchecks en
  los 3 servicios.

## Acceptance Criteria (Gherkin)

```gherkin
Feature: Deploy reproducible en staging/producción

  Scenario: Despliegue completo 1-click
    Given .env.prod configurado (DMART_MASTER_KEY fuerte)
    When `docker compose -f docker-compose.prod.yml up -d --build`
    Then los 3 servicios quedan healthy (proxy, server, valkey)
    And el frontend responde HTTP 200 vía Caddy (http://localhost:80)

  Scenario: Healthchecks válidos
    Given contenedores en ejecución
    When pasa un intervalo del healthcheck
    Then server valida `/obs/health` → "status":"healthy"
    And valkey responde `PING` → `PONG`
    And caddy responde HTTP 200 en la ruta raíz

  Scenario: Persistencia tras restart
    Given volumen named `dmart-data`
    When se reinicia el server (`docker compose restart dmart-server`)
    Then la BD no se re-seedea (Found N users, skipping seed)
    And el healthcheck vuelve a healthy

  Scenario: Valkey acotado en memoria
    Given valkey con config `maxmemory 256mb allkeys-lru`
    When el server usa revocación JWT y caché
    Then la memoria del contenedor se mantiene bajo el límite (`mem_limit`)
```

## API Contracts
N/A — Infraestructura, no API pública. Los contratos HTTP los expone el server
(SPEC-006: `/obs/health`, `/obs/live`, `/obs/ready`, `/obs/metrics`, SPA).

## Data Models
N/A — Sin esquemas nuevos. Persistencia: volumen named `dmart-data` (SurrealKV
embebido) y `valkey-data` (AOF).

## Edge Cases
| # | Caso | Comportamiento esperado |
|---|------|------------------------|
| 1 | `DMART_MASTER_KEY` faltante o débil | Compose falla fail-fast (`${VAR:?...}`) y crypto.rs aborta |
| 2 | Sin dominio público (LAN piloto) | `SITE_ADDRESS=:80`, HTTP directo, sin certificados |
| 3 | Valkey caído al arrancar | Server inicia igual (warn, cache opcional); `restart` relanza valkey |
| 4 | Port 3000 ocupado en el host | El server interno no publica puerto en prod; solo Caddy 80/443 |
| 5 | DB crece en el piloto | `dmart-data` es volumen named; backup/retentón en SPEC-009/030 |
| 6 | Crashloop por cambio malo | `restart: unless-stopped` + validación manual en staging antes de prod |

## Security Considerations

### Threat Model (STRIDE)
| Threat | Mitigación |
|--------|------------|
| Spoofing | Caddy X-Forwarded-For; `DMART_TRUST_PROXY=true` solventa el real-cliente |
| Tampering | `read_only` rootfs, `cap_drop: ALL`, volúmenes solo-write en `/app/data` y `/data` |
| Repudiation | logging con rotación (`max-size`/`max-file`), Caddy access.log en volumen |
| Information Disclosure | HSTS tiene sentido solo sobre HTTPS → `DMART_ENABLE_HSTS` por env |
| Denial of Service | `mem_limit`/`cpus` en los 3 servicios; valkey `maxmemory+allkeys-lru`; rate-limit del server |
| Elevation of Privilege | `no-new-privileges:true`, usuario no-root en imagen final (`USER dmart`) |

### Data Classification
- [x] PHI (Protected Health Information) — en `dmart-data` (SurrealKV, cifrada en reposo con `DMART_MASTER_KEY`)
- [ ] PII
- [x] Clinical Data
- [x] Operational/Metadata

### Auth/Autz Requirements
- N/A (infra). El server mantiene su propio RBAC; el proxy no añade auth.
- `DMART_MASTER_KEY` es obligatoria; `.env.prod` está en `.gitignore`.

## Testing Strategy

### Unit Tests
- N/A (sin código Rust). La infra se valida con comandos reales.

### Integration / Validación manual (Staging)
- [ ] `docker compose -f docker-compose.prod.yml config --quiet` pasa
- [ ] `up -d --build` con los 3 servicios `healthy`
- [ ] `curl http://localhost/obs/health` vía Caddy → `"status":"healthy"`
- [ ] `curl http://localhost/` → index.html SPA (doble ruta: `/` y `/patients`)
- [ ] `curl http://localhost/obs/metrics` → serie `dmart_` expuesta
- [ ] restart server → sin re-seed, healthy
- [ ] `docker compose ps` → 0 containers en restart/crash

### Load Test (k6)
- N/A; el rendimiento se mide en ISSUE (SPEC-005 dashboards) durante el piloto.

## Rollout Plan

### Feature Flag
- N/A (no hay código de feature). Switch de rollout: `SITE_ADDRESS` (HTTP→HTTPS).

### Canary Deployment
| Paso | Tráfico | Duración | Criterio go/no-go |
|------|---------|----------|-------------------|
| 1 | Staging (LAN) | — | 3 servicios healthy 24h, R1 del ROADMAP |
| 2 | Piloto 5 camas | 30 días | Sin intervención del desarrollador (R1) |

### Rollback Procedure (<15 min)
1. `docker compose -f docker-compose.prod.yml down` (no toca volumes named).
2. `git checkout <tag-previo>` (o `docker tag dmart-server:prod-vX` y editar `image:`).
3. `docker compose -f docker-compose.prod.yml up -d` → los datos persisten en
   `dmart-data`/`valkey-data` (rollback de DB no hace falta: el fresh `up` del
   Dockerfile NO toca el volumen).

## Definition of Done
- [x] Spec redactada
- [ ] `docker-compose.prod.yml` + `Caddyfile` + `.env.prod.example` versionados
- [ ] `docker compose -f docker-compose.prod.yml config --quiet` pasa
- [ ] Validación manual completa (sección Testing) ✅
- [ ] `CHANGELOG.md` y `ROADMAP.md` al día (SPEC-012 DONE, fila 6.8)