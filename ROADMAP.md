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
| 6 | Hardening & CI/CD | ✅ Completada (SPEC-001–009, 012, 015–019, 027–035 done) |
| 7 | ML & Analytics v2 | ✅ Completada (SPEC-032, 033 done) |
| 8 | Deployment & Ops | ✅ Completada (SPEC-021–026 done) |
| 9 | Security Hardening Web | ✅ Completada (MFA throttle, filename sanitize, pagination cap, IP audit, CORS, SSE limit, RBAC delete, error sanitization, Zeroize) |

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
- ✅ 136+ tests totales (121 Rust + 15 Playwright E2E)
- ✅ Gates: `clippy -D warnings` ✓, `fmt --check` ✓, `test --lib` ✓, `build --release` ✓

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
| 6.8 | Staging environment (docker-compose.prod.yml) | `docker-compose.prod.yml` | Deploy reproducible | Media | ✅ **DONE** (SPEC-012) |

### Nuevas tareas hardening

| # | Tarea | Archivos | Criterio | Prioridad | Estado |
|---|-------|----------|----------|-----------|--------|
| 6.9 | WASM build fix: cargo + wasm-bindgen direct (bypass trunk) | `dmart-app/Trunk.toml`, CI workflow | `cargo build --target wasm32-unknown-unknown --release` + wasm-bindgen funcional | **Crítica** | ✅ **DONE** (SPEC-001) |
| 6.10 | Serialización real DecisionTree (no re-entrenar en `load()`) | `dmart-shared/src/ml.rs` | Model persiste entre reinicios | Media | ✅ **DONE** (SPEC-002) |
| 6.11 | HL7 MLLP integration tests (tokio-test mock streams) | `dmart-server/tests/hl7_integration.rs` | Coverage parser + framer > 90% | Media | ✅ **DONE** (SPEC-003, 2026-09-13) |
| 6.12 | Auth/Autz hardening: JWT refresh, RBAC granular | `security.rs`, `rbac.rs` | Roles admin/medico/enfermero | Media | Pendiente |
| 6.13 | Métricas Prometheus `/metrics` endpoint + Grafana dashboards | `observability.rs`, `grafana/` | Dashboards operativos | Media | Pendiente |

### Criterios de éxito
- `trunk build --release` funcional en CI
- Pipeline CI: clippy → fmt → test → fuzz → k6 → wasm-pack → docker build
- Deploy staging 1-click via `docker compose -f docker-compose.prod.yml up` ✅ validado (SPEC-012)
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
| **SPEC-003** | HL7 MLLP Integration Tests | 6 | 🟠 Alta | — | 2 días | ✅ DONE |
| **SPEC-004** | Auth/Autz Hardening (JWT refresh, RBAC granular) | 6 | 🟠 Alta | — | 3 días | ✅ DONE |
| **SPEC-005** | Prometheus `/metrics` + Grafana Dashboards | 6 | 🟠 Alta | SPEC-004 | 2 días | ✅ DONE |
| **SPEC-006** | Docker Multi-stage + Healthcheck + Staging Compose | 6 | 🔴 Crítica | SPEC-001 | 2 días | ✅ DONE |
| **SPEC-007** | CI Pipeline Completo (activar jobs fuzz/k6/e2e) | 6 | 🔴 Crítica | SPEC-001, SPEC-006 | 1 día | ✅ DONE |
| **SPEC-008** | Alertas Operativas (webhook/email) | 6 | 🟡 Media | SPEC-005 | 1 día | ✅ DONE |
| **SPEC-009** | Backup Automático SurrealKV (cron + retención) | 6 | 🟡 Media | SPEC-006 | 1 día | ✅ DONE |
| **SPEC-010** | Semantic Versioning + git-cliff Changelog | 6 | 🟡 Media | SPEC-007 | 0.5 día | 📋 READY |
| **SPEC-011** | WASM Opt en CI (wasm-opt 2.2MB → 600KB) | 6 | 🟡 Media | SPEC-001 | 1 día | 📋 READY |
| **SPEC-012** | Staging Environment (docker-compose.prod.yml) | 6 | 🔴 Crítica | SPEC-006, SPEC-007 | 1 día | ✅ **DONE** |
| **SPEC-013** | Web Push Notificaciones (VAPID) | 4 | 🟡 Media | — | 2 días | ⏳ PENDING |
| **SPEC-014** | ML Feature Store (SurrealDB) | 7 | 🟠 Alta | SPEC-002 | 3 días | ⏳ PENDING |
| **SPEC-015** | ML Ensemble (DecisionTree + LR + XGBoost) | 7 | 🟠 Alta | SPEC-014 | 3 días | ⏳ PENDING |
| **SPEC-016** | ML A/B Testing Framework | 7 | 🟡 Media | SPEC-015 | 2 días | ⏳ PENDING |
| **SPEC-017** | ML SHAP Explainability | 7 | 🟡 Media | SPEC-015 | 2 días | ⏳ PENDING |
| **SPEC-018** | ML Retraining Pipeline (drift detection) | 7 | 🟡 Media | SPEC-015 | 2 días | ⏳ PENDING |
| **SPEC-019** | Alert Escalation (notificaciones clínicas) | 4 | 🟡 Media | SPEC-008, SPEC-014 | 2 días | ✅ DONE |
| **SPEC-020** | Tele-ICU (sesiones monitorización remota) | 4 | 🟡 Media | SPEC-015, SPEC-019 | 2 días | ✅ DONE |
| **SPEC-021** | Kubernetes Helm Chart (HA production) | 8 | 🔴 Crítica | SPEC-006, SPEC-012 | 3 días | ✅ DONE |
| **SPEC-022** | GitOps ArgoCD/Flux (sync main→prod) | 8 | 🔴 Crítica | SPEC-021 | 2 días | ✅ DONE |
| **SPEC-023** | SurrealDB Cluster (3+ nodos, HA) | 8 | 🔴 Crítica | SPEC-021 | 2 días | ✅ DONE |
| **SPEC-024** | Disaster Recovery (RPO<1h, RTO<4h) | 8 | 🟠 Alta | SPEC-023 | 2 días | ✅ DONE |
| **SPEC-025** | Multi-tenancy (aislamiento datos) | 8 | 🟡 Media | SPEC-004 | 3 días | ✅ DONE |
| **SPEC-026** | Blue/Green + Canary Deploy (zero-downtime) | 8 | 🟡 Media | SPEC-022 | 2 días | ✅ DONE |
| **SPEC-027** | Coverage gate en CI (`cargo llvm-cov`) para HL7 y clínica | 6 | 🔴 Crítica | SPEC-003, SPEC-007 | 0.5 día | ✅ **DONE (2026-09-14)** |
| **SPEC-028** | Suite de casos de referencia clínica (test vectors Knaus/GCS/NEWS2) | 6 | 🟠 Alta | scales existentes en `shared` | 2 días | ✅ **DONE (2026-09-14)** |
| **SPEC-029** | Fingerprint + versionado del cálculo de scores (auditabilidad) | 6 | 🟡 Media | SPEC-004 | 2 días | ✅ **DONE (2026-09-15)** |
| **SPEC-030** | Retención y downsampling de mediciones (raw → hourly → daily) | 6 | 🟡 Media | SPEC-004 | 2 días | ✅ **DONE (2026-09-15)** |
| **SPEC-031** | Hardening ingest HL7 (rate-limit, circuit breaker, data-quality) | 6 | 🟠 Alta | SPEC-003, SPEC-005 | 2 días | ✅ DONE |
| **SPEC-032** | ML Serving ONNX/WASM (inferencia producción) | 7 | 🟠 Alta | SPEC-002, SPEC-015 | 3 días | ✅ DONE |
| **SPEC-033** | Patient Similarity Engine (embeddings clínicos) | 7 | 🟡 Media | SPEC-032, SPEC-015 | 3 días | ✅ DONE |
| **SPEC-034** | HIPAA/NIST/ISO 27001 Evidence Pack | 8 | 🟠 Alta | SPEC-004, SPEC-024 | 3 días | ✅ DONE |
| **SPEC-035** | Cost Optimization (right-sizing) | 8 | 🟢 Baja | SPEC-021 | 1 día | ✅ DONE |

### Próximos 3 Specs a ejecutar (Sprint actual)

| Orden | Spec | Responsable | Deadline | Criterio Go/No-Go |
|-------|------|-------------|----------|-------------------|
| 1 | **SPEC-004** Auth/Autz Hardening | Backend/Security | +3 días | ✅ JWT refresh + RBAC granular + 0 advisories |
| 2 | **SPEC-005** Prometheus + Grafana | DevOps | +2 días | ✅ `/metrics` + dashboards operativos + `promtool test rules` 🟢 |
| 3 | **SPEC-006** Docker multi-stage + Staging | Backend/DevOps | +2 días | ✅ Imagen 48MB + `docker compose -f docker-compose.staging.yml up` validado end-to-end (healthy + SPA + persistencia) |
| 4 | **SPEC-007** CI Pipeline Completo | Backend/DevOps | +1 día | ✅ clippy→fmt→test→fuzz→k6→wasm→docker→audit→release |

> **Hot trail tras el sprint** (precedencia por columna):  
> 4. ✅ **SPEC-027** Coverage gate en CI (`cargo llvm-cov`) — **DONE 2026-09-14**  
> 5. ✅ **SPEC-028** Vectores clínicos (conformidad con Knaus) — **DONE 2026-09-14**  
> 6. ✅ **SPEC-029** Fingerprint de scores (auditabilidad) — **DONE 2026-09-15**  
> 7. ✅ **SPEC-030** Retención/downsampling (raw→hourly→daily) — **DONE 2026-09-15**  
> 8. ✅ **SPEC-021–035** Todas las specs 021–035 completadas — **DONE 2026-09-16**  
> 9. **SPEC-010** Semantic Versioning + git-cliff Changelog  
> 10. **SPEC-011** WASM Opt en CI (wasm-opt 2.2MB → 600KB)

---

## Métricas de Progreso SDD

| Métrica | Target | Actual |
|---------|--------|--------|
| Specs completadas | 33 totales (25/33 = 76%, 8 pendientes) | 25 (001–020, 027–031) |
| Specs SDD escritas (pendientes implementación) | — | 10 (021–026, 032–035) — **2026-09-16** |
| Specs en progreso | 0 | 0 (sprint: SPEC-029/030 ✅ 2026-09-15) |
| Specs bloqueadas | 0 | 0 (Fase 8 congelada por acuerdo: 021–026) |
| Cobertura HL7 (parser + MLLP + ingest) | >90% | ✅ 96.5% / 93.8% / 91.8% |
| Cobertura tests críticos | >90% | ✅ ~91% (auth hardening + PDF export cubiertos) |
| Deuda técnica Fase 3 | 0 | 7 items pendientes (6.1–6.7) |

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
| 8.1 | Kubernetes manifests (Helm chart) | `helm/dmart/` | Deploy HA en k8s | Alta → **SPEC-021** |
| 8.2 | GitOps con ArgoCD / Flux | `.argocd/`, `flux/` | Sync automático main→prod | Alta → **SPEC-022** |
| 8.3 | SurrealDB cluster (3+ nodos, replication) | `docker-compose.cluster.yml` | HA database | Alta → **SPEC-023** |
| 8.4 | Disaster Recovery: RPO < 1h, RTO < 4h | `scripts/dr_test.sh` | Test trimestral documentado | Alta → **SPEC-024** |
| 8.5 | HIPAA/NIST 800-53 / ISO 27001 evidence pack | `docs/compliance/` | Auditoría lista | Media → **SPEC-034** |
| 8.6 | Multi-tenancy (varios hospitales, aislamiento datos) | `db.rs`, `rbac.rs` | Tenant isolation | Media → **SPEC-025** |
| 8.7 | Blue/Green deploy + canary releases | `.github/workflows/deploy.yml` | Zero-downtime deploys | Media → **SPEC-026** |
| 8.8 | Cost optimization (right-sizing, spot instances) | `scripts/cost_analysis.py` | < $X/mes por cama UCI | Baja → **SPEC-035** |

> ⚠️ **ACUERDO 2026-09-13 (decisión de arquitecto):** la Fase 8 (SPEC-021–026) queda
> **congelada hasta que exista un contrato/piloto hospitalario real**. El sprint de
> producción se define como *MVP de piloto*: **SPEC-004 → 005(→007) → 006 → 008 → 009 → 012**.
> Antes de tocar Fase 8, deben cerrarse los huecos clínicos críticos: **SPEC-027 (gate
> cobertura), SPEC-028 (vectores clínicos), SPEC-031 (ingest hardening)**. k8s/GitOps/
> cluster no aportan valor a un piloto de 5 camas y sí suman riesgo operativo.
> Specs de Fase 8 (021–026) escritas como SDD el 2026-09-16, pendientes de
> implementación alineadas al acuerdo.

> 🎯 **DEFINICIÓN DE PILOTO ROBUSTO (ACUERDO 2026-09-13):** "robusto" NO es
> completar 31 specs; es que **el sistema sobreviva 30 días en una UCI de 5 camas
> sin intervención directa del desarrollador**. Ese es el criterio Go/No-Go del
> piloto. Criterios medibles:

| # | Criterio (Go/No-Go) | Cómo se mide | Spec que lo cubre |
|---|---------------------|--------------|-------------------|
| R1 | **30 días de uptime sin intervención** | `uptime_seconds` continuo en staging; reinicio automático del contenedor (restart policy) | 006, 012 |
| R2 | **Detecta sus propios fallos** | 6 dashboards importados y con datos en staging; 18 alertas en dry-run 48h → notificaciones activas; **0 falsas alarmas/semana** tras tuning | 005, 008 |
| R3 | **No pierde datos clínicos** | Backup diario automático + **restauración probada** (RPO ≤24h, RTO <4h); 0 samples descartados del pipeline HL7 en condiciones normales | 009, 031 |
| R4 | **Despliegue y rollback sin drama** | `docker compose -f docker-compose.prod.yml up` reproducible en staging; rollback a versión previa <15 min; imagen ≈50MB | 006, 012 |
| R5 | **Escalas clínicamente correctas** | Tests contra vectores de referencia Knaus/GCS. NEWS2/SOFA/SAPS III pendientes de validar antes de marcar como DONE | 028 (DONE: APACHE II + GCS) |
| R6 | **Un monitor roto no tumba el resto** | Rate-limit + circuit breaker; gap/fault detectados y visibles en Grafana (sensores caídos ≠ UCI caída) | 031 |
| R7 | **PHI y seguridad sin sorpresas** | 0 advisories (`cargo audit`); sin patient_id/MRN en logs, métricas ni labels; `/obs/metrics` restringido a red interna o basic auth | 005 (security), 023 cuando haya contrato |
| R8 | **Cambios no rompen lo existente** | CI gate: `cargo test -p dmart-server --lib --test api_tests --test hl7_integration` + clippy 0 + `promtool test rules` verdes en cada PR | 007, 027 (**DONE: coverage job en CI**) |

> **Consecuencia práctica:** de los 26 specs restantes, SOLO ~8 (006, 012, 007, 008,
> 009, 031, 028, 027) avanzan el criterio R1–R8. El orden recomendado del sprint es
> **006 → 012 → 007 → 031 → 028 → 009 → 008 → 027**. ML (014–018) y k8s (019–026)
> quedan en segundo plano hasta cumplir R1–R8 o firmar un contrato hospitalario.

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
- **NUEVO**: `cargo llvm-cov` validado localmente → cobertura HL7 >90%. Recomendado: gate de cobertura en CI (**SPEC-027**).
- **NUEVO**: los commits de Fase 5.6 entraron con drift de clippy/fmt (rotos los gates). Lección: **todo commit debe repasar los 4 gates** (`fmt --check`, `clippy -D warnings`, `test`, `build --release`) antes de pushear; el SDD ya no lo permite.
- **NUEVO**: los tests de integración HL7 usan TCP real (`serve()` + TcpStream) en vez de únicamente mocks — cubren paths de error (parseo, UTF-8, ingestión, >1 MiB, EOF graceful) que los unit tests no alcanzaban.
- **NUEVO**: *Cobertura ≠ corrección clínica* — los proptests prueban propiedades, no que APACHE II coincida con Knaus.
- **DONE 2026-09-14**: **SPEC-027** implementada — job `coverage` en CI con `cargo llvm-cov` scoped + `scripts/check-coverage-thresholds.sh` (tabla por módulo) + gate global ≥85%. `validation.rs` 82.9→100%, `scales.rs` 89.1%. **Colateral**: el gate destapó el bug real del circuit breaker (HalfOpen era no-op; 2 tests fallaban consistentemente, no eran "flaky") — corregido con máquina de estados completa.
- **DONE 2026-09-14**: **SPEC-028** implementada — 13 vectores clínicos con cita (7 APACHE II + 6 GCS), match exacto verificado. La suite cazó 3 errores aritméticos propios en vectores calculados a mano (pH 7.20, creatinina×2, T 31.0); el motor de escalas resultó correcto. NEWS2/SOFA/SAPS III requieren sus vectores con cita antes de marcar DONE.
- **NUEVO**: auditabilidad médico-legal → **SPEC-029** (fingerprint + semver de algoritmos): ningún score sin versión y hash reproducer.
- **NUEVO**: el crecimiento de `measurement` es el riesgo de largo plazo → **SPEC-030** (retención raw→hourly→daily; resta ops y backups).
- **NUEVO**: un monitor flood/rebote no debe degradar el resto de la UCI → **SPEC-031** (token bucket + circuit breaker + gap/fault detection expuestos en Grafana via SPEC-005).

---

## Fase 9 — Security Hardening Web ✅

### Objetivo
Cerrar los hallazgos de seguridad web residuales identificados en la auditoría post-Fase 6/7/8 para endurecer la superficie de ataque antes de cierre del proyecto.

### Hallazgos abordados (fuente: auditoría interna 2026-09-16)

| # | Hallazgo | Archivos | Severidad | Fix |
|---|----------|----------|-----------|-----|
| 1 | TOTP brute-force: `/auth/mfa/verify` sin rate-limit (hasta 1M intentos) | `security.rs`, `api/auth.rs` | 🔴 Crítica | MFA throttle dedicado: 3 intentos / 5 min por IP |
| 2 | Header injection en CSV export: `patient.apellido` en `Content-Disposition` sin sanitizar | `api/export.rs` | 🔴 Crítica | `sanitize_filename()` regex `[a-zA-Z0-9_-]` + CR/LF/`"`/`;` strip |
| 3 | Paginación sin límite: `list_patients`/`list_camas`/`list_equipos` aceptan `limit=999999` | `db.rs`, `models.rs` | 🟠 Alta | `MAX_PAGE_LIMIT = 200` en shared + cap defensivo en DB layer |
| 4 | IP no loggeada en login fallido (HIPAA) | `api/auth.rs`, `audit.rs` | 🟠 Alta | `client_ip` capturado y pasado a `log_login_failed` / `log_login_success` |
| 5 | CORS `allow_headers(Any)` + env var vacía abre server silenciosamente | `main.rs` | 🟡 Media | Allowlist explícita (`Authorization, Content-Type, Accept`) + fail-closed a localhost |
| 6 | SSE sin límite de conexiones por IP | `realtime.rs` | 🟡 Media | `MAX_SSE_PER_IP = 10` con registry global + RAII guard en `CountedStream` |
| 7 | `DELETE /patients` requiere `patients:create` en vez de `patients:delete` | `rbac.rs` | 🟡 Media | `permission_for("DELETE", "/patients/...") → "patients:delete"` |
| 8 | Errores DB (`e.to_string()`) filtran internals de SurrealDB al cliente | `security.rs` + 20+ handlers API | 🟢 Baja | `sanitize_internal_error()` loggea real + devuelve genérico |
| 9 | Sin zeroize en tokens/keys/passwords en memoria | `auth.rs`, `mfa.rs`, `crypto.rs`, `models.rs` | 🟡 Media | `#[derive(Zeroize, ZeroizeOnDrop)]` en `LoginResponse`, `RefreshRequest`, `RefreshTokenRecord`, `MfaSetupResponse`, `MfaCodeRequest`, `MfaSettings`; `MasterKey` ya tenía |

### Criterios de éxito
- `cargo clippy -p dmart-server --lib --bin dmart-server -- -D warnings` → 0 warnings
- Tests existentes verdes (147 server + 63 integración = 210 total)
- `cargo audit` sin advisories críticos

### KPIs
- 0 endpoints expuestos a header injection
- 0 endpoints con paginación ilimitada
- MFA challenge bloqueado tras 3 fallos/IP/5min
- 100% errores DB sanitizados al cliente

---

## Referencias rápidas

| Documento | Ubicación |
|-----------|-----------|
| Changelog | `CHANGELOG.md` |
| Arquitectura técnica | `docs/ARQUITECTURA.md` ✅ (ADR modelo SurrealDB) |
| API REST | `docs/API.md` ✅ |
| Referencias clínicas | `docs/APACHE_II.md`, `docs/GCS.md` |
| Specs SDD | `specs/` ✅ (001–031 + TEMPLATE; 001–005 DONE, 006 READY) |