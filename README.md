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
[![Tests](https://img.shields.io/badge/Tests-144%20verdes-10B981)](.#testing)
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

## 🎯 Roadmap entregado (SPEC-001 → SPEC-052)

Las **52 specs** del roadmap están completadas. Esta es la hoja de ruta que cerramos:

| Fase | Alcanzada | Qué se entregó |
|------|-----------|----------------|
| **1 · Fundaciones** | SPEC-001…008 | Fix WASM, auth hardening, observabilidad, Docker multistage, CI, seguridad |
| **2 · Interop + ML** | SPEC-009…038 | FHIR R4, HL7 integración, ML ensemble, LOS-NN, early warning, escalamiento |
| **3 · Producción** | SPEC-039…043 | Helm HA, GitOps ArgoCD/Flux, cluster SurrealDB, DR, multi-tenancy, SBOM/SLSA |
| **4 · Operación** | SPEC-044…052 | Soporte/RBAC, auditoría WORM, SLO/SLI, SLI streaming, **Web Push VAPID** |

> 🏷️ Tag de cierre: **`roadmap-final-SPEC-052`** · Ultimo commit: `40d213b`

---

## 🧩 Capacidades clínicas

| Dominio | Implementación |
|---------|----------------|
| **Pacientes** | CRUD, ingresos/egresos con desenlace, historial longitudinal con timeline |
| **Monitores de cama** | HL7 v2 `ORU^R01` + **MLLP (TCP)** y **MQTT** — drivers Mindray, Philips, genéricos; backpressure |
| **Severidad** | **APACHE II**, **GCS** animado, **NEWS2**, **SOFA**, **SAPS III** — validación clínica |
| **Mortalidad** | Riesgo hospitalario (fórmula APACHE II) + **ensemble de ML** (DecisionTree/LR/GBM) |
| **Estancia (LOS)** | Red neuronal **MLP/LSTM** (candle, feature-gated) + predictor stub sin dependencias |
| **Early Warning** | **EWS streaming**: NEWS2/Apache/SOFA → **SSE** en tiempo real por cama |
| **Prevención de esquirlas** | SLI/SLO + error budgets + **alerta de escalamiento** |
| **Interoperabilidad** | **FHIR R4**: Patient, Observation (LOINC), Condition (CIE-10), DiagnosticReport, Bundle |
| **Exportación** | CSV / PDF por paciente, con **fingerprint de integridad** (SPEC-029) |

---

## 🔒 Seguridad grado hospitalario

| Pilar | Implementación |
|-------|----------------|
| **AuthN** | **Argon2id**, JWT **HS256 revocable** (access 15 min / refresh 7 días), **MFA TOTP** RFC 6238 |
| **AuthZ** | **RBAC granular**: `Admin · Médico · Enfermero · Viewer · Soporte` — middleware `require_role!` |
| **Cifrado** | **ChaCha20-Poly1305**, secretos en memoria con **zeroize** |
| **Auditoría WORM** | Log inmutable con **SHA-256 encadenado + lotes firmados**, retención **6 años** |
| **Web Push** | **VAPID** real (RFC 8292) para notificaciones del navegador (SPEC-052) |
| **FW defensivo** | Rate limit por IP, login throttling, HSTS, CSP, sanitización, CORS estricto |
| **SBOM/SLSA** itoría** | Log inmutable **WORM** con cadena + lotes firmados (SPEC-049), retención 6 años |
| **Web Push** | VAPID (RFC 8292) + VAPID JWT firmado (SPEC-052) |
| **Hardening** | Rate limiting por IP, login throttle, HSTS, CSP, sanitización HTML/SQL, CORS estricto |

---

## 🧱 Arquitectura

```
dmart/
├─ dmart-shared/   # Modelos, escalas clínicas, validación, ML (DecisionTree/Ensemble/LOS-NN)
├─ dmart-server/   # Axum API · auth/RBAC · HL7+MLLP/MQTT · FHIR R4 · auditoría WORM
│  ├─ hl7/         #   Parser ORU^R01 · MLLP framing · ingest HL7 v2
│  ├─ api/         #   flags · versioning · push · ml · rbac · versionado
│  ├─ migrations/  #   SurrealQL versionado (DMART_001…049)
│  └─ fuzz/        #   cargo-fuzz (json, hl7, scales)
├─ dmart-app/      # Frontend Leptos/WASM (PWA offline, Service Worker, Web Push)
├─ specs/          # Spec-Driven Development (SPEC-001…052)
└─ docs/           # API · ADR · compliance (HIPAA/HITRUST/ISO)
```

**Runtime:**
- **Single binary** — sin procesos externos obligatorios (SurrealKV embebido)
- **SurrealDB** con `surrealkv` (DB embebida) y modo Docker/HA con migraciones versionadas
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

### Tests y calidad

```bash
# Gate rápido (lib + HL7 + contract) — ~15 s, sin build WASM
cargo test -p dmart-server --test api_tests --test hl7_integration

# Suites individuales
cargo test -p dmart-server --lib                 # 104 verdes
cargo test -p dmart-server --test contract_tests # 8 verdes
cargo llvm-cov -p dmart-server --lib --test api_tests   # cobertura
```

---

## 📚 Documentación

- [roadmapFinal.md](roadmapFinal.md) — Plan de fases y hitos
- [CHANGELOG.md](CHANGELOG.md) — Historial de versiones (conventional commits)
- [specs/](specs/) — Especificaciones SDD (SPEC-001…052)
- [docs/ADR.md](docs/ADR.md) — Architecture Decision Records
- [docs/API.md](docs/API.md) — Endpoints REST + FHIR + WebSocket

---

## 📊 Cobertura

**SPEC-027** exige coverage global **≥ 60 %** en el server (lib + bin). El gate de CI lo verifica con `cargo llvm-cov`. Módulos protegidos (ML, HL7, seguridad, auditoría, escalamiento, RBAC) tienen requisitos de cobertura por fichero.

---

## 🤝 Contribución

1. Lee [roadmapFinal.md](roadmapFinal.md) y elige una SPEC pendiente
2. Abre issue con la especificación (formato Gherkin en `specs/`)
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
