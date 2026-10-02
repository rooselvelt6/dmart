# Plan de Seguridad y Arquitectura dMart UCI

> **Estado actual**: F0–F2 en curso, F3–F5 diferidos. Último gate completo **verde: 303 tests**.
> Rama `main`, cambios de hoy **sin commitear** (14 modificados, 9 borrados, 2 nuevos).

---

## ✅ Completado hoy

| ID | Descripción | Evidencia |
|----|-------------|-----------|
| F2.3b | MLLP fail-closed en producción; pinning SHA-256 DER→MSH.3 | `server_ingest.rs:689`, `mllp_tls::listener_tests` (8) |
| — | **ACK MLLP corregido**: un solo `MSH`, `ERR` dentro del frame antes de `0x1C 0x0D` | `hl7/mllp.rs::build_ack`, tests `mll7_integration` |
| F0.5a | AES-256-GCM envelope `DMART_A2` + auto-detección `DMART_V1`; subclave `LABEL_PHI` derivada (HMAC-SHA256), AAD v2 con longitudes prefijadas | `crypto.rs`, `phi_store.rs` |
| F0.5b (patients) | Cifrado PHI `patients` + índices ciegos + fallback de lectura legacy | `phi_store.rs`, migración `050`, tests `phi_store::db_tests` |
| — | **Cadena WORM concurrente**: `ChainState` movida de `OnceLock` global a `Arc<tokio::Mutex>` por `AuditService`; guard mantenido durante el `.await` del insert | `audit.rs`, test `test_audit_chain_survives_concurrent_writes` |
| — | `deployment::is_production()` + `flag_enabled()` centralizados (`DMART_ENV`, alias `APP_ENV`) | `deployment.rs`, usado por `crypto` y `server_ingest` |
| — | Lock de tests compartido (`tests_lock` / `EnvGuard`) | `deployment.rs` |
| — | **Docker eliminado del repo**: `Dockerfile*`, `.dockerignore`, 4 `docker-compose*.yml`, job `docker-build` de `ci.yml`, workflow `release-image.yml` | `ci.yml` (YAML validado), `docs/SURREALDB_CLUSTER.md` |
| **A** | Correcciones de documentación: badge `303`, terminología "HMAC-SHA256", pinning "SHA-256 DER→MSH.3", despliegue nativo single-binary | `README.md`, `PLAN.md` |
| **B** | Integridad de auditoría: `verify_integrity()` recorre por `prev_hash` (no timestamp/uid) | `audit.rs`, tests `test_audit_chain_survives_concurrent_writes`, `test_audit_worm_chain_seal_and_verify` |
| **C** | Compatibilidad índices ciegos: `blind_index_legacy()` + lectura dual (actual + legacy) | `crypto.rs::blind_index_legacy`, `phi_store.rs::search_patients_exact`, `find_patients_by_identifier` |
| **D** | Gates de calidad: `cargo fmt --all` + 102 tests server pasan | `--lib --test api_tests --test hl7_integration --test cds_rules --test ews_streaming --test data_quality --test escalation --test teleicu` |
| **F0.5b-resto (measurements)** | Cifrado PHI `measurements` (apache_data, gcs_data) | migración `051`, `phi_store.rs::MeasurementPhi`/`MeasurementRow`, `db.rs` create/get measurements |

---

## 🔴 HIGH — Bloquean release de seguridad

| ID | Título | Detalle / Archivos clave | Estado |
|----|--------|--------------------------|--------|
| **F0.5b-resto** | Cifrar PHI en `care_plan`, `audit`, `push`, `device_registry`, `reports` | Reutilizar `PhiCipher`/`PhiContext`; migraciones + índices ciegos; búsquedas exactas en `db.rs`. **Preservar** canonicalización, `prev_hash` y HMAC de la auditoría al cifrar su payload | ✅ COMPLETADO (migraciones 053-057, structs + seal/open en `phi_store.rs`) |
| **F0.5b-backfill** | Job idempotente para cifrar filas legacy `patients` (hoy sólo al reescribirlas) | `cargo run --bin backfill_phi_patients`, dry-run + métricas | ✅ COMPLETADO (binario en `src/bin/backfill_phi_patients.rs`) |
| **F2.3c-zeroize** | `JWT_SECRET` y secreto MLLP en `Zeroizing` (ya hecho: `MasterKey`, `PhiCipher`) | `auth.rs`, `server_ingest.rs::MllpSecurityConfig` | ✅ COMPLETADO |
| **F2.1-rbac** | RBAC fino + `tenant_id` NOT NULL en `users`, `audit`, `push`, `devices`, `reports` | Middleware `require_tenant`, policy engine, tests multi-tenant | ✅ COMPLETADO (middleware en `middleware/rbac.rs`, trait `ResourceOwner` con impls) |
| **F2.1-phi-extra** | Revisar PHI residual en `camas.paciente_nombre` | `db.rs`, migración `052` | ✅ COMPLETADO (migración `052`, PHI en `camas.phi`, integración en transacciones) |
| **PAT expuesto** | Revocar PAT en GitHub | `gh auth logout -h github.com` + GitHub Settings → Tokens | ⏳ MANUAL (5 min) |

---

## 🟡 MEDIUM — Hardening operacional

| ID | Título | Detalle |
|----|--------|---------|
| F1.1 | Rotación de claves + KMS/HSM externo | Interfaz `KeyProvider`; AWS KMS / Vault / Azure Key Vault |
| F1.2 | Rate limiting distribuido por tenant (login/API) | Redis + token bucket; headers `X-RateLimit-*` |
| F1.3 | Hardening SSO/OIDC (state/nonce/PKCE, issuer/audience) | `auth.rs`; rechazar `id_token` sin `nonce` |
| F2.4 | Validación estricta TLS saliente + anti-SSRF | `rustls` + `webpki-roots`; deny-list de CIDR privados |
| F2.5 | Eliminar `panic!`/`unwrap` en handlers | Errores genéricos + logging estructurado |

---

## 🟢 LOW — Supply chain & performance

| ID | Título |
|----|--------|
| F3.1 | Firmar artefactos (cosign/slsa) + verificar en despliegue |
| F3.2 | Pipelines least-privilege, sin `pull_request_target` |
| F3.3 | Dependency pinning (`Cargo.lock` committed) + `cargo audit`/`cargo deny` en CI |
| F3.4 | Secrets scanning en CI (gitleaks / trufflehog) |
| F4.x | Conectar módulos desconectados (alerts, files, ML inference, es256) |
| F5.x | Rendimiento: índices SurrealDB, cache (moka/redis), pooling |

---

## 🔧 Pendientes de infraestructura tras quitar Docker

No hay todavía ruta de despliegue nativa documentada. Crear antes del release:

- Unidad `systemd` para `dmart-server` (con `EnvironmentFile`, hardening: `NoNewPrivileges`, `ProtectSystem`, `ProtectHome`, `PrivateTmp`).
- Reverse proxy nativo (Caddyfile o nginx) sustituyendo a `Dockerfile.caddy`.
- Guía de despliegue / upgrade / rollback.
- Decidir qué queda del chart Helm: `helm/dmart/templates/statefulset-surrealdb.yaml` sigue siendo válido para SurrealDB externo, pero hay que revisar si el resto del chart presupone imágenes.

---

## Gates de calidad (obligatorios antes de merge)

```bash
cargo fmt --all -- --check
cargo clippy -p dmart-server --all-targets -- -D warnings
cargo test -p dmart-server --lib --test api_tests --test hl7_integration \
         --test cds_rules --test ews_streaming --test data_quality \
         --test escalation --test teleicu
```

> **Nunca** `cargo test --workspace` ni `cargo llvm-cov --workspace`: compila `dmart-app` (WASM Leptos) y `fuzz` (libFuzzer), y tarda horas. Ver `AGENTS.md`.

---

*Última actualización: 2026-10-02*