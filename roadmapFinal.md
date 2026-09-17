# 🗺️ Roadmap Final dMart UCI — Trimestre final 2026

**Documento único de planificación.** Reemplaza a `ROADMAP.md` y `updateIO.md`
(eliminados el 2026-09-17). Cubre la **próxima actualización** en **4 fases** para
el **trimestre final de 2026 (octubre – diciembre)**.

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

## Objetivo del trimestre final

| # | Objetivo | Fase |
|---|----------|------|
| 1 | Cerrar bugs conocidos y deuda técnica pendiente (SPEC-010/011) | F1 |
| 2 | Validez clínica estadística y publicación (artículo científico) | F2 |
| 3 | Terminar el backlog operativo/UX restante | F3 |
| 4 | ML v2 y cierre del trimestre (tag v1.0.0 + retro) | F4 |

---

## Fase 1 — Estabilidad y cierre de deuda técnica 🔄 (octubre 2026)

### Objetivo
Sistema estable, sin pantallas rotas y con pipeline técnico cerrado.

### Tareas

| # | Área | Tarea | Archivos | Criterio de éxito | Prioridad |
|---|------|-------|----------|-------------------|-----------|
| 1.1 | Frontend/Bug | Fix **`/perfil` renderiza en blanco** (última pantalla rota detectada) | `dmart-app/src/pages/perfil.rs`, `stores/*` | `/perfil` muestra cambio de contraseña y datos de sesión | 🔴 Crítica |
| 1.2 | Auth/UX | **Reto TOTP en login web** (`login.rs`) para poder activar MFA desde UI sin quedarse fuera | `dmart-app/src/pages/login.rs`, `api.rs` | Flujo login → challenge TOTP → sesión; activar MFA desde `/perfil` verificado | 🔴 Crítica |
| 1.3 | Release | **SPEC-010** — Semantic versioning + git-cliff: tags `vX.Y.Z` + flujo release | `.github/workflows/release.yml`, `cliff.toml`, `CHANGELOG.md` | Tag `v0.9.0` creado desde CI + changelog automático | 🟠 Alta |
| 1.4 | Build | **SPEC-011** — `wasm-opt` en CI/Trunk (WASM 2.5MB → <1MB) | `.github/workflows/ci.yml`, `dmart-app/Trunk.toml` | Artifact WASM optimizado publicado | 🟠 Alta |
| 1.5 | QA | Smoke test completo tras F1 (login, MFA, perfil, pacientes, admin) | `tests/e2e/*.spec.ts` | Suite E2E verde en CI | 🟠 Alta |

### Criterios de éxito
- `cargo test -p dmart-server --test api_tests --test hl7_integration` verde (~15s).
- `trunk build --release` + clippy `-D warnings` + `fmt --check` sin errores.
- 0 pantallas en blanco recorriendo `/login → / → /patients → /perfil → /admin`.

---

## Fase 2 — Validez clínica y publicación 🟡 (octubre – noviembre 2026)

### Objetivo
Subir el nivel del informe ya entregado a **artículo científico** con evidencia estadística.

### Tareas

| # | Área | Tarea | Archivos | Criterio de éxito | Prioridad |
|---|------|-------|----------|-------------------|-----------|
| 2.1 | Clínica/Stats | **Validación APACHE II predicha vs observada**: AUROC, Hosmer-Lemeshow, IC 95 %, calibration plot | `dmart-server/src/validation.rs`, scripts | Métricas calculadas sobre piloto con 95 % CI | 🟠 Alta |
| 2.2 | Ética | **Declaración ética / consentimiento** para el dataset del informe | `docs/compliance/LEGAL.md` (o en informe) | Sección completa en informe | 🟠 Alta |
| 2.3 | Clínica | **Discusión vs Knaus y otros scores** (contexto literario) | informe/artículo | Sección de discusión redactada | 🟡 Media |
| 2.4 | Reproducibilidad | Método reproducible del piloto **R1–R8** (uptime, alerts, backups, escalas) | `scripts/bench.sh`, `tests/load/benchlat.mjs`, informe | Capítulo "resultados del piloto" con evidencia | 🟡 Media |
| 2.5 | Entrega | Nueva edición del PDF usando la plantilla (`docs/templates/`) con F2 | `Informe_Final_Tecnico_dMart_UCI.pdf` | PDF v2 actualizado y re-renderizado | 🟡 Media |

### Criterios de éxito
- AUROC y Hosmer-Lemeshow reportados con IС y n.
- Piloto R1–R8 documentado con evidencia medible (sin falsas alarmas/semana, etc.).

---

## Fase 3 — Backlog operativo y UX restante 🟡 (noviembre – diciembre 2026)

### Objetivo
Completar las pantallas y capacidades que hoy solo existen en API o quedaron en backlog.

### Tareas

| # | Área | Tarea | Archivos | Criterio de éxito | Prioridad |
|---|------|-------|----------|-------------------|-----------|
| 3.1 | Notificaciones | **SPEC-013** — Web Push (VAPID) para alertas clínicas | `dmart-app/`, `dmart-server/src/realtime.rs` | Suscripción + notificación push en vivo | 🟡 Media |
| 3.2 | Operativo | **UI Dispositivos/Monitores** (`/devices`) | `dmart-app/src/pages/devices.rs` | CRUD dispositivos con estado en dashboard | 🟡 Media |
| 3.3 | Calidad | **UI Data-quality** (`/data-quality`) | `dmart-app/src/pages/data_quality.rs` | Gaps/faults visibles (SPEC-031 ingest) | 🟡 Media |
| 3.4 | Clínica | **UI Alertas / Escalamiento** (`/escalation`) | `dmart-app/src/pages/`, `api/escalation.rs` | Reglas de escalamiento gestionables | 🟡 Media |
| 3.5 | Ops | **Pestaña Auditoría HIPAA en Admin** + cableado `uci_stats` | `dmart-app/src/pages/admin.rs`, `api/stats.rs` | Logs de auditoría navegables desde UI | 🟡 Media |
| 3.6 | UX | **Dashboard 2.0** (KPIs + ML + SSO de alertas) | `dmart-app/src/pages/dashboard.rs` | 6 KPIs + salud del sistema | 🟢 Baja |

### Criterios de éxito
- Re-correr `cargo test -p dmart-server --test api_tests` tras cada tarea.
- Verificación manual end-to-end de cada pantalla (wash de la F1).

---

## Fase 4 — ML v2 y cierre del trimestre 🟡 (diciembre 2026)

### Objetivo
Evolucionar el ML piloto y cerrar el trimestre con tag **v1.0.0** y métricas.

### Tareas

| # | Área | Tarea | Archivos | Criterio de éxito | Prioridad |
|---|------|-------|----------|-------------------|-----------|
| 4.1 | ML | **SPEC-014** — Feature Store versionado | `dmart-shared/src/ml_features.rs` | Features reproducibles entre reinicios | 🟠 Alta |
| 4.2 | ML | **SPEC-015** — Ensemble (DecisionTree + LR + XGBoost est. linfa) | `dmart-shared/src/ml_ensemble.rs` | Accuracy > 92 % (vs 85–90 % actual) | 🟠 Alta |
| 4.3 | ML | **SPEC-018** — Retraining + drift detection | `scripts/ml_retrain.rs` | Drift alerta < 24 h | 🟡 Media |
| 4.4 | Release | **Tag v1.0.0** + CHANGELOG final + README/roadmap actualizados | repo | Release formal v1.0.0 | 🟠 Alta |
| 4.5 | Retro | Retrospectiva trimestre + siguiente plan | `roadmapFinal.md` → v2027 | Decisiones documentadas | 🟡 Media |

> ⚙️ **Acuerdo vigente (2026-09-13):** Fase 8 (SPEC-021–026: Helm/GitOps/
> cluster/DR/multi-tenancy/blue-green) queda **congelada hasta firmar contrato o
> piloto hospitalario real** → **no se toca en este trimestre**.

### Criterios de éxito
- Modelo versionado + rollback; drift detectado automáticamente.
- Tag `v1.0.0` con changelog y artefactos WASM optimizados.

---

## Reglas de ejecución (invariantes)

- **SDD**: toda feature nueva nace de una spec en `specs/` con criterios Gherkin antes de código.
- **Testing acotado** (¡no compilar WASM/fuzz!): siempre
  `cargo test -p dmart-server --test api_tests --test hl7_integration --lib`.
- **Gates por fase**: `cargo fmt --check`, `clippy -D warnings`, tests verdes, release compilando.
- **Commits conventional** (feat:/fix:/test:/docs:) y **nunca commitear sin pedido explícito**.
- Cambios **aditivos**: no romper contratos API/modelos; conservar `data/dmart.db`.

---

## Fuera de alcance del trimestre

- Fase 8 completa (k8s/GitOps/cluster) — congelada hasta contrato hospitalario.
- Tele-ICU / similaridad ML adicional (SPEC-033 ya hecha; UI opcional en F3 si sobra tiempo).
- Nuevos hardware/sensores distintos del pipeline HL7 ya integrado.

---

## Referencias rápidas

| Documento | Ubicación |
|-----------|-----------|
| Changelog | `CHANGELOG.md` |
| Arquitectura / ADR | `docs/ARQUITECTURA.md`, `docs/ADR.md` |
| API REST | `docs/API.md` |
| Referencias clínicas | `docs/APACHE_II.md`, `docs/GCS.md`, `specs/028-...` |
| Specs SDD | `specs/` (001–035 + TEMPLATE) |
| Plantilla informe | `docs/templates/plantilla_informe_tecnico.html` |