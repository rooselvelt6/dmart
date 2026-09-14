# dMart UCI

<p align="center">
  <img src="dmart-app/icon.svg" alt="dMart UCI" width="140" height="140">
</p>

<p align="center">
  <strong>Sistema de Gestión de Unidad de Cuidados Intensivos — 100% Rust, WebAssembly, SurrealDB</strong>
</p>

<p align="center">
  <a href="https://github.com/rooselvelt6/dmart/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/rooselvelt6/dmart/actions/workflows/ci.yml/badge.svg"></a>
  <a href="https://www.rust-lang.org/"><img alt="Rust" src="https://img.shields.io/badge/Rust-1.98-orange?logo=rust"></a>
  <a href="https://webassembly.org/"><img alt="WebAssembly" src="https://img.shields.io/badge/Frontend-WebAssembly-654FF0?logo=webassembly"></a>
  <a href="https://leptos.dev/"><img alt="Leptos" src="https://img.shields.io/badge/UI-Leptos%200.8-FF4B4B?logo=leptos"></a>
  <a href="https://github.com/tokio-rs/axum"><img alt="Axum" src="https://img.shields.io/badge/Backend-Axum%200.8-99A0AA"></a>
  <a href="https://surrealdb.com/"><img alt="SurrealDB" src="https://img.shields.io/badge/DB-SurrealKV-FF00A0?logo=surrealdb"></a>
  <img alt="Tests" src="https://img.shields.io/badge/Tests-146%20passing-10B981">
  <img alt="Coverage" src="https://img.shields.io/badge/Coverage-65%25-22c55e">
  <a href="LICENSE"><img alt="License" src="https://img.shields.io/badge/License-MIT-3B82F6"></a>
</p>

---

## Descripción

**dMart UCI** es una plataforma integral de gestión para Unidades de Cuidados Intensivos, construida íntegramente en **Rust** y compilada a **WebAssembly** con **Leptos**. Diseñada para operar en red hospitalaria aislada (**offline-first**), sin dependencias externas obligatorias. Un solo binario (`dmart-server` ~8 MB) que incluye API REST, WebSocket/SSE, base de datos embebida (SurrealKV), parser HL7 v2 + MLLP, motor de scores clínicos y frontend PWA.

### Capacidades clínicas validadas

| Dominio | Implementación |
|---------|----------------|
| **Pacientes** | CRUD completo, ingresos/egresos con desenlace (Mejorado/Trasladado/Fallecido), historial longitudinal con timeline |
| **Monitores de cama** | Parser HL7 v2 (ORU^R01) + transporte **MLLP** (TCP) y **MQTT** — drivers Mindray, Philips, genéricos; backpressure SPEC-031 |
| **Scores de severidad** | **APACHE II** (71 pts: APS + Edad + Crónicos), **GCS** animado, **NEWS2**, **SOFA**, **SAPS III** — rangos clínicos con validación y desglose |
| **Mortalidad** | Riesgo hospitalario calculado (fórmula APACHE II) + ML piloto (DecisionTree, ~85–90 % prec.) |
| **Interoperabilidad** | **FHIR R4** nativo: Patient, Observation (LOINC 8867-4/9279-1/2708-6/8310-5), Condition (CIE-10), DiagnosticReport, Bundle transaction/batch/collection |
| **Exportación** | CSV / PDF por paciente con fingerprint SPEC-029 |
| **Frontend PWA** | Offline-capable (Service Worker), glassmorphism, dark/light, responsive, animaciones de severidad, QR FHIR |

### Seguridad grado hospitalario (HIPAA)

| Pilar | Implementación |
|-------|----------------|
| **Autenticación** | Argon2id (19 MiB, 3 passes, 4 lanes), JWT HS256 revocable (access 15 min / refresh 7 días), **MFA TOTP** RFC 6238 + backup codes |
| **Autorización** | **RBAC** granular: `Admin · Médico · Enfermero · Viewer` — permisos por ruta (`rbac.rs`), middleware `require_role!` |
| **Cifrado** | **ChaCha20-Poly1305** (XChaCha20-Poly1305-IETF) + zeroización de secretos en memoria (`zeroize`) |
| **Auditoría PHI** | Log inmutable de acceso con retención **6 años**, fingerprint SHA-256 por registro (SPEC-029) |
| **Hardening** | Rate limiting por IP real (token bucket), login throttling exponencial, HSTS, CSP, sanitización HTML/SQL, CORS estricto |
| **QA defensivo** | **cargo-fuzz** (3 targets: json, hl7, scales), **proptest** (bounds, monotonicidad, robustez), E2E Playwright (8 suites), k6 load (1000 VU) |

---

## Arquitectura

```
dmart/
├─ dmart-shared/   # Modelos, escalas clínicas, validación, ML (DecisionTree)
├─ dmart-server/   # Axum API · auth/RBAC · HL7+MLLP/MQTT · FHIR R4 · auditoría
│  ├─ hl7/         #   Parser ORU^R01 · MLLP framer · ingest · MQTT client
│  ├─ fhir_bundle/ #   Bundle R4 parser + conversión LOINC→VitalsMessage
│  ├─ ews_stream/  #   Early-Warning Streaming (NEWS2/Apache/SOFA → SSE)
│  ├─ migrations/  #   SurrealQL versionado (DMART_001..)
│  └─ fuzz/        #   cargo-fuzz (json, hl7, scales)
├─ dmart-app/      # Frontend Leptos/WASM (PWA, Tailwind, Service Worker)
├─ specs/          # Spec-Driven Development (SPEC-001…031+)
├─ tests/          # E2E (Playwright) · load (k6)
└─ docs/           # ADR · API · APACHE_II · GCS
```

**Características de runtime:**
- **Single binary** (~8 MB release) — `dmart-server` incluye todo: API, DB, HL7, FHIR, métricas
- **SurrealKV embebido** — sin proceso externo, ACID, consultas SQL-like
- **Hot-reload dev** — `cargo watch` + `trunk serve` para backend/frontend simultáneo
- **Observabilidad nativa** — Prometheus `/metrics`, health `/obs/health`, tracing structured JSON

---

## Inicio rápido

### Prerrequisitos
- **Rust 1.98+** (`rustup default 1.98`)
- Target WASM: `rustup target add wasm32-unknown-unknown`
- Frontend dev: `cargo install trunk --locked`

### Desarrollo local
```bash
# Terminal 1 — Backend (puerto 3030)
cd dmart-server
cargo run --release

# Terminal 2 — Frontend (puerto 8080, proxy a 3030)
cd dmart-app
trunk serve --open --port 8080
```

### Tests y gates de calidad
```bash
# Suite completa (lib + bin + integración)
cargo test -p dmart-server

# Formato + clippy estricto (gate CI)
cargo fmt --all -- --check
cargo clippy -p dmart-server --lib --bin dmart-server -- -D warnings

# Cobertura (SPEC-027: ≥ 60 % global)
cargo llvm-cov -p dmart-server --lib --test api_tests --test hl7_integration
```

### Docker (producción)
```bash
# Build multi-stage (builder + runtime distroless)
docker compose -f docker-compose.prod.yml up -d --build

# Health checks
curl -f http://localhost:3030/obs/health
```

---

## Estado del proyecto — Spec-Driven Development

### SPECs completadas — **gate `clippy -D warnings` = 0/0 (lib + bin) + tests verdes**

| SPEC | Tema | Commit | Tests | Gate |
|------|------|--------|-------|------|
| 001 | Fix WASM build | — | — | ✅ |
| 002 | ML model persistence | — | — | ✅ |
| 003 | HL7 MLLP integration tests | — | 32 | ✅ |
| 004 | Auth/AuthZ hardening (JWT, MFA, RBAC) | — | 31 | ✅ |
| 005 | Prometheus/Grafana | — | — | ✅ |
| 006 | Docker multistage staging | — | — | ✅ |
| 007 | CI pipeline completo | — | — | ✅ |
| 008 | Alertas operativas | — | — | ✅ |
| 009 | Backup automático | — | — | ✅ |
| **010** | **Restore / Disaster Recovery** | `c662e1e` | 3 | 0/0 |
| **011** | **Runbook / On-Call operativo** | `c662e1e` | 2 | 0/0 |
| 012 | Production staging compose | — | — | ✅ |
| **013** | **FHIR R4 Bundle ingestion** | `5143791` | 4 | 0/0 |
| **014** | **Early-Warning Streaming (EWS)** | `5143791` | 4 | 0/0 |
| 027 | Coverage gate CI (llvm-cov ≥ 60 %) | — | — | ✅ |
| 028 | Clinical reference vectors (conformance) | — | 38 | ✅ |
| 029 | Score fingerprint auditability | — | — | ✅ |
| 030 | Retention / downsampling | — | — | ✅ |
| **031** | **Bin/lib HL7 dedupe → 0 dead-code** | `bfecaf0` | 63 | 0/0 |

**Métricas globales:** **146 tests** pasando · `clippy -D warnings` = **0/0** (lib + bin) · coverage **65 %** (SPEC-027)

### Backlog pendiente (14 SPECs numeradas)

| Rango | Cuenta | Próxima acción |
|-------|--------|----------------|
| **015–016** | 2 | **Patient Timeline API** (event sourcing) + **Clinical Decision Support** (FHIR PlanDefinition + reglas) |
| 017–026 | 10 | Core platform: multi-tenancy, tele-ICU, device registry, alert escalation, data quality, cohort extraction, DR drill |
| 032–033 | 2 | ML serving ONNX/WASM + Patient similarity engine (embeddings) |

**Próximo paso natural:** abrir **SPEC-015 + 016 en paralelo** — extienden `api/patients` + `scales`/`ml` ya existentes, alto valor clínico, sin romper gates actuales.

---

## Documentación

- [ROADMAP.md](ROADMAP.md) — Plan de fases y hitos
- [CHANGELOG.md](CHANGELOG.md) — Historial de versiones (conventional commits)
- [specs/](specs/) — Especificaciones SDD con criterios Gherkin
- [docs/ADR.md](docs/ADR.md) — Architecture Decision Records
- [docs/API.md](docs/API.md) — Endpoints REST + FHIR + WebSocket

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