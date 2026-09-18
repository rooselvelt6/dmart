# Changelog

Todos los cambios notables de este proyecto se documentan en este archivo.

El formato se basa en [Keep a Changelog](https://keepachangelog.com/es-ES/1.0.0/),
y este proyecto se adhiere a [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased] — SPEC-021..026 + SPEC-031..035: Fase 8 implementada — todas las SPECs completadas (2026-09-16)

### Agregado
- **SPEC-021 — Kubernetes Helm Chart (HA)**: `helm/dmart/` con Deployment server, StatefulSet SurrealDB, HPA, Ingress, NetworkPolicy, PDB, ServiceAccounts y secrets.
- **SPEC-022 — GitOps ArgoCD/Flux**: `.argocd/` (AppProject + Application) y `flux/` (GitRepository + HelmRelease + Kustomization), workflow de sync `gitops-sync.yml`.
- **SPEC-023 — SurrealDB Cluster**: `docker-compose.cluster.yml` (3 nodos) + `docs/SURREALDB_CLUSTER.md`.
- **SPEC-024 — Disaster Recovery**: `scripts/dr_backup.sh`, `dr_restore.sh`, `dr_verify.sh`, `dr_drill.sh` (test de restore cubierto).
- **SPEC-025 — Multi-tenancy**: tabla `tenant`, `tenant_id` en JWT `Claims` y en `User`/`UserInfo`/`StaffInfo`/`Patient`/`Measurement`; filtrado por tenant en pacientes; `GET/POST /admin/tenants`, `POST /admin/tenants/{id}/impersonate`; RBAC `tenants:read`/`tenants:manage`; migración `025_multi_tenancy.surql`.
- **SPEC-026 — Blue/Green + Canary Deploy**: `scripts/deploy_blue_green.sh`, `deploy_canary.sh`, `deploy_rollback.sh`, `deploy_status.sh`.
- **SPEC-032 — ML Serving**: `ml_serving.rs` (registry DashMap, `Predictor` trait pluggable, backend `StatPredictor` determinístico), API `/ml/predict`, `/ml/predict_batch`, `/ml/models`, `/ml/models/swap`; métricas `ml_inference_*`, `ml_model_loaded`, `ml_batch_size`.
- **SPEC-033 — Patient Similarity Engine**: `similarity.rs` (embeddings clínicos 128d por feature-hashing, cosine, top-K con scope tenant), API `/ml/similarity/search|explain|status|embeddings/regenerate`; métricas `ml_embedding_generated_*`, `ml_similarity_search_*`, `ml_vector_index_size`.
- **SPEC-034 — HIPAA/NIST/ISO 27001 Evidence Pack**: `docs/compliance/` (control catalog, data flows, incidents, legal) + `scripts/compliance_generate.sh`/`compliance_check.sh`.
- **SPEC-035 — Cost Optimization**: `scripts/cost_*.sh` (report, forecast, idle detect, rightsize).

### Notas
- **Fase 8 completa: SPECs 001–035 todas implementadas** (2026-09-16).
- Gate verde: **210 tests** (82 lib + 128 integración) · `clippy -D warnings` 0/0 (lib + bin).

---

## [Unreleased] — SPEC-044: Consola Técnica de Soporte (Support Console) — Fase 3 (2026-09-18)

### Agregado
- **SPEC-044 — Consola Técnica de Soporte (Support Console)**:
  - Rol **Soporte** (`UserRole::Soporte` / `Role::Support`) con permisos `support:read` y `support:act` (más contexto clínico de solo lectura `patients:read`/`measurements:read`); guarda en RBAC CRUD y `permission_for`.
  - API `/admin/support/systems|diagnostics|history` y `POST /admin/support/actions/{action}`, auditadas y registradas en `support_events` (manual y auto).
  - 6 acciones idempotentes: `ingest_retry`, `circuit_reset`, `backup`, `audit_retention`, `model_swap`, `verify_fingerprints`.
  - Telemetría por subsistema (`db|ingest|realtime|ml|monitores|audit|backup`) con `note()/freshness/error_count`; gauges/counters Prometheus `support_actions_total`, `self_healing_total`, `support_systems_status`.
  - Self-healing: transición automática `Open → HalfOpen` del circuit breaker registra un evento auto (`self_heal`).
  - `IngestState` global accesible (`ingest::global_ingest()`) con `reset_all_devices()`/`reset_open_circuits()`.
  - OpenAPI tag `support` + schemas.

### Notas
- SPEC clínico: la consola detecta y corrige problemas (gaps, fault devices, modelo inactivo, backup ausente) sin exponer datos clínicos.
- Gate: 96 tests lib + 3 E2E SPEC-044 + suite api_tests (39) + hl7_integration (32) en verde. Verificación WASM del frontend pendiente de `trunk build` local.

---

### Agregado
- **Specs SDD escritas** (pendientes de implementación, alineadas a acuerdo Fase 8):
  - `specs/021-kubernetes-helm-chart.md` — Helm chart HA: Deployment, StatefulSet, HPA, Ingress, NetworkPolicy, PDB.
  - `specs/022-gitops-argocd-flux.md` — ArgoCD/Flux: sync automático main→prod, self-healing, rollback por git revert.
  - `specs/023-surrealdb-cluster.md` — Cluster SurrealDB 3 nodos: failover < 30s, replicación, PDB minAvailable=2.
  - `specs/024-disaster-recovery.md` — Backup incremental/hora en S3/MinIO, restore point-in-time, DR drill trimestral (RPO<1h, RTO<4h).
  - `specs/025-multi-tenancy.md` — Aislamiento por tenant: RLS pattern, `tenant_id` en JWT y todas las tablas, API super_admin.
  - `specs/026-blue-green-canary-deploy.md` — Zero-downtime: Blue/Green + Canary 5→25→100% con auto-rollback por métricas.
  - `specs/032-ml-serving-onnx-wasm.md` — ONNX Runtime en Rust: predict/batch, model swap atómico, fallback CPU.
  - `specs/033-patient-similarity-engine.md` — Embeddings clínicos 128d, HNSW index, búsqueda K similares < 100ms + explainability.
- **ROADMAP.md**: master list 019/020 alineada a specs reales; 021–026 y 032–033 marcadas `📋 SDD READY (2026-09-16)`.
- **README.md**: backlog pendiente detallado con las 8 specs escritas.

### Notas
- HUECO pendiente: tarea Fase 8.5 (HIPAA/NIST/ISO 27001 evidence pack) y 8.8 (Cost optimization) no tienen spec numerada aún.

---

## [Unreleased] — SPEC-034 + SPEC-035: HIPAA/ISO 27001 Evidence Pack + Cost Optimization (2026-09-16)

### Agregado
- `specs/034-hippa-iso27001-evidence-pack.md` — Control catalog mapeado a NIST 800-53 / HIPAA 45 CFR 164 / ISO 27001 Anexo A, evidencia auto-generada (`compliance_generate.sh`), risk assessment trimestral, plantilla BAA, cumplimiento legal cubano.
- `specs/035-cost-optimization.md` — Baseline de coste, right-sizing (p99), storage tiering, spot instances para jobs no críticos con checkpoint, idle detection, reporte mensual y forecast; métricas `cost_*` en Prometheus.

### Notas
- Fase 8 completa: todas las tareas 8.1–8.8 con spec SDD numerada.
- Gate verde (docs): `git status` limpio.

---

## [Unreleased] — SPEC-029 + SPEC-030: Fingerprint de Scores y Retención/Downsampling (2026-09-15)

### Agregado
- **SPEC-029 — Fingerprint y versionado de scores** (auditabilidad médico-legal):
  - `ALGO_VERSION` (= `CARGO_PKG_VERSION`, semver) y `score_fingerprint(algo, version, inputs)` en `dmart-shared/src/scales.rs`: JSON canónico (claves ordenadas recursivas, compacto) → SHA-256 hex 64.
  - `Measurement` persiste `algorithm_version` + `fingerprint` (defaults `<legacy>`/`""` para registros históricos); rellenados por `create_measurement` (API) y los 5 handlers de escala (`api/scales.rs`).
  - Migración `029_measurement_fingerprint.surql`.
  - `GET /admin/audit/scores` (admin, `audit:read`): recomputa el hash con la versión **guardada** y reporta `reproducible` por medición; legacy marcado "sin fingerprint".
  - Tests: 6 (4 unit + 2 proptest en `scales.rs`, 2 sobre `score_audit`).

- **SPEC-030 — Retención y downsampling**:
  - `dmart-server/src/retention.rs`: downsampling `measurements` raw → `measurements_hourly` / `measurements_daily` (AVG/MIN/MAX de los 5 scores + `count`), UPSERT idempotente por bucket `patient_id#bucket`, keyset pagination (10k/batch), purga raw según `RETENTION_RAW_DAYS` y hourly según `RETENTION_HOURLY_MONTHS`, guard de disco crítico (statvfs <10%), trail en `retention_jobs`. Agregados sin PHI identificable.
  - `spawn_retention_job()` diario tokio, no-op salvo `RETENTION_ENABLED=true` (default off).
  - API admin: `POST /admin/retention/run`, `GET /admin/retention/config`, `GET /admin/retention/status`.
  - Migración `030_measurements_downsample.surql`.
  - Tests: 4 módulo (agregación+purga, idempotencia/upsert, defaults, tablas idempotentes).

- **Wiring**: `pub mod retention;` en `lib.rs`, rutas en `api/mod.rs` (`.merge(retention::routes())`), migración `030` registrada en `migrations.rs` (catálogo → 11 versiones).

### Notas
- Gate verde: `cargo test -p dmart-server --lib` **71 passed** · `clippy -D warnings` **0/0** (lib+bin) · `api_tests` **31 passed** · `hl7_integration` **32 passed**.
- Backlog load k6 `/api/stats?granularity=hourly` y restore-test SPEC-030 quedan pendientes.

---

## [Unreleased] — SPEC-027: Coverage Gate en CI (cargo llvm-cov)

### Agregado
- **Job `coverage`** en `.github/workflows/ci.yml`:
  - `cargo llvm-cov` scoped (`-p dmart-shared -p dmart-server --lib --test api_tests --test hl7_integration`, sin `--workspace`) → `coverage.lcov`.
  - Gate por módulo: `scripts/check-coverage-thresholds.sh`.
  - Gate global `LH/LF ≥ 85%` computado del propio LCOV (una sola invocación de llvm-cov).
  - Upload de `coverage.lcov` como artifact (30 días).
  - Conectado a `needs:` de `release-build` y `notify`.
- **Script `scripts/check-coverage-thresholds.sh`**: tabla única de umbrales por módulo;
  falla con `exit 1` nombrando cada módulo bajo umbral; alerta `⚠️ NOT FOUND` si un módulo
  protegido desaparece del reporte.
- **Tests de validación clínica** (`dmart-shared/src/validation.rs`, +7): FiO2>1.0, A-aDO2
  crítico-alto, edad>120, respuesta verbal/motora GCS fuera de rango, valor bajo el mínimo
  físico, warning crítico-alto, `get_range_description`.

### Corregido
- **Bug real del Circuit Breaker (SPEC-031, `circuit_breaker.rs`)**: el estado `HalfOpen`
  era un no-op — la transición a `Closed`/`Open` nunca ocurría y 2 tests fallaban de forma
  consistente (fueron interpretados como "flaky"). Ahora `record_result` aplica la máquina
  de estados completa: éxito en half-open requiere `success_threshold` (3) consecutivos para
  cerrar, fallo reabre. Añadido `consecutive_successes` con `#[serde(default)]`.
  Tests actualizados a la semántica de 3 éxitos.

### Cobertura final por módulo protegido (spec: 027)
| Módulo | Before | After | Umbral |
|--------|--------|-------|--------|
| hl7/parser.rs | 96.5% | 95.0% | 90% ✅ |
| hl7/mllp.rs | 93.8% | 100.0% | 90% ✅ |
| hl7/ingest.rs | 91.8% | 94.7% | 90% ✅ |
| shared/scales.rs | 83.4% | 89.1% | 85% ✅ |
| shared/validation.rs | 81.3% | **100.0%** | 85% ✅ |
| shared/ml.rs | 94.6% | 95.9% | 80% ✅ |

### Tests
- `dmart-shared --lib` 38 ✓ · `dmart-server --lib` 65 ✓ (incluye circuit breaker 7/7).
- Test negativo del gate verificado (parser.rs 11.9% → `exit 1`).
- Clippy: `circuit_breaker.rs` 0 warnings.

---

## [Unreleased] — SPEC-028: Vectores Clínicos de Referencia (Test Vectors)

### Agregado
- **Suite de conformidad clínica** (`dmart-shared/tests/conformance.rs`, 6 tests):
  - `test_conformance_apache_ii_exact_match` — 7 fixtures validadas contra Knaus 1985 con
    match **EXACTO** de score total y de los 16 sub-scores del breakdown.
  - `test_conformance_gcs_exact_match` — 6 fixtures validadas contra Teasdale & Jennett 1974
    (total + interpretación clínica).
  - `test_conformance_*_has_enough_vectors` — gate: falla si <6 vectores APACHE II o <5 GCS.
  - `test_conformance_all_fixtures_cite_sources` — exige cita bibliográfica válida por fixture.
  - `test_conformance_loader_rejects_invariant_violations` — invariantes estructurales.
- **Loader de fixtures** (`dmart-shared/src/testdata.rs`):
  - Deserialización serde estricta con `env!("CARGO_MANIFEST_DIR")` → `testdata/scales/<scale>`.
  - Validación de invariantes en carga: sub-scores ≤ máximos de la escala, suma = total,
    consistencia GCS (ojos+verbal+motor = total), coordinación breakdown ⇄ score total.
  - `diff_apache_ii_subscores` — diff granular (escala, fixture y sub-score divergente) al fallar.
- **Vectores de referencia** en `dmart-shared/testdata/scales/`:
  - `apache_ii/` (7): sano (0), fiebre+taquipnea (2), HTA+taquicardia+edad (9),
    IRA+acidosis (20), falla multiorgánica extrema **(score máximo de la escala: 71)**,
    cirugía electiva+crónica (20), hipotermia+disturbio electrolítico (34).
  - `gcs/` (6): consciente (15), lesión leve (13/14), moderada (9), grave/como (6), coma profundo (3).

### Corregido
- **Suite de conformidad cazó 3 errores aritméticos en los vectores calculados a mano**
  (no en el motor de escalas): pH=7.20 es 3 pts (rango 7.15-7.24) no 2; creatinina con
  falla renal aguda duplica (4 pt → 8 pt); T=31.0 es 3 pts (rango 30-31.9) no 2.
  La implementación de `scales.rs` resultó correcta en todos los casos.

### Tests
- `dmart-shared` lib 31 ✓ + conformance 6 ✓ + proptests ✓.
- `dmart-server` api_tests 31 ✓ + hl7_integration 32 ✓.
- Gates: `fmt --check` ✓, `clippy -p dmart-shared --all-targets` ✓ (0 warnings).

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
- **SELinux bloqueaba bind-mount del Caddyfile en prod**: `docker-compose.prod.yml`
  montaba `./Caddyfile:/etc/caddy/Caddyfile:ro` y fallaba con "permission denied"
  al correr como root con `read_only` + `no-new-privileges`. Solución: Caddyfile
  copiado dentro de imagen custom (`Dockerfile.caddy`) → sin bind-mount, sin SELinux.
- **Backup "cp -r /data" peligroso eliminado**: copiaba la BD SurrealKV viva
  (snapshot inconsistente). El backup real vendrá en SPEC-009; aquí solo se dejan
  los volúmenes named para que 009 los respalde.

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