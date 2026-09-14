# dMart UCI

**Sistema de Gestión de Unidad de Cuidados Intensivos — 100% Rust, WebAssembly, SurrealDB**

[![CI](https://github.com/rooselvelt6/dmart/actions/workflows/ci.yml/badge.svg)](https://github.com/rooselvelt6/dmart/actions/workflows/ci.yml)
[![Rust](https://img.shields.io/badge/Rust-1.98-orange?logo=rust)](https://www.rust-lang.org/)
[![WASM](https://img.shields.io/badge/Frontend-WebAssembly-654FF0?logo=webassembly)](https://webassembly.org/)
[![Leptos](https://img.shields.io/badge/UI-Leptos%200.8-FF4B4B?logo=leptos)](https://leptos.dev/)
[![Axum](https://img.shields.io/badge/Backend-Axum%200.8-99A0AA)](https://github.com/tokio-rs/axum)
[![SurrealDB](https://img.shields.io/badge/DB-SurrealKV-FF00A0?logo=surrealdb)](https://surrealdb.com/)
[![Tests](https://img.shields.io/badge/Tests-146%20passing-10B981)](https://github.com/rooselvelt6/dmart/actions)
[![Coverage](https://img.shields.io/badge/Coverage-65%25-22c55e)](./dmart-server/tests)
[![License](https://img.shields.io/badge/License-MIT-3B82F6)](LICENSE)

---

## Descripción

**dMart UCI** es una plataforma integral de gestión para Unidades de Cuidados Intensivos, construida íntegramente en **Rust** y compilada a **WebAssembly** con **Leptos**. Diseñada para operar en red hospitalaria aislada (offline-first), sin dependencias externas obligatorias.

### Capacidades clínicas

| Dominio | Implementación |
|---------|----------------|
| **Pacientes** | CRUD completo, ingresos/egresos con desenlace, historial longitudinal |
| **Monitores de cama** | Parser HL7 v2 (ORU^R01) + transporte MLLP/MQTT — Mindray, Philips, genéricos |
| **Scores de severidad** | APACHE II (71 pts), GCS, NEWS2, SOFA, SAPS III — rangos clínicos validados |
| **Mortalidad** | Riesgo hospitalario calculado + ML piloto (DecisionTree, ~85–90 % prec.) |
| **Interoperabilidad** | FHIR R4: Patient, Observation (LOINC), Condition (CIE-10), DiagnosticReport, Bundle |
| **Exportación** | CSV / PDF por paciente |
| **Frontend** | PWA offline-capable, Service Worker, glassmorphism, dark/light, responsive |

### Seguridad HIPAA-grade

| Pilar | Detalle |
|-------|---------|
| **Auth** | Argon2id (19 MiB), JWT revocable, MFA TOTP + backup codes |
| **AuthZ** | RBAC: Admin · Médico · Enfermero · Viewer (permisos por ruta) |
| **Cifrado** | ChaCha20-Poly1305 + zeroización de secretos |
| **Auditoría** | Log PHI con retención 6 años |
| **Hardening** | Rate limiting, login throttling, HSTS, sanitización |
| **QA** | cargo-fuzz (3 targets), proptest, E2E Playwright, k6 load |

---

## Arquitectura

```
dmart/
├─ dmart-shared/   # Modelos, escalas clínicas, validación, ML
├─ dmart-server/   # Axum API · auth/RBAC · HL7+MLLP · FHIR · auditoría
│  ├─ hl7/         #   Parser ORU^R01 · MLLP · ingest · MQTT
│  ├─ migrations/  #   SurrealQL versionado
│  └─ fuzz/        #   cargo-fuzz (json, hl7, scales)
├─ dmart-app/      # Frontend Leptos/WASM (PWA, Tailwind)
├─ specs/          # Spec-Driven Development (SPEC-001…031+)
├─ tests/          # E2E (Playwright) · load (k6)
└─ docs/           # ADR · API · APACHE_II · GCS
```

---

## Inicio rápido

### Prerrequisitos
- Rust 1.98+ (`rustup default 1.98`)
- `wasm32-unknown-unknown` target (`rustup target add wasm32-unknown-unknown`)
- `trunk` para dev frontend (`cargo install trunk`)

### Desarrollo
```bash
# Backend
cd dmart-server
cargo run --release

# Frontend (otra terminal)
cd dmart-app
trunk serve --open
```

### Tests
```bash
# Suite completa (lib + bin + integración)
cargo test -p dmart-server

# Gates de calidad
cargo fmt --all -- --check
cargo clippy -p dmart-server --lib --bin dmart-server -- -D warnings
cargo llvm-cov -p dmart-server --lib --test api_tests --test hl7_integration
```

### Docker (producción)
```bash
docker compose -f docker-compose.prod.yml up -d
```

---

## Estado del proyecto (SPEC-Driven Development)

### SPECs completadas — **gate `clippy -D 0/0` + tests verdes**

| SPEC | Tema | Commit | Tests |
|------|------|--------|-------|
| 001–009 | Base: WASM, ML, HL7, Auth, Prometheus, Docker, CI, Alertas, Backup | — | — |
| 010 | **Restore / Disaster Recovery** | `c662e1e` | 3 |
| 011 | **Runbook / On-Call operativo** | `c662e1e` | 2 |
| 012 | Production staging compose | — | — |
| **013** | **FHIR R4 Bundle ingestion** | `5143791` | 4 |
| **014** | **Early-Warning Streaming (EWS)** | `5143791` | 4 |
| 027 | Coverage gate CI (llvm-cov ≥ 60 %) | — | — |
| 028 | Clinical reference vectors (conformance) | — | 38 |
| 029 | Score fingerprint auditability | — | — |
| 030 | Retention / downsampling | — | — |
| **031** | **Bin/lib HL7 dedupe → 0 dead-code** | `bfecaf0` | 63 |

**Métricas globales:** **146 tests** · `clippy -D warnings` = **0/0** (lib + bin) · coverage **65 %**

### Backlog pendiente (14 SPECs)

| Rango | Propuesta inmediata |
|-------|---------------------|
| **015–016** | **Patient Timeline API** + **Clinical Decision Support rules** |
| 017–026 | Core platform: multi-tenancy, tele-ICU, device registry, alert escalation, data quality, cohort extraction, DR drill |
| 032–033 | ML serving ONNX/WASM + Patient similarity engine |

---

## Documentación

- [ROADMAP.md](ROADMAP.md) — Plan de fases y hitos
- [CHANGELOG.md](CHANGELOG.md) — Historial de versiones
- [specs/](specs/) — Especificaciones SDD con criterios Gherkin
- [docs/ADR.md](docs/ADR.md) — Architecture Decision Records
- [docs/API.md](docs/API.md) — Endpoints REST + FHIR

---

## Contribución

1. Lee [ROADMAP.md](ROADMAP.md) y elige una SPEC pendiente
2. Abre issue con la especificación (formato Gherkin en `specs/`)
3. Implementa → tests → `cargo fmt && cargo clippy -D warnings`
4. Actualiza CHANGELOG.md y abre PR

---

## Licencia

**MIT** — Copyright © 2026 · [rooselvelt6](https://github.com/rooselvelt6)

---

<div align="center">

**dMart UCI** — Un solo binario para una UCI completa.  
Hecho con ❤️ y Rust.

</div>