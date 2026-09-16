# Control Catalog — dMart UCI Compliance

Catálogo de controles de seguridad mapeados a HIPAA (45 CFR 164), NIST 800-53 e ISO 27001.

Generado como parte del Evidence Pack SPEC-034.

## Controles

| Control ID | Familia | Título | Estándar (HIPAA/NIST/ISO) | Estado | Evidencia | Responsable |
|------------|---------|--------|--------------------------|--------|-----------|-------------|
| AC-1 | Access Control | Policy y procedimientos de control de acceso | 164.312(a)(1) / NIST AC-1 / ISO A.9 | ✅ Implemented | `docs/compliance/CONTROL_CATALOG.md`, RBAC docs | DevOps |
| AC-2 | Access Control | Gestión de cuentas de usuario | 164.312(a)(1) / NIST AC-2 / ISO A.9.2.1 | ✅ Implemented | `src/rbac.rs`, SPEC-004, `tests/api_tests.rs` | Backend |
| AU-2 | Audit & Accountability | Registro de eventos de auditoría | 164.308(a)(1)(ii)(D) / NIST AU-2 / ISO A.12.4.1 | ✅ Implemented | Audit logs en SurrealDB, SPEC-029 fingerprint | Backend |
| CP-1 | Contingency Planning | Plan de contingencia | 164.308(a)(7)(i) / NIST CP-1 / ISO A.17.1 | ✅ Implemented | SPEC-009/010/014/024 DR plan, `scripts/dr_*.sh` | SRE |
| RA-5 | Risk Assessment | Escaneo de vulnerabilidades | 164.308(a)(1)(ii)(A) / NIST RA-5 / ISO A.12.6.1 | 🟡 In progress | `cargo audit` en CI pipeline | Security |
| SI-4 | System & Info Integrity | Monitoreo de seguridad del sistema | 164.312(b) / NIST SI-4 / ISO A.12.4.1 | ✅ Implemented | Prometheus metrics, SPEC-008 alertas | SRE |
| IR-1 | Incident Response | Plan de respuesta a incidentes | 164.308(a)(6)(i) / NIST IR-1 / ISO A.16.1 | ✅ Implemented | SPEC-011 runbook, `docs/runbook/` | Security |
| SC-8 | Cryptography | Confidencialidad e integridad de transmisión | 164.312(e)(1) / NIST SC-8 / ISO A.10.1.1 | ✅ Implemented | TLS en HTTP/MLLP, encryption in transit | Backend |
| PE-1 | Physical & Environmental | Controles físicos y ambientales | 164.310(a)(1) / NIST PE-1 / ISO A.11.1 | ✅ Implemented | Infraestructura hospitalaria (fuera de alcance soft) | Hospital |

## Leyendas

- ✅ Implemented: Control implementado con evidencia verificable
- 🟡 In progress: Parcialmente implementado, con deadline definido
- 🔴 Not implemented: Gap documentado con plan de remediación
- ⬜ Not applicable: No aplica al alcance del sistema

## Referencias

- SPEC-004: RBAC + JWT refresh + tenant isolation
- SPEC-008: Alertas y métricas Prometheus
- SPEC-009/010/014: Backup y disaster recovery
- SPEC-011: Runbook de incidentes
- SPEC-024: Disaster Recovery automatizado
- SPEC-025: Tenant isolation
- SPEC-029: Audit log fingerprinting
- SPEC-031: Ingest hardening
