<div align="center">

# 🏥 dMart UCI

**Sistema de Gestión de Unidad de Cuidados Intensivos**
_100% Rust · WebAssembly · SurrealDB — offline-first, grado hospitalario_

<img src="dmart-app/icon.svg" alt="dMart UCI" width="120">

[![CI](https://github.com/rooselvelt6/dmart/actions/workflows/ci.yml/badge.svg)](https://github.com/rooselvelt6/dmart/actions/workflows/ci.yml)
[![Rust](https://img.shields.io/badge/Rust-1.98-orange?logo=rust)](https://www.rust-lang.org/)
[![WASM](https://img.shields.io/badge/Frontend-WebAssembly-654FF0?logo=webassembly)](https://webassembly.org/)
[![Leptos](https://img.shields.io/badge/UI-Leptos%200.8-FF4B4B?logo=leptos)](https://leptos.dev/)
[![Axum](https://img.shields.io/badge/Backend-Axum%200.8-99A0AA)](https://github.com/tokio-rs/axum)
[![SurrealDB](https://img.shields.io/badge/DB-SurrealKV-FF00A0?logo=surrealdb)](https://surrealdb.com/)
[![Tests](https://img.shields.io/badge/Tests-303%20verdes-10B981)](PLAN.md)
[![Coverage](https://img.shields.io/badge/Coverage-%E2%89%A560%25-22c55e)](specs/027-coverage-gate-ci.md)
[![License](https://img.shields.io/badge/License-MIT-3B82F6)](LICENSE)

</div>

---

## ✨ El proyecto en una frase

> Un **solo binario Rust (~8 MB)** que aloja la UCI completa: API REST + WebSocket/SSE,
> parser **HL7 v2 + MLLP/MQTT**, **FHIR R4**, scores clínicos (APACHE II, NEWS2, GCS, SOFA…),
> **ML de mortalidad y estancia**, auditoría inmutable **WORM**, Web Push con VAPID,
> y frontend **PWA (Leptos/WASM)** — operando en red hospitalaria **aislada, sin internet**.

---

## 🎯 Estado del Roadmap (SPEC-001 → SPEC-052)

| Fase | Alcanzada | Qué se entregó |
|------|-----------|----------------|
| **1 · Fundaciones** | ✅ SPEC-001…008 | Fix WASM, auth hardening, observabilidad, CI, seguridad |
| **2 · Interop + ML** | ✅ SPEC-009…038 | FHIR R4, HL7 integración, ML ensemble, LOS-NN, early warning, escalamiento |
| **3 · Producción** | ✅ SPEC-039…043 | Helm HA, GitOps ArgoCD/Flux, cluster SurrealDB, DR, multi-tenancy, SBOM/SLSA |
| **4 · Operación** | ✅ SPEC-044…052 | Soporte/RBAC, auditoría WORM, SLO/SLI, SLI streaming, **Web Push VAPID** |

> 🏷️ Tag de cierre SPEC-052: **`roadmap-final-SPEC-052`** · Último commit: `adf14b9`

---

## 🔒 Seguridad — Estado actual (hardening post-roadmap)

| Pilar | Implementado | Pendiente (ver PLAN.md) |
|-------|--------------|--------------------------|
| **AuthN** | Argon2id, JWT HS256 revocable (access 15 min / refresh 7 días), MFA TOTP RFC 6238 | Zeroize `JWT_SECRET` |
| **AuthZ** | RBAC: `Admin·Médico·Enfermero·Viewer·Soporte` + `require_role!` | RBAC fino + `tenant_id` NOT NULL en todas las entidades (F2.1) |
| **Cifrado reposo** | **AES-256-GCM** `DMART_A2` + auto-detección `DMART_V1` legacy; subclaves derivadas con separación de dominio (HMAC-SHA256, etiquetas `LABEL_PHI`, `LABEL_INDEX`) | `measurements`, `care_plan`, `audit`, `push`, `device_registry`, `reports` + backfill patients |
| **Zeroize** | `MasterKey`, `PhiCipher` | `JWT_SECRET`, `MLLP auth_secret` |
| **Auditoría WORM** | SHA-256 encadenado + lotes firmados HMAC-SHA256; **concurrencia segura** (`tokio::sync::Mutex`) | Cifrado payload WORM manteniendo encadenamiento |
| **MLLP** | TLS 1.3 obligatorio en prod, pinning SHA-256 DER→MSH.3, rate limit por IP, fail-closed | — |
| **Web Push** | VAPID RFC 8292 | — |
| **Hardening** | Rate limit IP, login throttle, HSTS, CSP, sanitización, CORS estricto | Pinning CA salientes (anti-SSRF), errores genéricos en handlers |

---

## 🧩 Capacidades clínicas (sin cambios)

| Dominio | Implementación |
|---------|----------------|
| **Pacientes** | CRUD, ingresos/egresos con desenlace, historial longitudinal con timeline **(PHI cifrado AES-256-GCM + índices ciegos)** |
| **Monitores de cama** | HL7 v2 `ORU^R01` + **MLLP (TCP)** y **MQTT** — drivers Mindray, Philips, genéricos; backpressure |
| **Severidad** | **APACHE II**, **GCS** animado, **NEWS2**, **SOFA**, **SAPS III** — validación clínica |
| **Mortalidad** | Riesgo hospitalario (fórmula APACHE II) + **ensemble de ML** (DecisionTree/LR/GBM) |
| **Estancia (LOS)** | Red neuronal **MLP/LSTM** (candle, feature-gated) + predictor stub sin dependencias |
| **Early Warning** | **EWS streaming**: NEWS2/Apache/SOFA → **SSE** en tiempo real por cama |
| **Prevención de esquirlas** | SLI/SLO + error budgets + **alerta de escalamiento** |
| **Interoperabilidad** | **FHIR R4**: Patient, Observation (LOINC), Condition (CIE-10), DiagnosticReport, Bundle |
| **Exportación** | CSV / PDF por paciente, con **fingerprint de integridad** (SPEC-029) |

---

## 🧱 Arquitectura

```
dmart/
├─ dmart-shared/   # Modelos, escalas clínicas, validación, ML (DecisionTree/Ensemble/LOS-NN)
├─ dmart-server/   # Axum API · auth/RBAC · HL7+MLLP/MQTT · FHIR R4 · auditoría WORM
│  ├─ crypto.rs    #   AES-256-GCM, PhiCipher, subclaves HMAC-SHA256, zeroize
│  ├─ phi_store.rs #   PHI envelope DMART_A2 + blind indexes
│  ├─ audit.rs     #   WORM chain + lotes firmados (tokio::Mutex concurrencia segura)
│  ├─ deployment.rs#   is_production() centralizado, lock tests
│  ├─ hl7/         #   Parser ORU^R01 · MLLP framing · ingest HL7 v2
│  ├─ mllp_tls.rs  #   TLS 1.3 + pinning + tests E2E (8)
│  ├─ api/         #   flags · versioning · push · ml · rbac · versionado
│  ├─ migrations/  #   SurrealQL versionado (050_phi_envelope + pendientes)
│  └─ fuzz/        #   cargo-fuzz (json, hl7, scales)
├─ dmart-app/      # Frontend Leptos/WASM (PWA offline, Service Worker, Web Push)
├─ specs/          # Spec-Driven Development (SPEC-001…052 + hardening)
└─ docs/           # API · ADR · compliance (HIPAA/HITRUST/ISO)
```

**Runtime:**
- **Single binary** — sin procesos externos obligatorios (SurrealKV embebida). **Despliegue nativo single-binary**; ya no hay ruta Docker (eliminado del repo).
- **SurrealDB** con `surrealkv` (DB embebida) y modo HA con migraciones versionadas
- **Hot-reload dev** — `cargo watch` + `trunk serve`
- **Observabilidad** — Prometheus `/metrics`, health endpoint, tracing JSON

---

## 🚀 Inicio rápido

```bash
# Requisitos
rustup default 1.84
rustup target add wasm32-unknown-unknown
cargo install trunk --locked

# Terminal 1 — Backend (puerto 3030)
cd dmart-server && cargo run --release

# Terminal 2 — Frontend (puerto 8080, proxy a 3030)
cd dmart-app && trunk serve --open --port 8080
```

### Tests y calidad (AGENTS.md)

```bash
# Gate rápido (lib + HL7 + contract) — ~15 s, sin build WASM
cargo test -p dmart-server --lib --test api_tests --test hl7_integration

# Suites individuales
cargo test -p dmart-server --lib                    # 188 verdes
cargo test -p dmart-server --test cds_rules         # 4
cargo test -p dmart-server --test ews_streaming     # 4
cargo test -p dmart-server --test data_quality      # 13
cargo test -p dmart-server --test escalation        # 7
cargo test -p dmart-server --test teleicu           # 8

# Cobertura
cargo llvm-cov -p dmart-server --lib --test api_tests
```

---

## 📋 Plan de trabajo actual

Ver **[PLAN.md](PLAN.md)** para el desglose completo priorizado (17 ítems: 6 HIGH, 5 MEDIUM, 6 LOW).

**Próximos hitos HIGH:**
1. Cifrar PHI en `measurements`, `care_plan`, `audit`, `push`, `device_registry`, `reports`
2. Backfill patients legacy (job idempotente)
3. Zeroize `JWT_SECRET` + `MLLP auth_secret`
4. RBAC fino + `tenant_id` NOT NULL
5. Commit + push + **revocar PAT expuesto**

---

## 📚 Documentación

- [CHANGELOG.md](CHANGELOG.md) — Historial de versiones (conventional commits)
- [PLAN.md](PLAN.md) — Plan priorizado de hardening post-roadmap
- [specs/](specs/) — Especificaciones SDD (SPEC-001…052)
- [docs/ADR.md](docs/ADR.md) — Architecture Decision Records
- [docs/API.md](docs/API.md) — Endpoints REST + FHIR + WebSocket

---

## 📊 Cobertura

**SPEC-027** exige coverage global **≥ 60 %** en el server (lib + bin). El gate de CI lo verifica con `cargo llvm-cov`. Módulos protegidos (ML, HL7, seguridad, auditoría, escalamiento, RBAC) tienen requisitos de cobertura por fichero.

---

## 🤝 Contribución

1. Revisa [PLAN.md](PLAN.md) y elige un ítem HIGH
2. Abre issue con la especificación
3. Implementa → tests → `cargo fmt && cargo clippy -D warnings`
4. Actualiza CHANGELOG.md y abre PR

---

## 📜 Licencia

**MIT** — Copyright © 2026 · [rooselvelt6](https://github.com/rooselvelt6)

---

<div align="center">

**dMart UCI** — Un solo binario para una UCI completa.
Hecho con ❤️ y Rust.

</div>