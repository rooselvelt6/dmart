# Changelog

Todos los cambios notables de este proyecto se documentan en este archivo.

El formato se basa en [Keep a Changelog](https://keepachangelog.com/es-ES/1.0.0/),
y este proyecto se adhiere a [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased] — SPEC-006: Docker Multi-stage + Staging Compose

### Corregido
- **Healthcheck del Dockerfile roto**: apuntaba a `/api/health` (endpoint
  inexistente); ahora valida `/obs/health` (health_check en observability.rs).
  Sin esto, un container "healthy" que no lo estaba, y el staging no podía
  validarse.
- **Contexto de build del workspace**: `cargo` exige el manifest de TODOS los
  miembros; faltaba `dmart-app/` en `builder-server` y `dmart-server/` en
  `builder-wasm` → `failed to load manifest for workspace member`. Copiados ambos
  + `dmart-server/fuzz/target` (3.5GB) excluido vía `.dockerignore`.
- **Server "sordo" en producción (bug crítico)**: en `main.rs` se hacía
  `graceful_shutdown(...).await` ANTES de `server.await`; al esperar SIGTERM/SIGINT
  antes de arrancar, axum nunca empezaba a servir (el socket escuchaba a nivel
  kernel, TCP conectaba, pero toda request colgaba). Ahora el shutdown graceful va
  dentro de `with_graceful_shutdown`.
- **Doble inicialización de métricas**: `init_metrics()` se llamaba dos veces en
  `main.rs` (línea ~75 y ~136). El registrar devolvía el error "failed to install
  exporter as global recorder: metrics already initialized" y el proceso moría al
  arrancar. Eliminada la llamada huérfana.
- **`tailwind.config.js` con SyntaxError**: las keys `box-shadow` sin comillas en
  `keyframes.scorePulse` rompían el pipeline CSS de trunk. Ahora van entre comillas.
- **wasm-opt rompía el build con rustc 1.98**: el módulo usa `memory.copy` sin
  declarar `bulk-memory` y binaryen rechaza el módulo. En trunk 0.21 la opción
  `wasm_opt` ya NO existe en `Trunk.toml`; se desactiva con el atributo per-link
  `data-wasm-opt="0"` en `index.html`. Se descarta el intento del `RUSTFLAGS`
  `-target-feature=-bulk-memory` (contraproducente).
- **Valkey no arrancaba con `cap_drop: ALL`**: `setpriv` del entrypoint necesita
  `SETUID`/`SETGID` (+ `CHOWN`/`FOWNER`/`DAC_OVERRIDE` para el volumen `/data`).
  Re-añadidos como `cap_add` en el servicio valkey del compose.

### Agregado
- `docker-compose.staging.yml`: server (build local, `read_only`, `cap_drop: ALL`,
  `no-new-privileges`, `tmpfs /tmp`, volumen `dmart-data`) + Valkey 8 (LRU 256MB),
  healthchecks, `restart: unless-stopped`, puerto configurable (`STAGING_PORT`) y
  fail-fast de `DMART_MASTER_KEY`. Sin container SurrealDB (DB embebida en archivo).
- `.env.staging.example` con `DMART_MASTER_KEY`/`DMART_ADMIN_PASSWORD`/Argon2id;
  `.env.staging` añadido a `.gitignore`.
- Spec-006 refrescada a la arquitectura real (DB embebida, `/obs/health`, frontend
  vía `ServeDir` sin nginx, env `DMART_*`); criterio R1/R4 del ROADMAP.

---

## [Unreleased] — SPEC-005: Prometheus `/metrics` + Grafana Dashboards

### Agregado
- **Extensión de métricas** (`/obs/metrics`, antes `/metrics`): todas las series
  REQUERIDAS del spec expuestas, incluido business KPIs:
  - HTTP/System: `http_requests_total{method,status}`, `http_request_duration_seconds`,
    `http_requests_errors_total{route}` (route agrupado, cardinalidad acotada),
    `uptime_seconds`, `process_cpu_seconds_total` + `process_resident_memory_bytes`
    (leídos de /proc), `db_connections_active`, `cache_connected`,
    `surreal_connection_pool`, `surreal_query_duration_seconds`.
  - Auth/Security: `auth_login_total`, `auth_refresh_total` (success/failure),
    `auth_failures_total{reason}`, `rbac_denials_total{permission}`.
  - Business: `patients_total{status=all|active}`, `patients_created_total`,
    `patients_deleted_total`, `measurements_total`, `measurements_created_total`,
    `scales_calculated_total{scale}`.
  - ML: `ml_predictions_total{model}`, `ml_accuracy_gauge{model}`,
    `ml_model_load_duration_seconds`.
  - Realtime/Interop: `sse_connections_active`, `hl7_messages_processed_total{source}`,
    `hl7_messages_errors_total{source}`.
  - Data-quality (SPEC-031, baseline a 0 hasta que se implemente):
    `ingest_gap_total`, `ingest_invalid_total`, `ingest_fault_devices`,
    `ingest_throttled_total`, `ingest_error_avg`.
- **Nuevo módulo `dmart-server/src/metrics.rs`**: registro descriptivo (SinLabels/
  LabelNames) de las ~27 métricas, helpers de instrumentación (auth, HL7, SSE,
  escalas, ML, HTTP errors) y `survey_db()` que recalcula en cada scrape
  `patients_total`, `measurements_total`, `surreal_connection_pool`,
  `surreal_query_duration_seconds` y gauges de proceso.
- **`/obs/metrics` handler** en `observability.rs`: comprueba DB viva (ping),
  actualiza gauges en vivo y renderiza las métricas; warm-up de HTTP counters en
  pruebas. `db_healthy()` corregido a `RETURN 1` (fix de `/health` 503).
- **Fix bug latente de dependencias**: `metrics` subido `0.21 → 0.22` para que el
  recorder instalado (`metrics-exporter-prometheus` 0.13.1 exige `metrics ^0.22`)
  y los macros de la app usen el MISMO crate. Antes había dos instancias del crate
  `metrics` y `/metrics` exponía cuerpo **vacío**.
- **6 dashboards Grafana** en `grafana/dashboards/` (+ `datasource.yaml`):
  `dmart-overview`, `clinical-kpis`, `ml-models`, `security`, `hl7-fhir`,
  `monitor-data-quality` (schema 39, JSON válidos, datasource Prometheus).
- **18 alerting rules** en `prometheus/rules/` (critical, auth-security,
  clinical-ml, ingest-hl7): DMartServerDown, DMartHighErrorRate (ratio 5xx >1%
  on writes), DMartHighLatencyP95 (>2s), DMartDatabaseDisconnected,
  DMartProcessMemoryHigh (>4GiB), LoginBruteForce (>30/min), RefreshTokenFailureBurst,
  LoginSuccessRateDrop, RBACDenialBurst (guard de 0 para accuracy sin baseline),
  ClinicalVolumeAnomaly, MLModelDrift, MLMetricsStale, ScalesEngineErrors,
  HL7IngestErrorRate, HL7IngestPipelineDown + 3 alertas ingest SPEC-031
  (SensorGap/SensorFault/Throttling).
- **`prometheus/prometheus.yml`**: scrape job `dmart-server` sobre `/obs/metrics`.
- **Config promtool validada**: `promtool check config` ✓, `check rules` ✓ (18 reglas),
  `promtool test rules` ✓ sobre `prometheus/rules/test.yml` (12 casos: positivos +
  negativos incl. guard de accuracy=0 y volúmenes estables sin falsa alarma).

### Corregido
- `db_healthy()` devolvía siempre `false` (`SELECT 1 AS health` no válido sin
  FROM) → `/health` devolvía 503 de forma persistente. Ahora usa `RETURN 1`.
- Dos instancias del crate `metrics` (0.21 en app vs 0.22 requerido por exporter)
  → `/metrics` vacío. Bump a `metrics = "0.22"`.

### Añadido en finalización (rollout)
- **Buckets finos de latencia** (`PrometheusBuilder::set_buckets`): le de
  0.1/0.5/1/2.5/5 ms (antes el tope fino era 5 ms) para p50/p95/p99 fiables en
  el rango clínico UCI (edge case #3). `test_metrics_histogram_buckets_fine_covered`
  lo verifica.
- **Feature flag `METRICS_EXTENDED`** (default ON): si `false`, `/obs/metrics`
  solo expone sistema/HTTP/infra y oculta KPIs clínicos/ML/HL7/ingest — switch
  de rollout de la spec (con tests unitarios del parseo).
- **Ocupación de camas**: nuevas series `camas_total` y `camas_ocupadas`
  (survey en cada scrape) + paneles "Ocupación (%)" y "Camas ocupadas" en
  `grafana/dashboards/clinical-kpis.json`.
- **Protección de cardinalidad** en `prometheus/prometheus.yml`: `label_limit`,
  `label_name_length_limit`, `label_value_length_limit`, `sample_limit` (edge
  case #2, defensa en profundidad sobre labels ya acotados).
- **Load test k6** `tests/load/metrics.js`: ramping a ~1000 req/s sobre
  `/obs/metrics` con threshold `p95 < 100ms` (objetivo del spec) + escenario en
  `tests/load/run_all.js`.
- Spec 005 actualizada: checklists de Testing Strategy/Done marcados y notas
  operativas (dependencia de staging = SPEC-006/012, alerts dry-run y routing =
  SPEC-008, restricción de red en `/obs/metrics` en prod).

### Tests
- `api_tests` 31 (3 nuevos: buckets finos, ocupación de camas e ingest en el
  E2E de métricas), `hl7_integration` 32, lib server 41 (incl. parse de flag).
  Todos verdes.
- Gates solo `-p dmart-server` (nunca workspace por WASM/fuzz).

---

## [Unreleased] — SPEC-004: Auth/Autz Hardening (JWT Refresh + RBAC Granular)

### Agregado
- **Refresh tokens rotativos (single-use)**: `POST /auth/login` emite par
  `access_token` (15min) + `refresh_token` (7d, cookie httpOnly `Secure`
  `SameSite=Strict`); `POST /auth/refresh` rota el refresh token en cada uso y
  revoca el anterior en DB (`refresh_token` con `token_hash` SHA-256, indexado
  UNIQUE).
- **Detección de reuso**: reutilizar un refresh token ya consumido revoca toda
  la familia de sesiones del usuario (posible robo → `revoke-all` de la familia).
- **Logout inmediato**: `POST /auth/logout` revoca access token (blacklist JWT
  en Valkey con TTL restante, fail-open si cache caída) + refresh token en DB;
  `POST /auth/revoke-all` revoca todas las sesiones del usuario.
- **RBAC granular**: middleware `require_permission` aplica la matriz
  `resource:action` (`rbac.rs`) al 100% de los endpoints autenticados vía
  tabla única `permission_for(method, path)`.
- Migración `003_refresh_tokens.surql` (tabla `refresh_token` SCHEMAFULL).

### Corregido
- `cargo audit` → **0 advisories**: actualización de `ammonia`, `crossbeam-*`,
  `quinn-proto`, `rustls-webpki` y `printpdf` (0.7 → 0.12, lopdf 0.44 parcheado
  del RUSTSEC-2026-0187). El generador de PDF de exportación se reescribió al
  nuevo API de ops de printpdf 0.12.

### Tests
- `api_tests` 29 (incl. refresh rotation, reuse detection, logout/revoke-all y
  RBAC granular), `hl7_integration` 32, lib server 39 (incl. nuevo
  `generate_pdf_produces_valid_document`). Todos verdes.

---

## [Unreleased] — SPEC-003: Tests de Integración HL7/MLLP

### Agregado
- `dmart-server/tests/hl7_integration.rs` — 32 tests de integración HL7 v2 + MLLP
  - Parser: ORU^R01 Mindray/Philips, LOINC/mnemónicos, PID-18 (UUID directo), rechazo graceful de malformados, rango de valores, secuencias de escape
  - MLLP: `build_ack` AA/AR, stream TCP real via `serve()` — error de parseo, payload no-UTF8, fallo de ingestión, mensaje >1 MiB descartado, cierre graceful del cliente
  - Ingest: creación de medición + severidad, paciente inexistente, resolución por UUID, ingesta concurrente (5 pacientes)
- Cobertura HL7 >90%: `parser.rs` 96%+, `mllp.rs` 94%+, `ingest.rs` 92% (vía `cargo llvm-cov`)
- Job CI `hl7-integration-test` en `.github/workflows/ci.yml`

### Corregido
- Clippy gate (`-D warnings`) en TODO el workspace, incl. pendientes de Fase 5.6:
  - prop-test tautológico `apache_score >= 0` eliminado, `manual_range_contains`, `module_inception` en `hl7/proptests.rs`
  - `needless_borrow`, `useless_vec`, `useless_format` en tests HL7
- `.github/workflows/ci.yml`: `--test integration` apuntaba a un target inexistente → `--test api_tests`

### Tests
- `hl7_integration` 32, `api_tests` 25, lib server 33, lib shared 31 — todos verdes
- Gates locales: `fmt --check` ✓, `clippy -D warnings` ✓

---

## [v0.5.0] - 2026-09-12 — Fase 5 Completa: Clínico y QA Avanzado

### Agregado
- **Fase 5.3**: Dashboard Ejecutivo con 6 KPIs (Egresados, Fallecidos, Mortalidad Real %, Mortalidad Predicha %, Delta, LOS Promedio)
  - `EjecutivoKpi` type + `EjecutivoKpiSection` component en `dmart-app/src/pages/dashboard.rs`
  - Backend `aggregate_patient_stats` ya computaba KPIs
- **Fase 5.4**: FHIR R4 DiagnosticReport + QR Codes
  - `GET /fhir/Patient/{id}/DiagnosticReport` — Bundle de reportes
  - `GET /fhir/Patient/{id}/DiagnosticReport/QR` — SVG + PNG Base64
  - Deps: `qrcode` 0.14, `image` 0.24
  - 3 tests QR generation
- **Fase 5.5**: E2E Playwright Infrastructure (15 tests)
  - `tests/e2e/login.spec.ts` (3 tests)
  - `tests/e2e/patients.spec.ts` (4 tests)
  - `tests/e2e/measurements.spec.ts` (4 tests)
  - `tests/e2e/admin.spec.ts` (4 tests)
  - Config: `playwright.config.ts` con webServer Python http.server
  - Node.js LTS via fnm + `npx playwright install chromium`
- **Fase 5.6**: Property-Based Testing (proptest) — 66 tests
  - Escalas: APACHE II, SAPS III, NEWS2, SOFA, GCS bounds + mortalidad + monotonicidad + breakdown
  - HL7 Parser: parse válido, malformado robusto, classify_vital LOINC/mnemonics, datetime conversion, detect_vendor
  - `dmart-shared/src/scales.rs` + `dmart-server/src/hl7/proptests.rs`
- **Fase 5.7**: Fuzzing API (cargo-fuzz) — 3 targets
  - `fuzz_api_json`: endpoints JSON (pacientes, mediciones, escalas)
  - `fuzz_hl7_parser`: parser HL7 v2 robustez
  - `fuzz_scales`: cálculo escalas clínicas desde bytes
  - `dmart-server/fuzz/` workspace member
- **Fase 5.8**: Load Testing k6 — 4 escenarios
  - `tests/load/auth.js` — autenticación + endpoints protegidos (100 VUs)
  - `tests/load/scales.js` — APACHE II, SOFA, NEWS2, SAPS III, GCS (100 VUs)
  - `tests/load/fhir.js` — Patient search/read, DiagnosticReport, QR (100 VUs)
  - `tests/load/hl7.js` — health check bajo carga simulada (50 VUs)
  - `tests/load/run_all.js` — runner maestro secuencial
  - k6 v0.54.0 instalado
- **Fase 5.9**: ML Piloto — Predicción Mortalidad ApacheII→Riesgo
  - `dmart-shared/src/ml.rs`: `MortalityModel` (DecisionTree linfa 0.7)
  - 14 features desde `ApacheIIData`
  - Datos sintéticos 1000 muestras con reglas clínicas APACHE II
  - Accuracy ~85-90% en holdout 200
  - Deps: `linfa`, `linfa-trees`, `linfa-linear`, `linfa-datasets`, `linfa-preprocessing`, `ndarray`, `rand`
  - 3 tests ML
- **Fase 5.10**: GCS Animado + ScoreBar Animado
  - `ScoreBar` con gradiente cónico rotatorio + brillo radial
  - 3 niveles pulse: critical (1s rojo >70%), warning (2s naranja 40-70%), normal (3s verde <40%)
  - Tailwind keyframes: `scorePulse`, `score-pulse-normal/critical/warning`

### Corregido
- Prop-test tuple size fix en `scales.rs` (14→11 campos orden correcto)
- Clippy fixes: `manual_clamp`, `useless_conversion`, `collapsible_if`, `needless_borrow`
- WASM build: `wasm_opt = false` en Trunk.toml (lightningcss/wasm-opt conflict)
- Fuzz targets: collapsible if, unused imports

### Seguridad
- 0 advisories críticos en `cargo audit`
- Fuzzing + Prop-testing = defensa en profundidad

### Tests
- **Total: 122+ tests** (31 lib + 66 prop + 25 e2e infrastructure)
- Gates: `clippy -D warnings` ✓, `test --lib` ✓, `build --release` ✓

---

## [v0.4.0] - 2026-09-11 — Fase 4: Frontend y UX

### Agregado
- Loading states + error handling (Suspense/fallback) — sin pantallas vacías
- PWA Offline: service worker cache-first para WASM + manifest
- Accesibilidad WCAG 2.1 AA (ARIA, contraste, teclado) — auditoría axe sin errores
- Virtual scrolling en listas 1000+ pacientes
- Dark mode respetando `prefers-color-scheme` + persistencia
- Búsqueda reactiva debounce 300ms
- Streaming de scores tiempo real vía SSE (axum `response::sse` + `EventSource`)
  - Auth por `?token=`, keep-alive 15s
  - Reemplaza WebSocket (sin openssl, sin deps extra)

### Corregido
- `chrono` año dinámico (fin 2026 hardcodeado)
- Diagnósticos persistidos en SurrealDB (antes HashMap memoria)

---

## [v0.3.0] - 2026-09-10 — Fase 3: DevOps y Observabilidad

### Agregado
- Métricas Prometheus (requests, latencia p50/p95/p99, errores) — `/metrics`
- Logging estructurado JSON + OpenTelemetry tracing (opcional)
- Health check enriquecido (DB ping, cache, uptime, versión) — `/health`, `/live`, `/ready`
- Graceful shutdown con drenado + timeout configurable
- Reconnect SurrealKV con backoff exponencial
- Docker Compose healthchecks

### Corregido
- `reqwest` → `rustls` (eliminado OpenSSL cadena)
- MSRV 1.98 + edition 2024

---

## [v0.2.0] - 2026-09-09 — Fase 2: Arquitectura y Datos

### Agregado
- Sistema migraciones SurrealQL versionado
- Transacciones atómicas (paciente + cama + equipos)
- Índices `DEFINE INDEX` (created_at, username, cama_id, estado)
- Stats con agregaciones `GROUP BY` (no cargar 50k filas)
- `WHERE` queries server-side (auditoría, auth)

---

## [v0.1.0] - 2026-09-08 — Fase 1: Seguridad Crítica

### Agregado
- `DMART_MASTER_KEY` obligatorio por env (fail al arranque si default)
- `/auth/register` protegido (solo admin autenticado con rol)
- RBAC conectado a todos los handlers (`require_role`)
- `password_hash` oculto en respuestas (`UserInfo`)
- Admin demo eliminado de `/auth/me`
- Throttle login fix: cuenta fallos reales, clave por IP real
- Rate limit IP real (proxy confiable configurable, no spoofeable `x-forwarded-for`)
- Argon2id configurable `m_cost` 19MB por defecto
- Revocación JWT: blacklist en logout (Valkey) + expiración corta
- HSTS + redirección HTTP→HTTPS
- Credenciales default cambiadas + mensajes setup claros
- Tests seguridad automatizados (auth/RBAC/throttle/rate-limit/E2E HTTP)

### Seguridad
- Arranque abortado si `DMART_MASTER_KEY` es default
- 100% rutas con permiso mapeado en `rbac::permission_for`

---

## [v0.0.1] - 2026-09-07 — Fase 0: Limpieza y Cimientos

### Agregado
- Migración MSRV 1.98 + edition 2024
- Toolchain pin `rust-toolchain.toml`
- Docker `rust:1.98-slim-bookworm`
- CI GitHub Actions en rama `main`
- Rama `master` → `main`
- Clippy 0 warnings, `fmt --check` limpio
- 66 tests shared + 3 API verdes
- ROADMAP único, docs obsoletas eliminadas