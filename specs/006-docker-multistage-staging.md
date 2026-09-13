# SPEC-006: Docker Multi-stage + Healthcheck + Staging Compose

## Contexto
- **Problema**: Dockerfile actual es single-stage (~1.2GB), sin healthcheck, sin staging compose reproducible. Necesario: multi-stage build (<100MB), healthchecks en compose, staging environment 1-click deploy.
- **Usuario objetivo**: DevOps / Release Engineer
- **Métrica de éxito (KPI)**: Imagen < 100MB, `docker compose -f docker-compose.staging.yml up` arranca en < 60s, healthchecks pasan.

## Acceptance Criteria (Gherkin)

```gherkin
Feature: Docker Multi-stage + Staging Environment
  As a DevOps Engineer
  I want optimized Docker images and reproducible staging
  So that deployments are fast, secure, and reliable

  Scenario: Multi-stage build produces small image
    Given Dockerfile with builder + runner stages
    When `docker build -t dmart-server:latest .`
    Then image size < 100MB
    And only runtime deps in final stage (no cargo, no rustc)
    And non-root user (uid 1000)
    And read-only rootfs

  Scenario: Healthcheck configured and passing
    Given container running
    When healthcheck interval elapses
    Then `curl -f http://localhost:3000/health` returns 200
    And healthcheck status = "healthy"
    And startup probe allows 60s grace period

  Scenario: Staging compose 1-click deploy
    Given `docker-compose.staging.yml` with all services
    When `docker compose -f docker-compose.staging.yml up -d`
    Then all services start in order (db → cache → server → frontend)
    And healthchecks pass for all
    And frontend accessible at http://localhost:8080
    And API accessible at http://localhost:3000

  Scenario: SurrealDB persistence in staging
    Given staging compose with SurrealDB volume
    When containers restarted
    Then data persists (patients, measurements, ML model)
    And no data loss

  Scenario: Valkey/Redis cache configured
    Given staging compose
    When API makes cached queries
    Then cache hit rate > 80% visible in metrics
    And session persistence works across server restarts

  Scenario: Frontend served via nginx (production-like)
    Given staging compose
    When accessing http://localhost:8080
    Then static assets served with proper caching headers
    And SPA routing works (fallback to index.html)
    And gzip/brotli compression enabled
```

## API Contracts
N/A — Infraestructura, no API pública.

## Data Models

### Dockerfile (multi-stage)
```dockerfile
# Stage 1: Builder
FROM rust:1.98-slim-bookworm AS builder
WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev libclang-dev && \
    rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock ./
COPY dmart-shared ./dmart-shared
COPY dmart-server ./dmart-server
RUN cargo build --release --bin dmart-server

# Stage 2: Runner
FROM debian:bookworm-slim AS runner
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 && \
    useradd -u 1000 -m appuser && \
    rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/dmart-server .
COPY --from=builder /app/migrations ./migrations
USER appuser
EXPOSE 3000
HEALTHCHECK --interval=30s --timeout=5s --start-period=60s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1
ENTRYPOINT ["./dmart-server"]
```

### docker-compose.staging.yml
```yaml
version: '3.8'

services:
  surrealdb:
    image: surrealdb/surrealdb:latest
    command: start --log trace --user root --pass root file:///data/dmart.db
    volumes:
      - surrealdb_data:/data
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8000/health"]
      interval: 10s
      timeout: 5s
      retries: 5
      start_period: 30s
    networks: [dmart-network]

  valkey:
    image: valkey/valkey:7-alpine
    command: valkey-server --appendonly yes --maxmemory 256mb --maxmemory-policy allkeys-lru
    volumes:
      - valkey_data:/data
    healthcheck:
      test: ["CMD", "valkey-cli", "ping"]
      interval: 10s
      timeout: 3s
      retries: 5
    networks: [dmart-network]

  server:
    build:
      context: .
      dockerfile: Dockerfile
    ports: ["3000:3000"]
    environment:
      - DMART_MASTER_KEY=${DMART_MASTER_KEY}
      - DMART_ADMIN_PASSWORD=${DMART_ADMIN_PASSWORD}
      - SURREALDB_URL=http://surrealdb:8000
      - VALKEY_URL=redis://valkey:6379
      - RUST_LOG=info
    depends_on:
      surrealdb:
        condition: service_healthy
      valkey:
        condition: service_healthy
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:3000/health"]
      interval: 30s
      timeout: 5s
      retries: 3
      start_period: 60s
    networks: [dmart-network]

  frontend:
    image: nginx:alpine
    ports: ["8080:80"]
    volumes:
      - ./dmart-app/dist:/usr/share/nginx/html:ro
      - ./nginx.staging.conf:/etc/nginx/conf.d/default.conf:ro
    depends_on:
      server:
        condition: service_healthy
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:80"]
      interval: 30s
      timeout: 3s
      retries: 3
    networks: [dmart-network]

volumes:
  surrealdb_data:
  valkey_data:

networks:
  dmart-network:
    driver: bridge
```

### nginx.staging.conf
```nginx
server {
    listen 80;
    server_name localhost;
    root /usr/share/nginx/html;
    index index.html;

    gzip on;
    gzip_types text/css application/javascript application/wasm;
    brotli on;
    brotli_types text/css application/javascript application/wasm;

    location / {
        try_files $uri $uri/ /index.html;
    }

    location /api/ {
        proxy_pass http://server:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }

    location /fhir/ {
        proxy_pass http://server:3000;
        proxy_set_header Host $host;
        proxy_set_header Accept application/fhir+json;
    }

    # WASM caching
    location ~* \.wasm$ {
        add_header Content-Type application/wasm;
        add_header Cache-Control "public, max-age=31536000, immutable";
    }
}
```

## Edge Cases
| # | Caso | Comportamiento esperado |
|---|------|------------------------|
| 1 | SurrealDB slow to start | `depends_on: condition: service_healthy` espera healthcheck |
| 2 | Valkey OOM | `maxmemory 256mb` + `maxmemory-policy allkeys-lru` |
| 3 | Frontend SPA routing | `try_files $uri $uri/ /index.html` |
| 4 | WASM MIME type | nginx sirve `.wasm` con `application/wasm` |
| 5 | Config secrets | Variables de entorno desde `.env.staging` (no en imagen) |

## Security Considerations
- **Non-root user**: `USER appuser` (uid 1000)
- **Read-only rootfs**: `read_only: true` en compose (excepto volúmenes)
- **No secrets in image**: `DMART_MASTER_KEY`, `DMART_ADMIN_PASSWORD` via env file
- **Minimal base**: `debian:bookworm-slim` (no build tools en runner)
- **Capabilities dropped**: `cap_drop: [ALL]` en compose

## Testing Strategy

### Manual Verification
- [ ] `docker build -t dmart-server:test .` → size < 100MB
- [ ] `docker compose -f docker-compose.staging.yml up -d` → all healthy
- [ ] `curl http://localhost:3000/health` → 200
- [ ] `curl http://localhost:8080` → serves index.html
- [ ] `docker compose -f docker-compose.staging.yml down -v` → clean

### CI Integration
- [ ] GitHub Actions job `docker-build` → build + push to GHCR
- [ ] Job `staging-deploy` → deploy to staging server on merge to main
- [ ] Smoke tests post-deploy

## Rollout Plan
- **Feature Flag**: N/A (infra)
- **Deploy**: Merge to main → CI builds + pushes → staging auto-deploy
- **Rollback**: `docker compose -f docker-compose.staging.yml down` + previous image tag

## Definition of Done
- [ ] Spec aprobada
- [ ] `Dockerfile` multi-stage (< 100MB)
- [ ] `docker-compose.staging.yml` funcional
- [ ] `nginx.staging.conf` con SPA routing + WASM caching
- [ ] Healthchecks pasan para todos los servicios
- [ ] `docker compose -f docker-compose.staging.yml up -d` 1-click
- [ ] CHANGELOG.md actualizado