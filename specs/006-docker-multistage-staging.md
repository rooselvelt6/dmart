# SPEC-006: Docker Multi-stage + Healthcheck + Staging Compose

## Contexto
- **Problema**: el Dockerfile existente era de un solo stage (~1.2GB) y el repositorio
  no tenía compose de staging reproducible. Parte ya estaba implementada (Dockerfile
  multi-stage real), pero con **healthcheck apuntando a un endpoint inexistente**
  (`/api/health` en vez de `/obs/health`) y sin `docker-compose.staging.yml`.
- **Usuario objetivo**: DevOps / Release Engineer
- **Métrica de éxito (KPI)**: imagen final < 100MB, `docker compose -f
  docker-compose.staging.yml up -d --build` arranca con todos los healthchecks
  `healthy`. Esto cubre R1/R4 del ROADMAP (criterio Go/No-Go de piloto robusto).

## Decisión de arquitectura (cambio respecto al draft original)
- **NO hay container SurrealDB**: el server usa **DB embebida en archivo**
  (`DMART_DB_PATH=/app/data/dmart.db`), persistida en un volumen named.
- **El frontend lo sirve el propio server** con `ServeDir` (main.rs:199) + handler
  SPA para rutas cliente. **No hay nginx**: en prod el reverse proxy es Caddy
  (docker-compose.prod.yml). La tabla `nginx.staging.conf` del draft se descarta.
- **Valkey es opcional y degrada con warn** (cache.rs): sirve para listas de
  revocación JWT distribuidas y caché de consultas; sin él el server arranca igual.

## Acceptance Criteria (Gherkin)

```gherkin
Feature: Docker Multi-stage + Staging Environment
  As a DevOps Engineer
  I want optimized Docker images and reproducible staging
  So that deployments are fast, secure, and reliable

  Scenario: Multi-stage build produces small image
    Given Dockerfile with builder + runner stages
    When `docker build -t dmart-server:staging .`
    Then image size < 100MB
    And only runtime deps in final stage (no cargo, no rustc)
    And non-root user (uid 1000)
    And read-only rootfs (read_only: true en compose)

  Scenario: Healthcheck valid (endpoint real)
    Given container running
    When healthcheck interval elapses
    Then `curl http://localhost:3000/obs/health` returns 200 with "status":"healthy"
    And healthcheck status = "healthy" (start_period 60s)

  Scenario: Staging compose 1-click deploy
    Given `docker-compose.staging.yml` with server + valkey
    When `docker compose -f docker-compose.staging.yml up -d --build`
    Then all services start (server depends on valkey started)
    And healthchecks pass
    And API+SPA accessible at http://localhost:3000

  Scenario: Persistence across restarts
    Given staging compose with named volume (dmart-data)
    When container restarted
    Then data persists (patients, measurements, ML model)
    And no data loss

  Scenario: Valkey/Redis cache available in staging
    Given staging compose
    When API starts with DMART_VALKEY_URL=redis://valkey:6379
    Then JWT revocation is propagated to Valkey (auth.rs)
    And server still starts if Valkey is down (warn, cache optional)

  Scenario: Frontend served by the server (production-like)
    Given staging compose
    When accessing http://localhost:3000
    Then index.html returned for SPA routes (fallback)
    And static assets (.wasm/.js/.css) served with correct MIME (application/wasm)
```

## API Contracts
N/A — Infraestructura, no API pública.

## Data Models

### Dockerfile (real, multietapa — builder-server + builder-wasm + runner)
```dockerfile
FROM rust:1.98-slim-bookworm AS builder-server
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY dmart-shared/ dmart-shared/
COPY dmart-server/ dmart-server/   # incluye fuzz/ (miembro del workspace)
COPY dmart-app/ dmart-app/         # miembro del workspace: necesario para resolución cargo
RUN cargo build --release --package dmart-server

FROM rust:1.98-slim-bookworm AS builder-wasm
RUN apt-get update && apt-get install -y pkg-config libssl-dev wget && rm -rf /var/lib/apt/lists/*
# trunk prebuilt: `cargo install trunk` no compila con rustc 1.98 (cssparser/parcel_selectors)
RUN wget -q https://github.com/trunk-rs/trunk/releases/download/v0.21.14/trunk-x86_64-unknown-linux-gnu.tar.gz -O /tmp/trunk.tar.gz \
    && tar xzf /tmp/trunk.tar.gz -C /usr/local/bin trunk \
    && chmod +x /usr/local/bin/trunk \
    && rm /tmp/trunk.tar.gz \
    && rustup target add wasm32-unknown-unknown
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY dmart-shared/ dmart-shared/
COPY dmart-app/ dmart-app/
COPY dmart-server/ dmart-server/   # resolución de workspace para trunk
COPY dmart-app/Trunk.toml dmart-app/
COPY dmart-app/index.html dmart-app/
COPY dmart-app/input.css dmart-app/
COPY dmart-app/tailwind.config.js dmart-app/
RUN cd dmart-app && trunk build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates wget && rm -rf /var/lib/apt/lists/*
RUN addgroup --system dmart && adduser --system --ingroup dmart dmart
WORKDIR /app
COPY --from=builder-server /build/target/release/dmart-server /app/dmart-server
COPY --from=builder-wasm /build/dist /app/dist
RUN mkdir -p /app/data && chown -R dmart:dmart /app
USER dmart
EXPOSE 3000
ENV DMART_PORT=3000 DMART_DB_PATH=/app/data/dmart.db DMART_DIST_PATH=/app/dist
HEALTHCHECK --interval=30s --timeout=5s --retries=3 \
  CMD wget -qO- http://localhost:3000/obs/health | grep -q '"status":"healthy"' || exit 1
ENTRYPOINT ["/app/dmart-server"]
```

> ⚠️ **Lección del build**: cargo en modo workspace exige **el manifest de TODOS los
> miembros** (`dmart-app`, `dmart-server/fuzz`). Falta cualquiera → `failed to load
> manifest for workspace member`. Y `dmart-server/fuzz/target` (3.5GB) puede omitirse
> del contexto con `.dockerignore`.
>
> ⚠️ **Trunk 0.21 + wasm-opt**: la opción `wasm_opt = false` de `Trunk.toml`
> desapareció en 0.21 (siempre corre wasm-opt en `--release`, salvo
> `data-wasm-opt="0"` en el `<link rel="rust">` de `index.html`). Con rustc 1.98 el
> módulo emite `memory.copy` sin declarar `bulk-memory` y binaryen lo rechaza →
> `data-wasm-opt="0"` evita el paso (la optimización real queda para SPEC-011). El
> intento previo con `RUSTFLAGS=-C target-feature=-bulk-memory` es contraproducente.
> **Ruta de dist**: `Trunk.toml` define `dist = "../dist"` (relativo a `dmart-app/`) ⇒
> en el builder la salida queda en `/build/dist`, no `/build/dmart-app/dist`.

### docker-compose.staging.yml (real)
`server` (build Dockerfile, `read_only: true`, `cap_drop: [ALL]`, `no-new-privileges`,
`tmpfs /tmp`, volumen `dmart-data:/app/data`) + `valkey` (8-alpine, `appendonly`,
`maxmemory 256mb allkeys-lru`, volumen propio). Puerta env-definible
`STAGING_PORT` (default 3000) para evitar choques con dev local. **Fail-fast**:
`DMART_MASTER_KEY:?` (crypto.rs lo exige en produccion de todos modos).

### .env.staging.example
`DMART_MASTER_KEY` (obligatoria, `openssl rand -hex 48`), `DMART_ADMIN_PASSWORD`
(opcional; vacía ⇒ password temporal de 1 uso), costes Argon2id por defecto. `.env.staging`
queda en `.gitignore`.

## Edge Cases
| # | Caso | Comportamiento esperado |
|---|------|------------------------|
| 1 | DB embebida lenta al abrir (WAL/first init) | `start_period 60s` en healthcheck + `restart: unless-stopped` |
| 2 | Valkey caído / OOM | `maxmemory 256mb` + LRU; server arranca con warn (cache opcional) |
| 3 | SPA routing (refresh en /patients/{id}) | `ServeDir` fallback en main.rs:199 devuelve index.html |
| 4 | MIME `.wasm` | `tower_http::services::ServeDir` → mime_guess → `application/wasm` |
| 5 | Secrets | Vars desde `.env.staging` (nunca en imagen); `.env.staging` ignorado por git |
| 6 | Puerto 3000 ocupado (dev local) | `STAGING_PORT=8000 docker compose ... up -d` |
| 7 | Workspace members ausentes en contexto | Dockerfile copia `dmart-app/` y `dmart-server/` (incl. fuzz) en ambas stages |

## Security Considerations
- **Non-root user**: `USER dmart` (uid 1000) en la imagen; `cap_drop: [ALL]` + `no-new-privileges` en compose
- **Read-only rootfs**: `read_only: true` (volúmenes y `tmpfs /tmp`; DMART_DB está en volumen)
- **No secrets en imagen**: `DMART_MASTER_KEY`, `DMART_ADMIN_PASSWORD` via `.env.staging`
- **Minimal base**: `debian:bookworm-slim` (libssl3/ca-certificates; sin toolchain)
- **Health endpoint público `obs`**: en prod restringir `/obs/metrics` por red (SPEC-005 security)

## Testing Strategy

### Validación local (docker real disponible)
- [x] `docker build -t dmart-server:staging .` → imagen < 100MB
- [x] `docker compose -f docker-compose.staging.yml config` → válido (fail-fast sin clave)
- [x] `curl http://localhost:${STAGING_PORT:-3000}/obs/health` → 200 + `"status":"healthy"`
- [ ] `docker compose ps` → `dmart-staging-server` y `dmart-staging-valkey` healthy
- [ ] `docker compose -f docker-compose.staging.yml down` → limpio

### CI Integration (depende de SPEC-007)
- [ ] Job `docker-build` (build + push GHCR) en cada PR
- [ ] Job `staging-deploy` en merge a main
- [ ] Smoke tests post-deploy (health + login)

## Rollout Plan
- **Feature Flag**: N/A (infra)
- **Deploy**: merge a main → CI construye y despliega staging automáticamente (SPEC-007)
- **Rollback**: `docker compose -f docker-compose.staging.yml down` + tag de imagen previo
- **Promoción**: mismo Dockerfile y vars que `docker-compose.prod.yml` (Caddy + backup)

## Definition of Done
- [x] Spec aprobada
- [x] `Dockerfile` multi-stage corrige healthcheck (`/obs/health`) y contexto workspace
- [ ] Imagen < 100MB verificada por `docker build`
- [x] `docker-compose.staging.yml` 1-click (server + valkey, healthchecks, hardened)
- [x] `.env.staging.example` + `.env.staging` en `.gitignore`
- [ ] Healthchecks verdes en staging (`docker compose ps`)
- [x] CHANGELOG.md actualizado