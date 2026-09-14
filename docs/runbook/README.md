# Runbook operativo oficial — dmart UCI

- **SPEC-011** oficializa este runbook; integra con **SPEC-008** (alertas que lo
  disparan), **SPEC-010** (restore/DR) y **SPEC-031** (backpressure).

## Índice de incidentes
1. [MLLP/Hl7 caído](incidents/mllp-down.md)
2. [Backpressure ingest SPEC-031](incidents/backpressure.md)
3. [Disco > 80% / backups SPEC-009](incidents/disk-backup.md)
4. [Restore / DR SPEC-010](incidents/dr-restore.md)
5. [Lockout MFA/auth SPEC-004](incidents/auth-lockout.md)

## Reglas de gate (SPEC-011)
- Cada incidente debe contener 4 secciones: `## Síntoma · ## Comando · ## Verificar · ## Escalar`.
- Test `runbook_check.rs` verifica existencia + secciones (gate CI verde = runbook vigente).
- MTTR objetivo por incidente ≤ 15 min, coherente con RTO SPEC-010.
