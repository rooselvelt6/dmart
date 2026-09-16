# SPEC-024: Disaster Recovery — RPO<1h, RTO<4h

## Contexto

- **Problema a resolver**: El backup automático (SPEC-009) crea snapshots diarios, pero no hay procedimiento documentado ni probado de restauración. En un escenario de desastre (fallo de disco, corrupción de datos, error humano), se necesita restaurar el estado de la base de datos con objetivos claros: RPO (Recovery Point Objective) < 1h y RTO (Recovery Time Objective) < 4h.
- **Usuario objetivo**: SRE / On-call / Administrador hospitalario
- **Métrica de éxito (KPI)**: DR drill trimestral documentado; restore probado en < 4h; RPO medido < 1h; 0 pérdida de datos confirmados post-restore

## Acceptance Criteria (Gherkin)

```gherkin
Feature: Disaster Recovery
  As a administrador de UCI
  I want restaurar la base de datos desde backup en < 4 horas
  So that un desastre no cause pérdida permanente de datos clínicos

  Scenario: Backup automático incremental cada hora
    Given SurrealDB corriendo con datos
    When pasa 1 hora
    Then se crea backup incremental en almacenamiento externo (S3/MinIO)
    And el backup contiene todos los cambios de la última hora
    And el backup es verificado (checksum + restore test)

  Scenario: Restore completo desde último backup
    Given un desastre que destruye la base de datos
    When ejecuto `./scripts/dr_restore.sh --backup latest`
    Then la base de datos se restaura al estado del último backup
    And RPO < 1h (máximo 1h de datos perdidos)
    And RTO < 4h (restauración completa en < 4 horas)
    And todos los pacientes, mediciones, scores son recuperados

  Scenario: Restore point-in-time
    Given backups incrementales desde hace 24h
    When ejecuto `./scripts/dr_restore.sh --timestamp "2026-09-15T14:30:00Z"`
    Then la base de datos se restaura al estado exacto de 14:30
    And todos los eventos posteriores a 14:30 son descartados
    And el timestamp de restore es verificado

  Scenario: DR drill trimestral
    Given un entorno de staging idéntico a producción
    When ejecuto `./scripts/dr_drill.sh`
    Then el script automatiza: backup → destruir DB → restore → verificar
    And genera reporte con métricas (tiempo, datos restaurados, checksums)
    And el reporte se archiva en `docs/compliance/dr_reports/`

  Scenario: Backup integrity check
    Given backups almacenados en S3/MinIO
    When ejecuto `./scripts/dr_verify.sh --backup latest`
    Then el backup es descargado y restaurado en DB temporal
    And los checksums de las tablas críticas coinciden
    And el reporte de verificación se genera en < 30 min

  Scenario: Alerta de backup failure
    Given backup job configurado (cron cada hora)
    When el backup falla 2 veces consecutivas
    Then alerta DRBackupFailure dispara (SPEC-008)
    And el on-call es notificado en < 5 min
```

## API Contracts

### Scripts de DR

```bash
# Backup incremental (cron cada hora)
./scripts/dr_backup.sh \
  --type incremental \
  --retention 7d \
  --destination s3://dmart-backups/ \
  --compress zstd

# Restore completo
./scripts/dr_restore.sh \
  --backup latest \
  --target /data/surrealdb \
  --verify-checksums

# Restore point-in-time
./scripts/dr_restore.sh \
  --timestamp "2026-09-15T14:30:00Z" \
  --target /data/surrealdb \
  --verify-checksums

# DR drill automatizado
./scripts/dr_drill.sh \
  --env staging \
  --report docs/compliance/dr_reports/

# Verificación de integridad
./scripts/dr_verify.sh \
  --backup latest \
  --tables patients,measurements,scores \
  --report /tmp/dr_verify_report.json
```

### Métricas de DR

```prometheus
# Backup status
dr_backup_last_success_timestamp{type="full|incremental"}    # Gauge: Unix timestamp
dr_backup_duration_seconds{type="full|incremental"}          # Histogram
dr_backup_size_bytes{type="full|incremental"}                # Gauge
dr_backup_errors_total{type="full|incremental"}              # Counter

# Restore metrics
dr_restore_last_duration_seconds                             # Gauge
dr_restore_data_recovered_bytes                              # Gauge
dr_restore_checksums_verified                                # Gauge (0/1)

# DR Drill
dr_drill_last_run_timestamp                                  # Gauge
dr_drill_last_rpo_seconds                                    # Gauge (RPO medido)
dr_drill_last_rto_seconds                                    # Gauge (RTO medido)
dr_drill_success_total                                       # Counter
```

### Configuración

```bash
# Backup
DMART_DR_BACKUP_ENABLED=true
DMART_DR_BACKUP_SCHEDULE="0 * * * *"          # cada hora
DMART_DR_BACKUP_TYPE="incremental"
DMART_DR_BACKUP_RETENTION_DAYS=7
DMART_DR_BACKUP_DESTINATION="s3://dmart-backups/"
DMART_DR_BACKUP_S3_ENDPOINT="https://minio.hospital.cu"
DMART_DR_BACKUP_S3_ACCESS_KEY="${MINIO_ACCESS_KEY}"
DMART_DR_BACKUP_S3_SECRET_KEY="${MINIO_SECRET_KEY}"

# Restore
DMART_DR_RESTORE_VERIFY=true
DMART_DR_RESTORE_CHECKSUM_TABLES="patients,measurements,scores,timeline_events"

# Alerts
DMART_DR_ALERT_CONSECUTIVE_FAILURES=2
```

## Data Models

### Backup Manifest

```json
{
  "id": "backup-20260915-140000",
  "type": "incremental",
  "timestamp": "2026-09-15T14:00:00Z",
  "parent_id": "backup-20260915-130000",
  "tables": {
    "patients": { "count": 156, "checksum": "sha256:abc..." },
    "measurements": { "count": 45230, "checksum": "sha256:def..." },
    "scores": { "count": 8901, "checksum": "sha256:ghi..." },
    "timeline_events": { "count": 23456, "checksum": "sha256:jkl..." }
  },
  "size_bytes": 52428800,
  "duration_seconds": 45,
  "s3_key": "backups/2026/09/15/backup-20260915-140000.tar.zst",
  "status": "completed"
}
```

### DR Report

```json
{
  "drill_id": "drill-20260915",
  "timestamp": "2026-09-15T16:00:00Z",
  "rpo_measured_seconds": 2847,
  "rto_measured_seconds": 8934,
  "rpo_target_seconds": 3600,
  "rto_target_seconds": 14400,
  "rpo_pass": true,
  "rto_pass": true,
  "data_verification": {
    "patients_restored": 156,
    "measurements_restored": 45230,
    "checksums_match": true
  },
  "steps": [
    { "step": "backup_latest", "duration_seconds": 45, "status": "ok" },
    { "step": "destroy_db", "duration_seconds": 12, "status": "ok" },
    { "step": "restore_db", "duration_seconds": 8934, "status": "ok" },
    { "step": "verify_checksums", "duration_seconds": 120, "status": "ok" },
    { "step": "verify_api_health", "duration_seconds": 30, "status": "ok" }
  ],
  "report_path": "docs/compliance/dr_reports/drill-20260915.json"
}
```

## Edge Cases

| # | Caso | Comportamiento esperado |
|---|------|------------------------|
| 1 | Backup durante escritura activa | Consistent snapshot (SurrealDB snapshot) |
| 2 | S3/MinIO inaccesible durante backup | Retry 3x; alerta si falla |
| 3 | Restore con backup corrupto | Checksum mismatch → abort; siguiente backup |
| 4 | Restore en cluster con datos actuales | Pregunta confirmación; --force para emergencia |
| 5 | DR drill en producción | Solo en staging; --prod requiere approval |
| 6 | Backup > 100GB | Parallel upload; timeout extendido |
| 7 | Restore parcial (1 tabla) | Soportado via `--tables patients` |

## Security Considerations

### Threat Model (STRIDE)

| Threat | Mitigación |
|--------|------------|
| Spoofing | Backup firmado con HMAC; verify antes de restore |
| Tampering | Checksum SHA-256 en cada backup; immutable storage |
| Repudiation | Audit log de cada operación DR (backup/restore/drill) |
| Information Disclosure | Backup en-at-rest encryption (S3 SSE-KMS) |
| Denial of Service | Rate limit restore; window de mantenimiento para DR drill |
| Elevation of Privilege | DR scripts requieren RBAC admin; audit trail |

### Encryption

```bash
# Backup en-at-rest
DMART_DR_BACKUP_ENCRYPTION="aes-256-gcm"
DMART_DR_BACKUP_KMS_KEY="${DR_KMS_KEY_ID}"

# Backup en tránsito
DMART_DR_BACKUP_S3_ENDPOINT="https://minio.hospital.cu"  # TLS
```

### Data Classification
- [x] PHI (Protected Health Information)
- [x] Clinical Data
- [x] Operational/Metadata (backup manifests)

## Testing Strategy

### Unit Tests
- [ ] Backup script genera archivo .tar.zst válido
- [ ] Checksum calculado correctamente
- [ ] Restore script extrae y verifica archivos
- [ ] Reporte DR genera JSON válido

### Integration Tests
- [ ] Backup → destroy → restore → verify completo
- [ ] Point-in-time restore funciona
- [ ] Backup incremental solo incluye cambios
- [ ] Verify detecta backup corrupto

### DR Drill (automatizado)
- [ ] `dr_drill.sh` ejecuta todo el ciclo en staging
- [ ] RPO medido < 1h
- [ ] RTO medido < 4h
- [ ] Checksums coinciden post-restore
- [ ] API funcional post-restore

### Chaos Tests
- [ ] Backup durante alta carga de escritura
- [ ] Restore con disco casi lleno
- [ ] Network interruption durante backup S3

## Rollout Plan

### Feature Flag
```yaml
dr:
  enabled: true
  backup:
    schedule: "0 * * * *"
    retention_days: 7
  restore:
    verify: true
  drill:
    schedule: "0 0 1 */3 *"  # trimestral
```

### Canary Deployment
| Paso | Tráfico | Duración | Criterio go/no-go |
|------|---------|----------|-------------------|
| 1 | Backup manual 1 vez | — | Backup exitoso + verificado |
| 2 | Backup automático (cron) | 1 semana | 0 failures; tamaño consistente |
| 3 | Restore test en staging | 1h | RPO/RTO targets cumplidos |
| 4 | DR drill automatizado | Trimestral | Reporte pasa sin issues |

### Rollback Procedure
1. Si restore falla: mantener DB actual
2. Intentar restore desde backup anterior
3. Si todos los backups fallan: escalar a equipo de DBA
4. Comunicar a stakeholders: pérdida potencial de datos

## Definition of Done

- [ ] `specs/024-disaster-recovery.md` (este archivo)
- [ ] `scripts/dr_backup.sh` funcional (full + incremental)
- [ ] `scripts/dr_restore.sh` funcional (latest + point-in-time)
- [ ] `scripts/dr_verify.sh` funcional
- [ ] `scripts/dr_drill.sh` automatizado
- [ ] Backup automático cada hora (cron)
- [ ] Almacenamiento en S3/MinIO con encryption
- [ ] Métricas DR en Prometheus (`dr_*`)
- [ ] Alerta DRBackupFailure (SPEC-008)
- [ ] DR drill ejecutado exitosamente 1 vez en staging
- [ ] Documentación: `docs/DISASTER_RECOVERY.md`
- [ ] ROADMAP, CHANGELOG actualizados

## Gate CI SPEC-024
```bash
./scripts/dr_backup.sh --type full --destination /tmp/dr-test/  # exit 0
./scripts/dr_verify.sh --backup latest --report /tmp/dr-verify.json  # exit 0
cat /tmp/dr-verify.json | jq '.checksums_match'  # = true
```

## Métricas
- RPO medido: < 1h (backup cada hora)
- RTO medido: < 4h (restore completo)
- Backup success rate: > 99.9%
- DR drill success rate: 100% (trimestral)
- Backup size (incremental promedio): < 50MB
- Restore throughput: > 100MB/s
