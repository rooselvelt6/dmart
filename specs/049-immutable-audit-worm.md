# SPEC-049: Auditoría inmutable (WORM) — cadena de hashes + lotes firmados

## Contexto
- **Problema a resolver**: los eventos de `audit_logs` son mutables en la práctica
  (tabla schemaless, sin firma ni encadenamiento) y no hay forma de probar que un
  registro no fue alterado o borrado dentro de la retención de 6 años exigida por HIPAA.
- **Usuario objetivo**: oficial de cumplimiento / auditor externo (Fase 4.2) y Admin.
- **Métrica de éxito (KPI)**: 100 % de eventos nuevos encadenados con SHA-256 y
  todos los lotes sellados verificables por HMAC-SHA256; `GET /admin/audit/verify`
  reporta `ok=true`; detección de tampering en < 1 s.

## Acceptance Criteria (Gherkin)

```gherkin
Feature: Auditoría inmutable (WORM)
  As a compliance officer
  I want every audit event chained and batch-signed
  So that tampering or deletion within retention is detectable

  Scenario: Cada evento se encadena al anterior
    Given el servicio de auditoría inicializado
    When se registra un evento
    Then el registro incluye "prev_hash" y "content_hash" (SHA-256)
    And "prev_hash" apunta al "content_hash" del evento previo
    And el primer evento apunta al hash génesis

  Scenario: Sellado de lote firmado
    Given N eventos sin sellar
    When POST /api/v1/admin/audit/seal
    Then se crea un registro audit_batches con sequence incremental
    And "prev_batch_hash" encadena con el lote anterior
    And "batch_hash" cubre hashes de contenido + rango
    And "signature" = HMAC-SHA256(batch_hash) con la clave de auditoría

  Scenario: Verificación de integridad
    Given una cadena de eventos y lotes
    When GET /api/v1/admin/audit/verify
    Then devuelve IntegrityReport con "ok", logs_valid, signatures_valid y chain_valid
    And si un evento fue alterado, "ok" es false

  Scenario: Export de lectura
    Given una cadena existente
    When GET /api/v1/admin/audit/export (admin, audit:read)
    Then devuelve logs + lotes firmados + head_batch_hash para verificación externa

  Scenario: Retención intacta
    Given eventos con más de 6 años
    When POST /api/v1/admin/audit/cleanup
    Then solo esos eventos se eliminan
    And la retención sigue siendo 6 años
```

## API Contracts

| Método | Path | Auth | Request | Response | Errores |
|--------|------|------|---------|----------|---------|
| POST | `/api/v1/admin/audit/seal` | `audit:act` | — | `200 {data: AuditBatch\|null}` | 401, 403, 500 |
| GET | `/api/v1/admin/audit/verify` | `audit:read` | — | `200 {data: IntegrityReport}` | 401, 403, 500 |
| GET | `/api/v1/admin/audit/export` | `audit:read` | `?limit` | `200 {data: AuditExport}` | 401, 403, 500 |

## Data Models

```sql
DEFINE TABLE IF NOT EXISTS audit_batches SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS batch_id ON audit_batches TYPE string;
DEFINE FIELD IF NOT EXISTS sequence ON audit_batches TYPE int;
-- first_uid, last_uid, count, first_ts, last_ts,
-- prev_batch_hash, batch_hash, signature, created_at : string/int
DEFINE INDEX IF NOT EXISTS idx_audit_batches_sequence ON audit_batches COLUMNS sequence;
DEFINE INDEX IF NOT EXISTS idx_audit_logs_content_hash ON audit_logs COLUMNS content_hash;
```

Evento (`audit_logs`, aditivo): `prev_hash`, `content_hash` (opcionales para legado).

### Migraciones requeridas
- `migrations/048_audit_worm.surql`

## Edge Cases
| # | Caso | Comportamiento esperado |
|---|------|------------------------|
| 1 | Sin eventos | `seal` devuelve `null`; `verify` con logs_total 0 → ok true |
| 2 | Registro legado sin hash | Se cuenta en logs_unhashed y usa hash derivado al sellar |
| 3 | Firma inválida | `signatures_valid < batches_total` y `chain_valid=false` |
| 4 | Alterar `success`/`details` | Se detecta: `logs_valid < logs_total` |
| 5 | Fallo de inserción | La cadena revierte `last_hash` al valor previo |

## Security Considerations

### Threat Model (STRIDE)
| Threat | Mitigación |
|--------|------------|
| Spoofing | Firma HMAC con clave derivada de `DMART_MASTER_KEY`/`DMART_AUDIT_HMAC_KEY` |
| Tampering | Cadena SHA-256 + verificación recomputable + detección de alteración |
| Repudiation | Hash encadenado y lote firmado con secuencia monotónica |
| Information Disclosure | RBAC `audit:read`/`audit:act`; sin PHI en campos de metadatos |
| Denial of Service | `limit` acotado en export |
| Elevation of Privilege | Permiso `audit:act` solo admin para sellar |

### Data Classification
- [x] Operational/Metadata
- [x] PII (usuario/IP en eventos)

## Testing Strategy
- [x] `test_audit_worm_chain_seal_and_verify` (sellado, verificación, tampering, export)
- [x] `cargo test -p dmart-server --lib` (migración 15 + tests existentes)
- [x] `cargo test -p dmart-server --test api_tests`

## Definition of Done
- [x] Código + clippy sin warnings nuevos
- [x] Tests verdes
- [x] CHANGELOG actualizado
