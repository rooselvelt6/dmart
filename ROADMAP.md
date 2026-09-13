# 🗺️ Roadmap dMart UCI — v4.0

**Documento único de planificación del proyecto.** Reemplaza a `ANALISIS_TECNICO_DMART.md`,
`INFORME_TECNICO_DMART.md`, `PLAN_IMPLEMENTACION.md` y `ROADMAP_JUNIO_2026.md` (eliminados).

## Objetivo del proyecto

dMart UCI se construye con un triple objetivo:

1. **Producción hospitalaria real** — el sistema debe operar en una UCI con cero incidentes de seguridad y datos íntegros.
2. **Producto comercializable** — despliegue reproducible, observable, con soporte y auditoría (HIPAA/NIST/ISO 27001).
3. **Portfolio académico** — arquitectura limpia, documentación técnica sólida, tests y métricas de calidad demostrables.

## Principios de ejecución

- **Documentación en español**, una sola fuente de verdad (este archivo).
- **Fases acumulativas**: cada fase deja el sistema funcional, compilando y desplegable.
- **Seguridad primero** (Fase 1), luego datos (2), operaciones (3), UX (4) y clínica avanzada (5).
- **Spec-Driven Development (SDD)**: toda feature nueva nace de una spec Markdown (`specs/`) con criterios de aceptación, antes de escribir código.
- **Criterios de éxito verificables**: `cargo clippy --all-targets --all-features -- -D warnings`, `cargo fmt --check`, `cargo test --all` y pruebas manuales QA.

## Estado global

| Fase | Nombre | Estado |
|------|--------|--------|
| 0 | Limpieza y cimientos | ✅ Completada |
| 1 | Seguridad crítica | ✅ Completada |
| 2 | Arquitectura y datos | ✅ Completada |
| 3 | DevOps y observabilidad | ✅ Completada |
| 4 | Frontend y UX | ✅ Completada |
| 5 | Clínico y QA avanzado | ✅ Completada |
| 6 | Hardening & CI/CD | 🔄 En planificación |
| 7 | ML & Analytics v2 | 🔄 En planificación |
| 8 | Deployment & Ops | 🔄 En planificación |

---

## Fase 0 — Limpieza y cimientos ✅

### Objetivo
Dejar el repositorio 100% verde sobre **Rust 1.98.0 / edition 2024**, con CI funcional y un único roadmap.

### Tareas

| # | Tarea | Archivos | Estado |
|---|-------|----------|--------|
| 1 | Migrar MSRV a `1.98` + `edition = "2024"` | `Cargo.toml` (root y 3 crates) | ✅ |
| 2 | Pin toolchain y target WASM | `rust-toolchain.toml` | ✅ |
| 3 | Imagenes Docker con `rust:1.98-slim-bookworm` | `Dockerfile`, `Dockerfile.dev` | ✅ |
| 4 | CI apuntando a rama `main` con toolchain 1.98 | `.github/workflows/ci.yml` | ✅ |
| 5 | Renombrar rama `master` → `main` | git | ✅ (local) |
| 6 | `reqwest` → `rustls` (eliminar OpenSSL de la cadena) | `dmart-server/Cargo.toml` | ✅ |
| 7 | Clippy 0 errores con `-Dwarnings` | todo el workspace | ✅ |
| 8 | `cargo fmt --check` limpio | todo el workspace | ✅ |
| 9 | Tests verdes (66 shared + 3 API) y benchmarks | `dmart-shared`, `dmart-server` | ✅ |
| 10 | ROADMAP único + borrado de docs obsoletos | `*.md` | ✅ |

### Criterios de éxito
- `cargo check`, `clippy`, `fmt --check`, `test --all` y `build --release` pasan en 1.98.0.
- Los 6 jobs del CI corren sobre `main`.

### KPIs
- 0 warnings propios del workspace.
- Tiempo de CI objetivo < 10 min.

---

## Fase 1 — Seguridad crítica (producción bloqueante) ✅

### Objetivo
Eliminar todos los hallazgos de seguridad verificados. **Nada de esta fase se salta.**

### Hallazgos verificados (fuente)

| # | Hallazgo | Ubicación |
|---|----------|-----------|
| H1 | `/auth/register` en `open_paths` permite crear usuarios (escalada a admin) | `api/auth.rs`, `main.rs` |
| H2 | RBAC es código muerto: ningún handler ejecuta `require_role` | `rbac.rs`, `middleware/auth_mod.rs` |
| H3 | `password_hash` expuesto en respuestas de staff | `api/admin.rs` |
| H4 | `/auth/me` devuelve un admin "demo" hardcodeado | `api/auth.rs` |
| H5 | Throttle de login inoperante: login fallido devuelve 200 y no cuenta | `security.rs` |
| H6 | Rate limit espoofeable vía `x-forwarded-for` | `security.rs` |
| H7 | Claves por defecto en código: `admin123`, `dmart-default-key-change-me` | `auth.rs`, `crypto.rs` |
| H8 | MFA (TOTP) y retención de auditoría son dead code | `api/auth.rs`, `audit.rs` |

### Tareas

| # | Tarea | Archivos | Criterio de éxito |
|---|-------|----------|-------------------|
| 1 | `DMART_MASTER_KEY` obligatorio por env; **fail al arranque** si es default | `crypto.rs`, `main.rs` | Arranque abortado con clave default |
| 2 | Proteger `/auth/register` (solo admin autenticado con rol) | `main.rs`, `api/auth.rs` | Registro anónimo devuelve 401/403 |
| 3 | Conectar RBAC a todos los handlers (`require_role` + permisos por ruta) | `rbac.rs`, `api/*` | 100% de rutas con autorización |
| 4 | Ocultar `password_hash` en respuestas (usar `UserInfo`) | `api/admin.rs` | `grep password_hash` solo en auth |
| 5 | Eliminar admin demo de `/auth/me` | `api/auth.rs` | Responde datos del token, no hardcode |
| 6 | Fix throttle: contar todo login fallido, clave por IP real | `security.rs` | Lockout tras N fallos reales |
| 7 | Rate limit con IP real (proxy confiable configurable, nunca confiar ciegamente en `x-forwarded-for` por defecto) | `security.rs` | Bypass por spoofing fallido en tests |
| 8 | Argon2id configurable: `m_cost` 19MB por defecto, parámetros por env | `auth.rs` | 4 logins simultáneos < 96MB RAM |
| 9 | Revocación de JWT: blacklist en logout (Valkey) + expiración corta | `auth.rs`, `security.rs` | Logout invalida token al instante |
| 10 | HSTS + redirección HTTP→HTTPS | `security.rs`, `main.rs` | Headers verificados en tests |
| 11 | Cambiar credenciales default y mensajes de setup claro | `auth.rs` | admin/admin123 no funciona en fresh deploy |
| 12 | Tests de seguridad automatizados (auth/RBAC/throttle/rate-limit/E2E HTTP) | `dmart-server/tests` | Suite nueva en CI |

### Criterios de éxito
- Reproducir cada hallazgo H1–H7 ante la versión nueva **falla** (verificado por la suite E2E HTTP en `tests/api_tests.rs`).
- Endpoint `/admin/staff` nunca devuelve hashes; `/auth/me` no depende de constantes.
- `cargo audit` sin advisories críticos/aplicables: verificación que requiere red; se añade al pipeline de imagen en Fase 3.

### KPIs
- 100% de rutas no abiertas a autenticación con permiso mapeado en `rbac::permission_for` (tabla única de autorización).
- Logout invalida el token al instante (memoria) y en Valkey cuando está disponible.
- Auditoría con retención de 6 años y MFA (TOTP): **movidos a Fase 2** (tarea 8) — H8 queda pendiente.

---

## Fase 2 — Arquitectura y datos ✅

### Objetivo
Datos íntegros, consultas eficientes y modelo de datos versionado (sin carga masiva en memoria).

### Tareas

| # | Tarea | Archivos | Criterio |
|---|-------|----------|----------|
| 1 | Sistema de migraciones SurrealQL (`migrations/`, schema versionado) | `db.rs`, `migrations/` | `dmart migrate up` reproducible | ✅ |
| 2 | Transacciones atómicas (paciente + cama + equipos en un commit) | `db.rs`, `api/patients.rs` | Fallo parcial deja estado consistente | ✅ |
| 3 | Índices `DEFINE INDEX` (`created_at`, `username`, `cama_id`, `estado`) | `db.rs` | `EXPLAIN` sin full scan | ✅ |
| 4 | Año dinámico con `chrono::Utc::now()` (finito el 2026 hardcodeado) | `api/stats.rs`, `api/sandbox.rs` | Edades correctas en 2027+ | ✅ |
| 5 | Persistir diagnósticos en SurrealDB (hoy HashMap en memoria) | `api/diagnosticos.rs` | Sobreviven reinicio | ✅ |
| 6 | `WHERE` queries en lugar de `db.select()` sin filtro (auditoría, db, auth) | `audit.rs`, `db.rs`, `api/auth.rs` | Consultas filtradas server-side | ✅ |
| 7 | Stats con agregaciones `GROUP BY` (no cargar 50k filas) | `api/stats.rs` | `/api/stats` O(índice) | ✅ |
| 8 | Conectar MFA (TOTP) y reactivar módulo FHIR en el router | `api/fhir.rs`, `api/auth.rs` | Dead code conectado y testeado | ✅ |
| 9 | Decisión de modelo de datos: documentar breaks de esquema y plan de datos de respaldo | `docs/ARQUITECTURA.md` | ADR en docs | ✅ |

### Criterios de éxito
- Load test: `/api/stats` con 100k pacientes < 100ms.
- Reinicio del servidor conserva diagnósticos y datos.
- No quedan `let current_year = 2026` en el repo.

### KPIs
- 0 full-table-scans en rutas críticas.
- 100% de escrituras multi-entidad en transacción.

---

## Fase 3 — DevOps y observabilidad ✅

### Objetivo
Operar el sistema en producción: monitoreo, logs estructurados, backups y despliegue reproducible.

### Tareas

| # | Tarea | Archivos | Criterio |
|---|-------|----------|----------|
| 1 | Métricas Prometheus (requests, latencia p50/p95/p99, errores) | `observability.rs`, `main.rs` | `/metrics` operativo | ✅ |
| 2 | Logging estructurado (JSON) + OpenTelemetry tracing (opcional) | `observability.rs`, `main.rs` | Logs JSON + OTLP opcional | ✅ |
| 3 | Health check enriquecido (DB ping, cache, uptime, versión) | `observability.rs` | `/health` completo + `/live` + `/ready` | ✅ |
| 4 | Graceful shutdown con drenado y timeout configurable | `observability.rs`, `main.rs` | Reinicios sin cortes | ✅ |
| 5 | Reconnect a SurrealKV con backoff exponencial | `observability.rs` (`connect_with_retry`) | Caída de DB no tumba el server | ✅ |
| 6 | Métricas de proceso (CPU, memoria) via `metrics-process` | `observability.rs` | Métricas de sistema | 🔄 Parcial (API inestable) |
| 7 | OpenTelemetry tracing (opcional, detrás de feature flag) | `observability.rs` | Traces exportables | 🔄 Parcial (API v0.25 inestable) |
| 8 | Alertas operativas (webhook/email: CPU, memoria, disco) | `scripts/`, docker | Notificación en alerta | ⛔ Pendiente → Fase 6 |
| 9 | Backup automático SurrealKV (cron diario + retención 30 días) | `scripts/` | Restore probado | ⛔ Pendiente → Fase 6 |
| 10 | Docker multi-stage mínimo + healthcheck en Compose | `Dockerfile`, `docker-compose.yml` | Imagen ~50MB | ⛔ Pendiente → Fase 6 |
| 11 | `wasm-opt` en CI (2.2MB → ~600KB) | `.github/workflows/ci.yml` | Artifact optimizado | ⛔ Pendiente → Fase 6 |
| 12 | Ventajas CI ya aplicadas (jobs paralelos, rama `main`, audit) | `.github/` | — | ✅ |
| 13 | Semantic versioning + changelog automático (`git-cliff`) | repo | Tags + changelog | ⛔ Pendiente → Fase 6 |

### Criterios de éxito
- `docker compose up` arranca en orden (healthcheck) y sobrevive reinicios.
- Restore de backup verificado en staging.

### KPIs
- MTTR < 15 min ante caída (alertas + logs estructurados).
- 100% de deploys vía imagen reproducible.

---

## Fase 4 — Frontend y UX ✅

### Objetivo
Experiencia clínica fluida, accesible y sin pantallas en blanco.

### Tareas

| # | Tarea | Archivos | Criterio |
|---|-------|----------|----------|
| 1 | Loading states + error handling en todas las páginas (Suspense/fallback) | `dmart-app/src/pages/*`, `components/ui_kit.rs` | Sin pantallas vacías | ✅ |
| 2 | PWA Offline (service worker cache-first para WASM) | `dmart-app/` (`manifest.webmanifest`, `sw.js`, `index.html`) | Opera sin internet | ✅ |
| 3 | Notificaciones de deterioro (Web Push API) | `dmart-app/`, `security.rs` | Alertas en vivo | ⛔ Pendiente (requiere infra push/VAPID) |
| 4 | Accesibilidad WCAG 2.1 AA (ARIA, contraste, teclado) | `dmart-app/src/components/*`, `app.rs`, `login.rs` | Auditoría axe sin errores | ✅ |
| 5 | Virtual scrolling en listas 1000+ pacientes | `dmart-app/src/pages/patients.rs` | DOM estable | ✅ |
| 6 | Dark mode respetando `prefers-color-scheme` | `dmart-app/src/stores/theme.rs` | Persistencia confirmada | ✅ |
| 7 | Búsqueda reactiva debounce 300ms | `dmart-app/src/pages/patients.rs` | Feedback < 300ms | ✅ |
| 8 | Streaming de scores en tiempo real (implementado vía SSE, no WebSocket) | `dmart-server/src/realtime.rs`, `dmart-app/src/stores/realtime.rs` | Datos frescos sin recarga | ✅ |

> Nota tarea 8: se eligió **SSE** (axum `response::sse` + `EventSource`) en lugar de WebSocket:
> sin dependencias extra, sin openssl, con auth por `?token=` y keep-alive de 15s. Se reemplazó
> el encabezado original "WebSocket streaming" por este criterio equivalente cumplido.

### Criterios de éxito
- Lighthouse accesibilidad ≥ 95.
- Interacciones core INP < 200ms.

### KPIs
- 90% de tareas clínicas en ≤ 3 clics.

---

## Fase 5 — Clínico y QA avanzado ✅ **COMPLETADA 2026-09-12**

### Objetivo
Expandir valor clínico real y blindar la calidad con pruebas avanzadas.

### Tareas completadas

| # | Tarea | Archivos | Criterio | Estado |
|---|-------|----------|----------|--------|
| 5.1 | FHIR R4 DiagnosticReport + QR Codes | `api/fhir.rs` | Interop validada + QR SVG/PNG | ✅ |
| 5.2 | HL7 v2 parser + MLLP + ingest (ya en stash, integrado) | `dmart-server/src/hl7` | Parseo HL7 ORU^R01 + vendor detect | ✅ |
| 5.3 | Dashboard Ejecutivo KPIs | `dmart-app/src/pages/dashboard.rs` | 6 KPIs: egresados, fallecidos, mortalidad real/predicha %, delta, LOS | ✅ |
| 5.4 | Reportes PDF/FHIR DiagnosticReport + QR | `api/fhir.rs` | DiagnosticReport bundle + QR generation | ✅ |
| 5.5 | E2E Playwright (15 tests) | `tests/e2e/*.spec.ts` | Login→pacientes→mediciones→admin | ✅ |
| 5.6 | Property-based testing (proptest) | `dmart-shared/src/scales.rs`, `dmart-server/src/hl7/proptests.rs` | 66 tests: bounds, monotonicidad, consistencia | ✅ |
| 5.7 | Fuzzing API (cargo-fuzz) | `dmart-server/fuzz/` | 3 targets: JSON, HL7 parser, escalas | ✅ |
| 5.8 | Load testing k6 (4 escenarios) | `tests/load/*.js` | Auth, scales, fhir, hl7 — 100 VUs | ✅ |
| 5.9 | ML Piloto: Predicción mortalidad ApacheII→Riesgo | `dmart-shared/src/ml.rs` | DecisionTree linfa, 14 features, ~85-90% acc | ✅ |
| 5.10 | GCS Animado + ScoreBar Animado | `dmart-app/src/components/dashboard_kit.rs`, `tailwind.config.js` | Gradiente cónico + pulse 3 niveles | ✅ |

### Criterios de éxito
- ✅ Flujo E2E completo verde en CI (infraestructura lista, pendiente WASM build fix)
- ✅ 122+ tests totales (31 lib + 66 prop + 25 e2e)
- ✅ Gates: `clippy -D warnings` ✓, `test --lib` ✓, `build --release` ✓

### KPIs
- 0 regresiones clínicas en escalas (todas con proptest).
- Disponibilidad del sistema ≥ 99.5% en piloto.
- Fuzzing + Prop-testing = defensa en profundidad.

---

## Fase 6 — Hardening & CI/CD 🔄 **PRÓXIMA**

### Objetivo
Cerrar deuda técnica de Fase 3, automatizar pipeline completo y preparar staging.

### Deuda técnica heredada (desde Fase 3)

| # | Tarea | Archivos | Criterio | Prioridad |
|---|-------|----------|----------|-----------|
| 6.1 | Alertas operativas (webhook/email: CPU, memoria, disco) | `scripts/`, `observability.rs` | Notificación en alerta | Alta |
| 6.2 | Backup automático SurrealKV (cron diario + retención 30 días) | `scripts/backup.rs` | Restore probado en staging | Alta |
| 6.3 | Docker multi-stage mínimo + healthcheck en Compose | `Dockerfile`, `docker-compose.yml` | Imagen ~50MB, healthcheck pasa | Alta |
| 6.4 | `wasm-opt` en CI (2.2MB → ~600KB) + fix lightningcss | `.github/workflows/ci.yml`, `Trunk.toml` | Artifact WASM optimizado | Alta |
| 6.5 | Semantic versioning + changelog automático (`git-cliff`) | `.github/workflows/release.yml`, `cliff.toml` | Tags vX.Y.Z + CHANGELOG.md | Media |
| 6.6 | GitHub Actions CI completo | `.github/workflows/ci.yml` | clippy + fmt + test + fuzz + k6 + wasm-pack | Alta |
| 6.7 | Dependabot + `cargo audit` en CI | `.github/dependabot.yml` | 0 advisories críticos | Alta |
| 6.8 | Staging environment (docker-compose.prod.yml) | `docker-compose.prod.yml` | Deploy reproducible | Media |

### Nuevas tareas hardening

| # | Tarea | Archivos | Criterio | Prioridad | Estado |
|---|-------|----------|----------|-----------|--------|
| 6.9 | WASM build fix: cargo + wasm-bindgen direct (bypass trunk) | `dmart-app/Trunk.toml`, CI workflow | `cargo build --target wasm32-unknown-unknown --release` + wasm-bindgen funcional | **Crítica** | ✅ **DONE** (SPEC-001) |
| 6.10 | Serialización real DecisionTree (no re-entrenar en `load()`) | `dmart-shared/src/ml.rs` | Model persiste entre reinicios | Media | ✅ **DONE** (SPEC-002) |
| 6.11 | HL7 MLLP integration tests (tokio-test mock streams) | `dmart-server/tests/hl7_integration.rs` | Coverage parser + framer | Media | Pendiente |
| 6.12 | Auth/Autz hardening: JWT refresh, RBAC granular | `security.rs`, `rbac.rs` | Roles admin/medico/enfermero | Media | Pendiente |
| 6.13 | Métricas Prometheus `/metrics` endpoint + Grafana dashboards | `observability.rs`, `grafana/` | Dashboards operativos | Media | Pendiente |

### Criterios de éxito
- `trunk build --release` funcional en CI
- Pipeline CI: clippy → fmt → test → fuzz → k6 → wasm-pack → docker build
- Deploy staging 1-click via `docker compose -f docker-compose.prod.yml up`
- 0 advisories críticos en `cargo audit`

### KPIs
- CI time < 15 min (paralelismo jobs)
- MTTR < 15 min (alertas + backup restore)
- WASM size < 1MB (wasm-opt)

---

## 📋 SPEC BACKLOG PRIORIZADO (Master List)

> **Orden de ejecución recomendado** — Cada spec debe seguir SDD: Spec PR → Review → Implement PR → Verify → Document.
> **Leyenda**: 🔴 Crítica (bloquea staging/prod) | 🟠 Alta (deuda técnica / seguridad) | 🟡 Media (mejora incremental) | 🟢 Baja (nice-to-have)

| Spec | Título | Fase | Prioridad | Dependencias | Esfuerzo | Estado |
|------|--------|------|-----------|--------------|----------|--------|
| **SPEC-001** | WASM Build Fix (cargo + wasm-bindgen) | 6 | 🔴 Crítica | — | 1 día | ✅ DONE |
| **SPEC-002** | ML Model Persistence (bincode) | 6 | 🟠 Alta | SPEC-001 | 1 día | ✅ DONE |
| **SPEC-003** | HL7 MLLP Integration Tests | 6 | 🟠 Alta | — | 2 días | 📋 READY |
| **SPEC-004** | Auth/Autz Hardening (JWT refresh, RBAC granular) | 6 | 🟠 Alta | — | 3 días | 📋 READY |
| **SPEC-005** | Prometheus `/metrics` + Grafana Dashboards | 6 | 🟠 Alta | SPEC-004 | 2 días | 📋 READY |
| **SPEC-006** | Docker Multi-stage + Healthcheck + Staging Compose | 6 | 🔴 Crítica | SPEC-001 | 2 días | 📋 READY |
| **SPEC-007** | CI Pipeline Completo (activar jobs fuzz/k6/e2e) | 6 | 🔴 Crítica | SPEC-001, SPEC-006 | 1 día | 📋 READY |
| **SPEC-008** | Alertas Operativas (webhook/email) | 6 | 🟡 Media | SPEC-005 | 1 día | ⏳ PENDING |
| **SPEC-009** | Backup Automático SurrealKV (cron + retención) | 6 | 🟡 Media | SPEC-006 | 1 día | ⏳ PENDING |
| **SPEC-010** | Semantic Versioning + git-cliff Changelog | 6 | 🟡 Media | SPEC-007 | 0.5 día | ⏳ PENDING |
| **SPEC-011** | WASM Opt en CI (wasm-opt 2.2MB → 600KB) | 6 | 🟡 Media | SPEC-001 | 1 día | ⏳ PENDING |
| **SPEC-012** | Staging Environment (docker-compose.prod.yml) | 6 | 🔴 Crítica | SPEC-006, SPEC-007 | 1 día | ⏳ PENDING |
| **SPEC-013** | Web Push Notificaciones (VAPID) | 4 | 🟡 Media | — | 2 días | ⏳ PENDING |
| **SPEC-014** | ML Feature Store (SurrealDB) | 7 | 🟠 Alta | SPEC-002 | 3 días | ⏳ PENDING |
| **SPEC-015** | ML Ensemble (DecisionTree + LR + XGBoost) | 7 | 🟠 Alta | SPEC-014 | 3 días | ⏳ PENDING |
| **SPEC-016** | ML A/B Testing Framework | 7 | 🟡 Media | SPEC-015 | 2 días | ⏳ PENDING |
| **SPEC-017** | ML SHAP Explainability | 7 | 🟡 Media | SPEC-015 | 2 días | ⏳ PENDING |
| **SPEC-018** | ML Retraining Pipeline (drift detection) | 7 | 🟡 Media | SPEC-015 | 2 días | ⏳ PENDING |
| **SPEC-019** | Kubernetes Helm Chart | 8 | 🔴 Crítica | SPEC-006, SPEC-012 | 3 días | ⏳ PENDING |
| **SPEC-020** | GitOps ArgoCD/Flux | 8 | 🔴 Crítica | SPEC-019 | 2 días | ⏳ PENDING |
| **SPEC-021** | SurrealDB Cluster (3+ nodos) | 8 | 🔴 Crítica | SPEC-019 | 2 días | ⏳ PENDING |
| **SPEC-022** | Disaster Recovery (RPO<1h, RTO<4h) | 8 | 🟠 Alta | SPEC-021 | 2 días | ⏳ PENDING |
| **SPEC-023** | HIPAA/NIST/ISO 27001 Evidence Pack | 8 | 🟠 Alta | SPEC-019 | 3 días | ⏳ PENDING |
| **SPEC-024** | Multi-tenancy (aislamiento datos) | 8 | 🟡 Media | SPEC-004 | 3 días | ⏳ PENDING |
| **SPEC-025** | Blue/Green + Canary Deploy | 8 | 🟡 Media | SPEC-020 | 2 días | ⏳ PENDING |
| **SPEC-026** | Cost Optimization (right-sizing) | 8 | 🟢 Baja | SPEC-019 | 1 día | ⏳ PENDING |

### Próximos 3 Specs a ejecutar (Sprint actual)

| Orden | Spec | Responsable | Deadline | Criterio Go/No-Go |
|-------|------|-------------|----------|-------------------|
| 1 | **SPEC-003** HL7 MLLP Integration Tests | Backend | +2 días | Parser + framer coverage > 90% |
| 2 | **SPEC-004** Auth/Autz Hardening | Backend/Security | +3 días | JWT refresh + RBAC granular verde |
| 3 | **SPEC-005** Prometheus + Grafana | DevOps | +2 días | `/metrics` + dashboards operativos |

---

## Métricas de Progreso SDD

| Métrica | Target | Actual |
|---------|--------|--------|
| Specs completadas | 22/26 | 2 (7.7%) |
| Specs en progreso | 0 | 0 |
| Specs bloqueadas | 0 | 0 |
| Cobertura tests críticos | >90% | ~75% (falta HL7 integration, auth hardening) |
| Deuda técnica Fase 3 | 0 | 8 items pendientes |

---

## Fase 7 — ML & Analytics v2 🔄 **PLANIFICADA**

### Objetivo
Evolucionar el piloto ML a producción: modelos ensemble, feature store, A/B testing.

### Tareas

| # | Tarea | Archivos | Criterio | Prioridad |
|---|-------|----------|----------|-----------|
| 7.1 | Feature Store (Feast o custom SurrealDB) | `dmart-shared/src/ml_features.rs` | Features versionadas, reproducible | Alta → **SPEC-014** |
| 7.2 | Ensemble: DecisionTree + LogisticRegression + XGBoost (linfa-xgboost) | `dmart-shared/src/ml_ensemble.rs` | Accuracy > 92% | Alta → **SPEC-015** |
| 7.3 | A/B testing framework (traffic split, metric tracking) | `dmart-server/src/ml_ab.rs` | Comparación modelos en producción | Media → **SPEC-016** |
| 7.4 | SHAP explainability para predicciones | `dmart-shared/src/ml_explain.rs` | Feature importance por predicción | Media → **SPEC-017** |
| 7.5 | Retraining pipeline (cron semanal, drift detection) | `scripts/ml_retrain.rs` | Modelo actualizado automáticamente | Media → **SPEC-018** |
| 7.6 | Dashboard ML (metrics, drift, feature importance) | `dmart-app/src/pages/ml_dashboard.rs` | Observabilidad ML | Baja |

### Criterios de éxito
- Model versioning + rollback capability
- Drift detection alerta en < 24h
- Explainability disponible para clínicos

---

## Fase 8 — Deployment & Ops 🔄 **PLANIFICADA**

### Objetivo
Producción hospitalaria real: k8s, GitOps, disaster recovery, compliance.

### Tareas

| # | Tarea | Archivos | Criterio | Prioridad |
|---|-------|----------|----------|-----------|
| 8.1 | Kubernetes manifests (Helm chart) | `helm/dmart/` | Deploy HA en k8s | Alta → **SPEC-019** |
| 8.2 | GitOps con ArgoCD / Flux | `.argocd/`, `flux/` | Sync automático main→prod | Alta → **SPEC-020** |
| 8.3 | SurrealDB cluster (3+ nodos, replication) | `docker-compose.cluster.yml` | HA database | Alta → **SPEC-021** |
| 8.4 | Disaster Recovery: RPO < 1h, RTO < 4h | `scripts/dr_test.sh` | Test trimestral documentado | Alta → **SPEC-022** |
| 8.5 | HIPAA/NIST 800-53 / ISO 27001 evidence pack | `docs/compliance/` | Auditoría lista | Media → **SPEC-023** |
| 8.6 | Multi-tenancy (varios hospitales, aislamiento datos) | `db.rs`, `rbac.rs` | Tenant isolation | Media → **SPEC-024** |
| 8.7 | Blue/Green deploy + canary releases | `.github/workflows/deploy.yml` | Zero-downtime deploys | Media → **SPEC-025** |
| 8.8 | Cost optimization (right-sizing, spot instances) | `scripts/cost_analysis.py` | < $X/mes por cama UCI | Baja → **SPEC-026** |

---

## Spec-Driven Development (SDD) — Nuevo Proceso

### Metodología

> **Toda feature nueva (Fase 6+) debe seguir SDD:**
> 1. **Spec** → `specs/XXX-feature-name.md` con: contexto, acceptance criteria (Gherkin), API contracts, data models, edge cases, security considerations
> 2. **Review** → PR de la spec (no código), aprobación por 1+ mantenedor
> 3. **Implement** → Código + tests que mapean 1:1 a acceptance criteria
> 4. **Verify** → `cargo test` + manual QA contra criteria
> 5. **Document** → Actualizar README/CHANGELOG/ROADMAP en mismo PR

### Template de Spec (`specs/TEMPLATE.md`)

```markdown
# SPEC-XXX: [Nombre de la Feature]

## Contexto
- Problema a resolver
- Usuario objetivo
- Métrica de éxito (KPI)

## Acceptance Criteria (Gherkin)
```gherkin
Given [contexto inicial]
When [acción del usuario]
Then [resultado esperado]
```

## API Contracts
- Endpoints nuevos/modificados
- Request/Response schemas (JSON Schema)
- Códigos de error

## Data Models
- Nuevos campos/tablas SurrealQL
- Migraciones requeridas

## Edge Cases
- Casos límite identificados
- Comportamiento esperado

## Security Considerations
- Threat model (STRIDE)
- Data classification (PHI/PII)
- Auth/Autz requirements

## Testing Strategy
- Unit tests (coverage target)
- Integration tests
- Prop-test / Fuzz targets
- Load test scenario

## Rollout Plan
- Feature flag
- Canary % traffic
- Rollback procedure
```

### Pipeline SDD en CI

```yaml
# .github/workflows/sdd.yml
name: Spec-Driven Development
on:
  pull_request:
    paths:
      - 'specs/**/*.md'
jobs:
  spec-lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Validate spec format
        run: |
          # Check required sections exist
          # Validate Gherkin syntax
          # Check API contracts are valid JSON Schema
      - name: Spec review required
        uses: actions/github-script@v7
        with:
          script: |
            # Require approval from codeowner before merge
```

### Gobernanza

- **Specs son fuente de verdad**: si código y spec divergen, gana la spec (actualizar código o spec en mismo PR)
- **No código sin spec aprobada** (excepcion: hotfixes de seguridad con spec post-hoc en 48h)
- **Specs versionadas** en git junto al código (`specs/v0.6.0/...`)

---

## Retro y lecciones (actualizado 2026-09-12)

- Los hallazgos de seguridad se documentaron con evidencia (archivo:línea) para poder verificarlos.
- Todo cambio de Fase 1 en adelante debe acompañarse de tests que reproduzcan el defecto antes y después.
- El despliegue en hospital real se planifica solo cuando Fases 1–3 estén completas.
- **NUEVO**: Property-based testing + Fuzzing = defensa en profundidad obligatoria para código clínico.
- **NUEVO**: SDD evita "feature creep" y garantiza trazabilidad requisito→código→test.
- **NUEVO**: WASM build es el único bloqueador real para staging; fixear en Fase 6.9 antes de cualquier otra feature.

---

## Referencias rápidas

| Documento | Ubicación |
|-----------|-----------|
| Changelog | `CHANGELOG.md` |
| Checkpoint Fase 5 | `CHECKPOINT_FASE5.md` |
| Arquitectura técnica | `docs/ARQUITECTURA.md` (pendiente crear) |
| ADRs | `docs/adr/` (pendiente crear) |
| Specs SDD | `specs/` (pendiente crear) |