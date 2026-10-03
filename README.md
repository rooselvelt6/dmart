<div align="center">

# 🏥 dMart UCI

**Sistema de Gestión de Unidad de Cuidados Intensivos**
_100% Rust · WebAssembly · SurrealDB — designed for an isolated hospital network_

<img src="dmart-app/icon.svg" alt="dMart UCI" width="120">

[![CI](https://github.com/rooselvelt6/dmart/actions/workflows/ci.yml/badge.svg)](https://github.com/rooselvelt6/dmart/actions/workflows/ci.yml)
[![Rust](https://img.shields.io/badge/Rust-1.98-orange?logo=rust)](https://www.rust-lang.org/)
[![WASM](https://img.shields.io/badge/Frontend-WebAssembly-654FF0?logo=webassembly)](https://webassembly.org/)
[![Leptos](https://img.shields.io/badge/UI-Leptos%200.8-FF4B4B?logo=leptos)](https://leptos.dev/)
[![Axum](https://img.shields.io/badge/Backend-Axum%200.8-99A0AA)](https://github.com/tokio-rs/axum)
[![SurrealDB](https://img.shields.io/badge/DB-SurrealKV-FF00A0?logo=surrealdb)](https://surrealdb.com/)
[![Tests](https://img.shields.io/badge/Tests-479%20verdes-10B981)](PLAN39.md)
[![Coverage](https://img.shields.io/badge/Coverage-gate%20%E2%89%A560%25-22c55e)](specs/027-coverage-gate-ci.md)

</div>

---

## ✨ El proyecto en una frase

> Un **solo binario Rust** que aloja la UCI completa: API REST + SSE, parser
> **HL7 v2 + MLLP**, **FHIR R4**, scores clínicos (APACHE II, NEWS2, GCS, SOFA, SAPS III),
> **ML de mortalidad y estancia**, auditoría inmutable **WORM**, Web Push con VAPID,
> y frontend **PWA (Leptos/WASM)** — en red hospitalaria **aislada, sin internet**.

---

## 🎯 Estado del Roadmap (SPEC-001 → SPEC-052)

| Fase | Alcanzada | Qué se entregó |
|------|-----------|----------------|
| **1 · Fundaciones** | ✅ SPEC-001…008 | Fix WASM, auth hardening, observabilidad, CI, seguridad |
| **2 · Interop + ML** | ✅ SPEC-009…038 | FHIR R4, HL7 integración, ML ensemble, LOS-NN, early warning, escalamiento |
| **3 · Producción** | ✅ SPEC-039…043 | GitOps ArgoCD/Flux, cluster SurrealDB, DR, multi-tenancy, SBOM/SLSA |
| **4 · Operación** | ✅ SPEC-044…052 | Soporte/RBAC, auditoría WORM, SLO/SLI, **Web Push VAPID** |

> 47 specs en [`specs/`](specs/) · Estado abierto y real en **[PLAN39.md](PLAN39.md)**

---

## 🔒 Seguridad — Estado real (verificado 2 de octubre)

| Pilar | Implementado | Pendiente |
|-------|--------------|------------|
| **AuthN** | Argon2id, JWT HS256 revocable (access 15 min / refresh 7 días), MFA TOTP RFC 6238, `JWT_SECRET` en `Zeroizing` | — |
| **AuthZ** | RBAC `Admin·Médico·Enfermero·Viewer·Soporte`; `ResourceOwner` + `require_tenant_ownership` con verificación de pertenencia en cada acceso a recurso | Aislamiento por tenant en todas las rutas: hoy el middleware existe y se aplica a `patients`, no a las demás entidades |
| **Cifrado reposo** | **AES-256-GCM** envelope `DMART_A2` + auto-detección `DMART_V1` legacy; subclaves HMAC-SHA256 (`LABEL_PHI`, `LABEL_INDEX`); índices ciegos | **Backfill de filas legacy** — ver P0.2 de PLAN39 |
| **Secretos** | `validate_secret_strength()` fail-closed: ≥32 chars, hex de 64, rechazo de placeholders | KMS/HSM externo + rotación sin downtime |
| **Auditoría WORM** | SHA-256 encadenado (`prev_hash`), lotes firmados HMAC-SHA256, concurrencia segura (`tokio::Mutex`) | — |
| **MLLP** | TLS 1.3 obligatorio en prod, pinning SHA-256 DER→MSH.3, fail-closed | — |
| **Supply chain** | `cargo deny` + `cargo audit` con lista de ignorados sincronizada y justificada | Firma de artefactos, gitleaks, PAT de GitHub por revocar |

### Hallazgos del 2 de octubre

Tres cosas que los tests llevaban tiempo señalando y que nadie había visto, porque
**el workflow de CI estaba siendo rechazado por GitHub** y por tanto no se ejecutaba:

- El parser PEM usaba `rustls-pemfile`, **sin mantener** (RUSTSEC-2025-0134). Sustituido
  por `PemObject` de `rustls-pki-types`.
- El servidor MLLP escribía un ACK explicando el rechazo de un frame gigante y acto
  seguido cerraba con bytes sin leer: el kernel respondía **RST y el emisor nunca se
  enteraba del rechazo**. Corregido drenando el frame hasta su terminador `0x1C 0x0D`,
  con tope de 64 KiB para no reabrir el DoS que el límite previene.
- El rate limiter global estaba **hardcodeado** a 100 req/min por IP sin override.

---

## 🧩 Capacidades clínicas

| Dominio | Implementación |
|---------|----------------|
| **Pacientes** | CRUD, ingresos/egresos con desenlace, timeline longitudinal (**PHI cifrado + índices ciegos**) |
| **Monitores de cama** | HL7 v2 `ORU^R01` sobre **MLLP (TCP)** — drivers Mindray, Philips, genéricos; backpressure |
| **Severidad** | **APACHE II**, **GCS**, **NEWS2**, **SOFA**, **SAPS III** — con validación clínica |
| **Mortalidad** | Riesgo hospitalario (fórmula APACHE II) + **ensemble de ML** (DecisionTree/LR/GBM) |
| **Estancia (LOS)** | Red neuronal **MLP** con `candle` 0.8 (feature `ml-nn`) |
| **Early Warning** | **EWS streaming**: NEWS2/APACHE/SOFA → **SSE** en tiempo real por cama |
| **Prevención de esquirlas** | SLI/SLO + error budgets + alerta de escalamiento |
| **Interoperabilidad** | **FHIR R4**: Patient, Observation (LOINC), Condition (CIE-10), DiagnosticReport, Bundle |
| **Auditoría** | Export encadenado con verificación de integridad de la cadena WORM |
| **Asistencia** | Timeline, planes de decisión clínica (CDS), auditoría, tenants con impersonación acotada |

---

## 🧱 Arquitectura

```
dmart/
├─ dmart-shared/   # Modelos, escalas clínicas, validación, ML (DecisionTree/Ensemble/LOS-NN)
├─ dmart-server/   # Axum API · auth/RBAC · HL7+MLLP · FHIR R4 · auditoría WORM
│  ├─ crypto.rs    #   AES-256-GCM, PhiCipher, subclaves HMAC-SHA256, zeroize
│  ├─ phi_store.rs #   PHI envelope DMART_A2 + blind indexes
│  ├─ audit.rs     #   WORM chain + lotes firmados (tokio::Mutex)
│  ├─ server_ingest.rs # Ingestión MLLP: framing, anti-DoS, ACK de rechazo
│  ├─ security.rs  #   Rate limiter, login throttle, cabeceras, CSP/HSTS
│  ├─ mllp_tls.rs  #   TLS 1.3 + pinning
│  ├─ hl7/         #   Parser ORU^R01 · framing MLLP
│  ├─ api/         #   Endpoints versionados (/api y /api/v1)
│  ├─ migrations/  #   SurrealQL versionado
│  └─ fuzz/        #   cargo-fuzz (json, hl7, scales)
├─ dmart-app/      # Frontend Leptos/WASM (PWA offline, Service Worker, Web Push)
├─ tests/load/     # k6: autenticación y escalas clínicas
├─ tests/e2e/      # Playwright: login, patients, measurements, admin
├─ specs/          # Spec-Driven Development (SPEC-001…052)
└─ docs/           # API · arquitectura · runbooks · compliance
```

**Runtime**
- **Single binary** — sin procesos externos obligatorios (SurrealKV embebida).
  **Despliegue nativo single-binary**: ya no hay ruta Docker (eliminado del repo el 2/oct).
- **SurrealDB** con `surrealkv` embebida y migraciones versionadas.
- **Observabilidad** — Prometheus `/metrics`, `/api/health`, tracing JSON.

**Features de `dmart-shared`** — `chrono` (default), `wasm-time` (frontend),
`ml-nn` (red neuronal, requiere Candle 0.8). Ningún crate del workspace activa
`ml-nn` por defecto: se compila y prueba en CI, no en el binario de producción.

---

## 🚀 Inicio rápido

```bash
# Requisitos (el toolchain lo fija rust-toolchain.toml: 1.98.0)
rustup target add wasm32-unknown-unknown
cargo install trunk --locked

# Variables obligatorias — el arranque es fail-closed sin ellas
export DMART_MASTER_KEY="$(openssl rand -hex 32)"   # cifrado de PHI
export JWT_SECRET="$(openssl rand -hex 32)"          # firma de tokens

# Terminal 1 — Backend (puerto 3000, configurable con DMART_PORT)
cargo run --release --bin dmart-server

# Terminal 2 — Frontend (puerto 8080)
cd dmart-app && trunk serve --port 8080
```

El primer arranque siembra un usuario `admin` con la contraseña de
`DMART_ADMIN_PASSWORD` (si no se define, genera una temporal y la muestra una vez).

---

## 🧪 Tests y calidad

**479 tests** en total. Comandos según [`AGENTS.md`](AGENTS.md):

```bash
# Gate rápido (~15 s, sin compilar WASM ni fuzz) — el que se usa siempre
cargo test -p dmart-server --lib --test api_tests --test hl7_integration

# Suites sueltas
cargo test -p dmart-server --lib                    # 188
cargo test -p dmart-shared --features ml-nn --lib  # 66 (Candle 0.8)
cargo test -p dmart-server --test hl7_integration   # 32
cargo test -p dmart-server --test api_tests         # 48

# Lint y formato
cargo clippy -p dmart-server -p dmart-shared --all-targets -- -D warnings
cargo fmt --all -- --check

# Carga (requiere el server levantado)
ADMIN_USERNAME=admin ADMIN_PASSWORD=<la que definiste> k6 run tests/load/scales.js
```

> **Nunca** `cargo test --workspace`: compila `dmart-app` (WASM Leptos) y `fuzz`
> (libFuzzer), y tarda horas.

### CI — 15 jobs

`Spec Lint` · `Format` · `Clippy` · `Library Tests` · `Integration Tests (API)` ·
`HL7 + MLLP` · `Property Tests` · `Fuzzing` · `Coverage Gate` ·
`Dependency Policy (cargo-deny)` · `Security Audit` · `Load Test (k6)` ·
`WASM Build` · `E2E (Playwright)` · `Release Build`

Plus `notify`, que informa del resultado al chat del equipo.

**Cobertura**: gate global ≥60 % con requisitos por módulo (SPEC-027).
Los advisories ignorados viven en un único sitio, [`deny.toml`](deny.toml), y el job
de `cargo audit` los extrae de ahí.

---

## 📋 Estado del plan de trabajo

- **[PLAN39.md](PLAN39.md)** — evaluación honesta (6,5/10) y camino a 10, con el
  comando que comprueba cada punto.
- **[PLAN.md](PLAN.md)** — hardening post-roadmap histórico (F0–F5).

**Lo que sigue, en orden:**

1. **P0.1 E2E real** — los specs de Playwright nunca se han ejecutado. Es el hueco
   navegador→API y no requiere escribir código nuevo.
2. **P0.2 Backfill de PHI** — el cifrado se aplica al escribir; las filas legacy en
   disco siguen en claro. Es el mayor riesgo de cumplimiento que queda abierto.
3. **P0.3 i18n del contenido** — la navegación está traducida en ES/EN/PT/FR, pero el
   contenido de las páginas nuevas sigue hardcodeado en español.

---

## 📚 Documentación

- **[PLAN39.md](PLAN39.md)** — plan vigente, de 6,5 a 10
- [CHANGELOG.md](CHANGELOG.md) — historial (conventional commits)
- [specs/](specs/) — especificaciones SDD (SPEC-001…052)
- [docs/API.md](docs/API.md) — endpoints REST + FHIR + SSE
- [docs/ARQUITECTURA.md](docs/ARQUITECTURA.md) — arquitectura del sistema
- [docs/runbook/](docs/runbook/) — procedimientos de incidentes (`auth-lockout`,
  `mllp-down`, `backpressure`, `disk-backup`, `dr-restore`)
- [docs/compliance/](docs/compliance/) — catálogo de controles, flujos de datos, incidentes, legal
- [AGENTS.md](AGENTS.md) — convenciones y comandos de verificación

---

## 🤝 Contribución

1. Lee [PLAN39.md](PLAN39.md) y elige un punto con su comando de comprobación
2. Abre issue con la especificación
3. Implementa → tests → `cargo fmt --all` y `cargo clippy -D warnings`
4. Actualiza CHANGELOG.md y abre PR

---

<div align="center">

**dMart UCI** — Un solo binario para una UCI completa.
Hecho con Rust.

</div>
