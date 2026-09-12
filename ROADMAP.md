# 🗺️ Roadmap dMart UCI — v3.0

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
- **Criterios de éxito verificables**: `cargo clippy --all-targets --all-features -- -D warnings`, `cargo fmt --check`, `cargo test --all` y pruebas manuales QA.

## Estado global

| Fase | Nombre | Estado |
|------|--------|--------|
| 0 | Limpieza y cimientos | ✅ Completada |
| 1 | Seguridad crítica | ✅ Completada |
| 2 | Arquitectura y datos | ✅ Completada |
| 3 | DevOps y observabilidad | ✅ Completada |
| 4 | Frontend y UX | ⛔ Pendiente |
| 5 | Clínico y QA avanzado | ⛔ Pendiente |

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
| 8 | Alertas operativas (webhook/email: CPU, memoria, disco) | `scripts/`, docker | Notificación en alerta | ⛔ Pendiente |
| 9 | Backup automático SurrealKV (cron diario + retención 30 días) | `scripts/` | Restore probado | ⛔ Pendiente |
| 10 | Docker multi-stage mínimo + healthcheck en Compose | `Dockerfile`, `docker-compose.yml` | Imagen ~50MB | ⛔ Pendiente |
| 11 | `wasm-opt` en CI (2.2MB → ~600KB) | `.github/workflows/ci.yml` | Artifact optimizado | ⛔ Pendiente |
| 12 | Ventajas CI ya aplicadas (jobs paralelos, rama `main`, audit) | `.github/` | — | ✅ |
| 13 | Semantic versioning + changelog automático (`git-cliff`) | repo | Tags + changelog | ⛔ Pendiente |

### Criterios de éxito
- `docker compose up` arranca en orden (healthcheck) y sobrevive reinicios.
- Restore de backup verificado en staging.

### KPIs
- MTTR < 15 min ante caída (alertas + logs estructurados).
- 100% de deploys vía imagen reproducible.

---

## Fase 4 — Frontend y UX ⛔

### Objetivo
Experiencia clínica fluida, accesible y sin pantallas en blanco.

### Tareas

| # | Tarea | Archivos | Criterio |
|---|-------|----------|----------|
| 1 | Loading states + error handling en todas las páginas (Suspense/fallback) | `dmart-app/src/pages/*` | Sin pantallas vacías |
| 2 | PWA Offline (service worker cache-first para WASM) | `dmart-app/` | Opera sin internet |
| 3 | Notificaciones de deterioro (Web Push API) | `dmart-app/`, `security.rs` | Alertas en vivo |
| 4 | Accesibilidad WCAG 2.1 AA (ARIA, contraste, teclado) | `dmart-app/src/components/*` | Auditoría axe sin errores |
| 5 | Virtual scrolling en listas 1000+ pacientes | `dmart-app/src/pages/patients.rs` | DOM estable |
| 6 | Dark mode respetando `prefers-color-scheme` | `dmart-app/src/stores/theme.rs` | Persistencia confirmada |
| 7 | Búsqueda reactiva debounce 300ms | `dmart-app/src/pages/patients.rs` | Feedback < 300ms |
| 8 | WebSocket streaming de scores en tiempo real | `dmart-server`, `dmart-app` | Datos frescos sin recarga |

### Criterios de éxito
- Lighthouse accesibilidad ≥ 95.
- Interacciones core INP < 200ms.

### KPIs
- 90% de tareas clínicas en ≤ 3 clics.

---

## Fase 5 — Clínico y QA avanzado ⛔

### Objetivo
Expandir valor clínico real y blindar la calidad con pruebas avanzadas.

### Tareas

| # | Tarea | Archivos | Criterio |
|---|-------|----------|----------|
| 1 | Activar endpoints FHIR R4 (Patient, Observation, Condition CIE-10) | `api/fhir.rs` | Interop validada |
| 2 | HL7 V2 / MQTT para monitores (Mindray, Philips) | `dmart-server/src/hl7` | Parseo de HL7 en tests |
| 3 | Dashboard ejecutivo (heatmap camas, KPIs mortalidad predicha vs real, LOS) | `dmart-app/src/pages/dashboard.rs` | Vista de mando |
| 4 | Reportes PDF con marca, FHIR DiagnosticReport y QR | `api/export.rs` | PDF verificable |
| 5 | E2E tests (Playwright: login, pacientes, mediciones, admin) | `e2e/` | Flujos completos en CI |
| 6 | Property-based testing (proptest) de escalas clínicas | `dmart-shared` | Casos borde cubiertos |
| 7 | Fuzzing de API (JSON malformado, inyección) | `dmart-server/tests` | Resistente a entrada hostil |
| 8 | Load testing con k6 (100/500/1000 usuarios) | `load/` | Perfil de rendimiento |
| 9 | Predicción de deterioro (ML, Burn) como piloto académico | `dmart-server/src/ml` | Prototipo con métricas |
| 10 | GCS animado (input visual interactivo) | `dmart-app/src/components/scales/gcs.rs` | Ingreso rápido |

### Criterios de éxito
- Flujo E2E completo verde en CI.
- Reporte de load test documentado y publicado en repo.

### KPIs
- 0 regresiones clínicas en escalas (todas con proptest).
- Disponibilidad del sistema ≥ 99.5% en piloto.

---

## Retro y lecciones (a mantener)

- Los hallazgos de seguridad se documentaron con evidencia (archivo:línea) para poder verificarlos.
- Todo cambio de Fase 1 en adelante debe acompañarse de tests que reproduzcan el defecto antes y después.
- El despliegue en hospital real se planifica solo cuando Fases 1–3 estén completas.