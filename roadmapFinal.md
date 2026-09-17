# 🗺️ Roadmap dMart UCI — Camino único al 100/100

**Documento único de planificación.** Reemplaza a `ROADMAP.md`, `updateIO.md`
(eliminados el 2026-09-17) y a `docs/PLAN_100_DIOS.md` (integrado el 2026-09-17).
Define **un solo camino en 4 fases** para el **trimestre final de 2026 (octubre –
diciembre)** para llevar el producto a **nivel empresa (estándar Microsoft /
Novell / Sun): 100/100 en todos los dominios**.

## Estado actual (entregado 2026-09-17)

- **Fases 0–9 completadas.** SPECs SDD 001–035 implementadas. Escalas clínicas
  validadas contra vectores Knaus/GCS (SPEC-028), coverage>90% (SPEC-027),
  auditabilidad de scores (SPEC-029), retención/downsampling (SPEC-030),
  ingest HL7 endurecida (SPEC-031), ML serving + similaridad (SPEC-032/033),
  evidence pack HIPAA/ISO (SPEC-034), cost opts (SPEC-035).
- **Informe técnico final entregado** a supervisor (PDF 30 págs. con 7 capturas
  reales del sistema; plantilla reutilizable en `docs/templates/`).
- **Funcional hoy**: web (WASM `dist/`), API REST/FHIR/HL7-MLLP/SSE, RBAC + MFA
  server-side, arranque persistente systemd de usuario, backup diario.

## Puntaje 100/100 (diagnóstico con evidencia en código)

| # | Dominio | Actual | Fase de cierre |
|---|---------|-------:|----------------|
| D1 | Clínica y ciencia (escalas validadas) | 80 | F2 |
| D2 | ML e IA (mortalidad, LOS, sepsis, biomarkers) | 40 | F2 |
| D3 | Ingeniería de software (SDD, CI/CD, release) | 75 | F1 |
| D4 | Seguridad y gobernanza | 90 | F3 |
| D5 | Operación y SRE (HA, SLO, autoscaling) | 70 | F3 |
| D6 | Cumplimiento y regulatorio (software médico) | 60 | F4 |
| D7 | Producto y negocio (licencias, soporte, billing) | 45 | F4 |
| | **PROMEDIO** | **66** | **→ 100** |

**Qué define "100/100 tipo Microsoft"** — 5 órdenes de exigencia atacados en las
4 fases: **Vendible** (contrato/licencia/SLA/facturación → F4) · **Confiable**
(artefactos firmados + SBOM + pen-test + certificaciones → F1/F3/F4) ·
**Operable** (SLO + autoscaling + HA + DR comprobado → F3) ·
**Científico** (modelos NN con pipeline de entrenamiento y calibración → F2) ·
**Evolucionable** (release trains semver + telemetría + i18n → F1/F4).

## Objetivo del trimestre final (un solo camino)

| # | Objetivo | Fase |
|---|----------|------|
| 1 | Cimientos de ingeniería de producto: bugs críticos + release formal + API estable | F1 |
| 2 | Ciencia e IA de grado hospitalario: LOS-NN, ensemble, calibración, artículo | F2 |
| 3 | Seguridad empresarial, SRE y backlog operativo | F3 |
| 4 | Regulatorio, producto, negocio y cierre v1.0.0 | F4 |

---

## Fase 1 — Cimientos de ingeniería de producto 🔄 (octubre 2026)

### Objetivo
Cierra Deuda técnica y D3→100: sistema estable, sin pantallas rotas, liberado con
semver, API versionada y artefactos verificables. Desbloquea el resto del camino.

### Tareas

| # | Área | Tarea | Archivos | Criterio de éxito | Prioridad |
|---|------|-------|----------|-------------------|-----------|
| 1.1 | Frontend/Bug | Fix **`/perfil` renderiza en blanco** (última pantalla rota detectada) | `dmart-server/src/main.rs` | `/perfil` muestra cambio de contraseña y datos de sesión | 🔴 Crítica | ✅ Hecho |
| 1.2 | Auth/UX | **Reto TOTP en login web** (`login.rs`) para activar MFA desde UI sin quedarse fuera | `dmart-app/src/pages/login.rs`, `api.rs` | Flujo login → challenge TOTP → sesión; activar MFA desde `/perfil` verificado | 🔴 Crítica | ✅ Hecho |
| 1.3 | Release | **SPEC-010** — Semantic versioning + git-cliff: tags `vX.Y.Z` + flujo release en CI | `.github/workflows/release.yml`, `cliff.toml`, `CHANGELOG.md` | Tag `v0.9.0` creado desde CI + changelog automático | 🟠 Alta | ✅ Hecho |
| 1.4 | Build | **SPEC-011** — `wasm-opt` en CI (WASM 2.59MB → 1.73MB; gate anti-regresión 2MB) | `.github/workflows/ci.yml`, `scripts/optimize-wasm.sh`, `dmart-shared/Cargo.toml`, `dmart-shared/src/time.rs` | WASM < 1MB (chrono desacoplado de `dmart-shared` via feature `wasm-time`) | 🟠 Alta | ✅ Hecho |
| 1.5 | API | **API Gateway + versionado**: prefijo `/api/v1`, cabecera de versión, `utoipa` (OpenAPI) + SDK cliente generado en CI | `main.rs`, `dmart-server/src/api/mod.rs`, `dmart-server/src/api/versioning.rs` | `/api/v1/*` documentado en `openapi.json` | 🟠 Alta | ✅ Hecho |
| 1.6 | Flags | **Feature flags server** (flags en SurrealDB + middleware) | `dmart-server/src/api/flags.rs`, `db.rs`, `dmart-shared/src/models.rs`, `migrations/047_feature_flags.surql` | Flags por tenant sin redeploy | 🟡 Media | ✅ Hecho |
| 1.7 | Deps | **Política de dependencias**: `cargo-deny` (licencias + advisories) + dependabot/renovate | `.github/`, `deny.toml` | `cargo deny check` verde en CI | 🟠 Alta | ✅ Hecho |
| 1.8 | QA | **Contratos API estables**: golden JSON entre server y app; semver para breaking + smoke E2E | `tests/`, `tests/e2e/*.spec.ts`, `dmart-server/tests/contract_tests.rs` | Test de contrato + suite E2E verdes en CI | 🟠 Alta | ✅ Hecho |

### Criterios de éxito
- `cargo test -p dmart-server --test api_tests --test hl7_integration` verde (~15s).
- `trunk build --release` + clippy `-D warnings` + `fmt --check` sin errores.
- 0 pantallas en blanco recorriendo `/login → / → /patients → /perfil → /admin`.
- Tag release v0.9.0 generado automáticamente.

---

## Fase 2 — Ciencia e IA de grado hospitalario 🟡 (octubre – noviembre 2026)

### Objetivo
Cierra D1→100 y D2→100: **redes neuronales para LOS**, ensemble de mortalidad,
calibración estadística, terminología clínica — y lo publica como **artículo
científico** con evidencia.

### Tareas

| # | Área | Tarea | Archivos | Criterio de éxito | Prioridad |
|---|------|-------|----------|-------------------|-----------|
| 2.1 | Datos | **SPEC-014** — Feature store versionado | `dmart-shared/src/ml_features.rs` | Mismas features ↔ mismo hash, reproducible | 🟠 Alta |
| 2.2 | ML/NN | **SPEC-036** — **Predicción de estancia (LOS) con red neuronal**: serie temporal (MLP ventana fija + LSTM/GRU) sobre `measurement`/`patient_events`; backend `candle`/`ort` vía trait `Predictor` (sin tocar endpoints); split temporal, validación por hospital, MAE/RMSE/MAPE + binned acc (±24h/±48h), calibración isotónica, CI bootstrap | `dmart-shared/src/ml_los.rs`, `dmart-server/src/ml_serving.rs` | MAE reportado, modelo registrado en `MlRegistry` con SHA-256 | 🔴 Crítica |
| 2.3 | ML | **SPEC-015** — Ensemble mortalidad (DecisionTree + LR + GB) | `dmart-shared/src/ml_ensemble.rs` | Accuracy > 92 % + AUROC con IC 95 % sobre split temporal | 🟠 Alta |
| 2.4 | Stats | **Calibración y métricas clínicas**: AUROC, Hosmer-Lemeshow, calibration plot (predicha vs observada) | `dmart-server/src/validation.rs` | Métricas calculadas y exportables | 🟠 Alta |
| 2.5 | ML | **SPEC-016** — A/B testing framework (split por paciente/tenant) | `dmart-server/src/ml_ab.rs` | Swap con significancia estadística | 🟡 Media |
| 2.6 | ML | **SPEC-017** — SHAP real (sin heurística top-3) | `dmart-shared/src/ml_explain.rs` | Explicación por predicción reproducible | 🟡 Media |
| 2.7 | ML | **SPEC-018** — Retraining + drift detection | `scripts/ml_retrain.rs` | Drift alerta < 24 h; modelo auto-actualizado | 🟡 Media |
| 2.8 | Clínica | **Terminología LOINC/SNOMED/ICD-10** + mapa desde mediciones | `dmart-server/src/api/`, `migrations/` | Mapeo medida→LOINC en API | 🟡 Media |
| 2.9 | Ética | **Declaración ética / consentimiento** para el dataset | `docs/compliance/LEGAL.md` | Sección completa | 🟠 Alta |
| 2.10 | Clínica | **Discusión vs Knaus y otros scores** (contexto literario) | informe/artículo | Sección de discusión redactada | 🟡 Media |
| 2.11 | Reproducibilidad | Método reproducible del piloto **R1–R8** (uptime, alerts, backups, escalas) | `scripts/bench.sh`, `tests/load/benchlat.mjs`, informe | Capítulo "resultados del piloto" con evidencia | 🟡 Media |
| 2.12 | Entrega | Nueva edición del PDF usando la plantilla (`docs/templates/`) con F2 | `Informe_Final_Tecnico_dMart_UCI.pdf` | PDF v2 actualizado y re-renderizado | 🟡 Media |

### Criterios de éxito
- **AUROC mortalidad > 0.90**, **MAPE LOS < 20 %**, ambos con IC 95 % y n reportado.
- Piloto R1–R8 documentado con evidencia medible (sin falsas alarmas/semana, etc.).

---

## Fase 3 — Seguridad empresarial, SRE y backlog operativo 🟡 (noviembre – diciembre 2026)

### Objetivo
Cierra D4→100 y D5→100 (seguridad empresarial + operación con SLAs) y completa las
pantallas que hoy solo existen en API.

### Tareas

| # | Área | Tarea | Archivos | Criterio de éxito | Prioridad |
|---|------|-------|----------|-------------------|-----------|
| 3.1 | Supply chain | **SBOM + firma de imágenes (cosign) + provenance SLSA nivel 2** | CI, `scripts/`, `helm/` | SBOM publicada + imágenes firmadas | 🔴 Crítica |
| 3.2 | Secrets | **HashiCorp Vault** (o external-secrets) en k8s; cero secretos en repo/env | `helm/dmart/`, `.github/` | `grep -r secret` en infra sin credenciales | 🟠 Alta |
| 3.3 | Red | **Zero-trust interno**: mTLS entre pods + NetworkPolicy default-deny | `helm/dmart/`, `cert-manager` | Tráfico interno cifrado | 🟠 Alta |
| 3.4 | Seguridad | **Pen-test de alcance** (firma externa; OWASP ZAP en CI como mínimo) | `.github/workflows/`, reporte | Reporte + correcciones trazadas | 🟠 Alta |
| 3.5 | SRE | **SLOs + error budgets**: SLI/SLO (latencia, disponibilidad, freshness SSE), dashboards + routing alertmanager | `grafana/`, `observability.rs` | SLOs publicados y medidos | 🟠 Alta |
| 3.6 | Capacidad | **Autoscaling validado**: load k6 al límite + HPA con métricas reales + informe | `tests/load/`, `helm/dmart/` | Curva de latencia documentada | 🟡 Media |
| 3.7 | DR | **DR game anual** (SPEC-024 + drill en entorno real) | `scripts/dr_drill.sh` | RPO<1h, RTO<4h probado + log | 🟠 Alta |
| 3.8 | Auditoría | **Auditoría inmutable**: retención 6 años, WORM (firma por lote), export de lectura | `audit.rs`, `migrations/` | Log firmado e inmutable | 🟠 Alta |
| 3.9 | Notificaciones | **SPEC-013** — Web Push (VAPID) para alertas clínicas | `dmart-app/`, `realtime.rs` | Suscripción + notificación push en vivo | 🟡 Media |
| 3.10 | Operativo | **UI Dispositivos/Monitores** (`/devices`) | `dmart-app/src/pages/devices.rs` | CRUD dispositivos + estado en dashboard | 🟡 Media |
| 3.11 | Calidad | **UI Data-quality** (`/data-quality`) | `dmart-app/src/pages/data_quality.rs` | Gaps/faults visibles (SPEC-031 ingest) | 🟡 Media |
| 3.12 | Clínica | **UI Alertas / Escalamiento** (`/escalation`) | `dmart-app/src/pages/`, `api/escalation.rs` | Reglas de escalamiento gestionables | 🟡 Media |
| 3.13 | Ops | **Pestaña Auditoría HIPAA en Admin** + cableado `uci_stats` | `dmart-app/src/pages/admin.rs`, `api/stats.rs` | Logs de auditoría navegables desde UI | 🟡 Media |
| 3.14 | UX | **Dashboard 2.0** (KPIs + ML + SSO de alertas) | `dmart-app/src/pages/dashboard.rs` | 6 KPIs + salud del sistema | 🟢 Baja |
| 3.15 | Soporte | **SPEC-044 — Support Console (Consola Técnica)**: interfaz en `/admin/soporte` (roles Admin/Soporte) que mide **cada subsistema** en vivo — DB, HL7/ingest, SSE/realtime, ML, monitores, auditoría, backups — y ofrece **acciones de corrección** sin acceso a CLI/SSH | `dmart-server/src/api/support.rs`, `observability.rs`, `dmart-app/src/pages/support.rs` | Cada subsistema con estado, latencia y errores visibles; acciones auditable e idempotentes | 🔴 Crítica |
| 3.16 | Soporte | **Correcciones operativas desde la consola**: reintentar ingest, reset circuit breaker (SPEC-031), trigger de backup, retención de auditoría on-demand, swap de modelo ML, verificación de fingerprints | `api/support.rs`, `ingest/`, `retention.rs`, `ml_serving.rs` | Cada acción registrada en auditoría (usuario+IP+resultado) | 🟠 Alta |
| 3.17 | Soporte | **Panel de diagnóstico por subsistema** con SLI medibles (latencia p95, error rate, freshness SSE, gap/fault ingest) y **minutes del problema → acción sugerida** | `api/support.rs`, `grafana/` | Runbook guiado en la pantalla (zero tacos de conocimiento) | 🟠 Alta |
| 3.18 | Soporte | **Self-healing opcional**: reintentos y failovers automáticos en ingest/SSE con notificación; la consola muestra historial de autosnapshot | `ingest/`, `realtime.rs`, `audit.rs` | Automático sin pérdida de datos; todo trazable | 🟡 Media |

### Criterios de éxito
- 0 críticos en audit + pen-test limpio; imágenes firmadas con SBOM.
- SLOs 99.5 % publicados; DR probado con tiempos medidos.
- Re-correr `cargo test -p dmart-server --test api_tests` tras cada tarea.

---

## Fase 4 — Regulatorio, producto, negocio y cierre 🟡 (diciembre 2026)

### Objetivo
Cierra D6→100 y D7→100: convierte el software en **producto vendible/licenciable**
y cierra el trimestre con **tag v1.0.0**.

### Tareas

| # | Área | Tarea | Archivos | Criterio de éxito | Prioridad |
|---|------|-------|----------|-------------------|-----------|
| 4.1 | Médico | **Marco de software médico**: IEC 62304 (clase II), ISO 14971 risk mgmt, IEC 62366 usabilidad | `docs/compliance/` | Matriz de riesgo firmada + plan de seguridad | 🔴 Crítica |
| 4.2 | Regulatorio | **Auditoría externa**: SOC 2 Tipo I + ISO 27001 (evidence pack → evidencia auditada), LOPDPPD VE + DPA | `docs/compliance/` | Certificados + informe de auditoría | 🔴 Crítica |
| 4.3 | Licencias | **Módulo `licensing`**: claves/entitlements por UCI/cama, off-line renew, firmado, sin bypass | `dmart-server/src/api/licensing.rs`, `migrations/` | Licencia revocable y auditable | 🔴 Crítica |
| 4.4 | Billing | **Billing de suscripción**: precio por cama/año ($100–300), factura, ciclo, dashboard comercial | `dmart-server/src/api/billing.rs`, app | Simulación de factura + dashboard | 🟠 Alta |
| 4.5 | Telemetría | **Telemetría de producto** (opt-in, sin PHI): uso de módulos, errores, velocidad | `observability.rs`, `metrics.rs` | Dashboard interno de producto | 🟡 Media |
| 4.6 | i18n | **i18n/l10n**: strings a `l10n/`, ES/EN, multibyte, fechas por región | `dmart-app/` | Cambio de idioma completo | 🟡 Media |
| 4.7 | Comercial | **Contrato + EULA + DPA + SLA 24/7** y portal de soporte | `docs/compliance/`, repo | Paquete de documentos listos para firma | 🔴 Crítica |
| 4.8 | Marketing | **Demo pública** (sandbox online con datos sintéticos) + landing técnica | `docs/`, `docker-compose.demo.yml` | Demo funcional pública | 🟢 Baja |
| 4.9 | Release | **Tag v1.0.0** + CHANGELOG final + README/roadmap actualizados | repo | Release formal v1.0.0 | 🔴 Crítica |
| 4.10 | Retro | Retrospectiva trimestre + siguiente plan | `roadmapFinal.md` → v2027 | Decisiones documentadas | 🟡 Media |

> ⚙️ **Acuerdo vigente (2026-09-13):** Fase 8 (SPEC-021–026: Helm/GitOps/
> cluster/DR/multi-tenancy/blue-green infraestructura) queda **congelada hasta
> firmar contrato o piloto hospitalario real** → no se amplía en este trimestre;
> el DR de 3.7 conserva el scope del acuerdo.

### Criterios de éxito
- Modelo versionado + rollback; drift detectado automáticamente.
- Licencias + billing operativos; paquete comercial y demo pública.
- Tag `v1.0.0` con changelog y artefactos WASM optimizados.

---

## Reglas de ejecución (invariantes)

- **SDD estricto**: toda feature nace de una spec en `specs/` con criterios Gherkin
  antes de código. Nuevas: **SPEC-036** LOS-NN, 037 mortality-NN, 038 licensing,
  039 billing, 040 telemetry, 041 i18n, 042 medical-device-docs, 043 sre-slo,
  **044 support-console** (soporte técnico en modo administrador).
- **Testing acotado** (¡no compilar WASM/fuzz completo!): siempre
  `cargo test -p dmart-server --test api_tests --test hl7_integration --lib`.
- **Gates por fase**: `cargo fmt --check`, `clippy -D warnings`, tests verdes, `trunk build --release`.
- **Commits conventional** (feat:/fix:/test:/docs:) y **nunca commitear sin pedido explícito**.
- Cambios **aditivos**: no romper contratos API/modelos; conservar `data/dmart.db`.
- **NN en el servidor, no en WASM**: inferencia con `candle`/`ort` en backend; el
  bundle WASM sigue < 1MB (clave de SPEC-011).

---

## Fuera de alcance del trimestre

- Fase 8 completa (Helm/GitOps/cluster/DR de infraestructura) — congelada hasta contrato.
- Nuevos hardware/sensores distintos del pipeline HL7 ya integrado.
- Escalado multi-nube/multi-región hasta que exista el piloto hospitalario real.

---

## Métricas objetivo del 100/100 (a fin de trimestre)

| Métrica | Objetivo |
|---------|----------|
| Specs implementadas | 44/44 (001–035 + 036…044) |
| AUROC mortalidad | > 0.90 con IC 95 % |
| Error de LOS (MAPE) | < 20 % (±24h binned) |
| Cobertura HL7/clínica | > 90 % (ya) + feature store reproducible |
| Releases semver | tags automáticos desde CI |
| SLOs | 99.5 % disponible · p95 < 100ms API · alertas < 1min |
| Seguridad | 0 críticos audit · pen-test limpio · SBOM + firma cosign |
| Certificaciones | SOC 2 · ISO 27001 · IEC 62304 · LOPDPPD |
| Negocio | licencia + billing + EULA/DPA + soporte 24/7 |

---

## Referencias rápidas

| Documento | Ubicación |
|-----------|-----------|
| Changelog | `CHANGELOG.md` |
| Arquitectura / ADR | `docs/ARQUITECTURA.md`, `docs/ADR.md` |
| API REST | `docs/API.md` |
| Referencias clínicas | `docs/APACHE_II.md`, `docs/GCS.md`, `specs/028-...` |
| Specs SDD | `specs/` (001–035 + TEMPLATE + 036…043 en backlog) |
| Compliance | `docs/compliance/` (CONTROL_CATALOG, DATA_FLOWS, LEGAL, INCIDENTS) |
| Plantilla informe | `docs/templates/plantilla_informe_tecnico.html` |