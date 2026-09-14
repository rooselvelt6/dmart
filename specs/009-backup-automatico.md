# SPEC-009: Backup Automático SurrealKV (cron + retención)

## Contexto

- **Problema a resolver**: Los volúmenes `dmart-data` (SurrealKV) y `valkey-data` (cache/sesiones) no tienen backup automático. En producción hospitalaria se requiere RPO ≤ 24h y RTO < 4h con restore probado.
- **Usuario objetivo**: Operaciones / DevOps / Compliance (auditoría)
- **Métrica de éxito (KPI)**: Backup diario exitoso 100%; restore probado mensualmente < 30 min; retención 30 días

## Acceptance Criteria (Gherkin)

```gherkin
Feature: Backup Automático SurrealKV
  As a operador
  I want backups diarios automáticos con retención
  So that pueda recuperar datos ante desastre (RPO ≤ 24h, RTO < 4h)

  Scenario: Backup diario a medianoche
    Given contenedor dmart-server corriendo con volumen dmart-data
    When hora 02:00 UTC
    Then script backup se ejecuta
    And crea snapshot consistente de /app/data/dmart.db
    And comprime y sube a almacenamiento (S3/local/NFS)
    And registra métrica backup_success{status="ok"} = 1

  Scenario: Retención 30 días
    Given backups diarios acumulados
    When pasa 31 días
    Then backups > 30 días se borran automáticamente
    And solo quedan últimos 30

  Scenario: Backup Valkey (cache/sesiones)
    Given Valkey con AOF habilitado
    When backup diario
    Then snapshot RDB + AOF copiados
    And métrica backup_valkey_success registrada

  Scenario: Restore probado (RTO < 4h)
    Given backup válido de ayer
    When operador ejecuta restore en staging
    Then base de datos restaurada en < 30 min
    And healthcheck /obs/health pasa
    And datos clínicos íntegros (pacientes, mediciones, escalas)

  Scenario: Fallo de backup alerta
    Given backup programado
    When script falla (espacio, permisos, red)
    Then métrica backup_success{status="failed"} = 1
    And alerta "BackupFailed" dispara (SPEC-008)

  Scenario: Backup no interrumpe servicio
    Given servidor atendiendo tráfico
    When backup corre
    Then latencia p99 < baseline + 10%
    And 0 requests fallados
```

## API Contracts

No nuevos endpoints HTTP. Interface operacional:
- Script: `/app/scripts/backup.sh` (invocado por cron/systemd)
- Métricas expuestas en `/metrics`:
  - `backup_success{target="surreal|valkey", status="ok|failed"}` (gauge)
  - `backup_duration_seconds{target}` (histogram)
  - `backup_size_bytes{target}` (gauge)
  - `backup_age_hours{target}` (gauge, tiempo desde último backup exitoso)

## Data Models

### Estructura de backup
```
/backups/
├── surreal/
│   ├── dmart-2026-09-14-020000.db.gz     # SurrealKV (copia en frío)
│   ├── dmart-2026-09-13-020000.db.gz
│   └── ...
├── valkey/
│   ├── dump-2026-09-14-020000.rdb.gz     # Valkey RDB
│   ├── appendonly-2026-09-14-020000.aof.gz
│   └── ...
└── metadata/
    └── backup-2026-09-14-020000.json     # {timestamp, targets, sizes, checksums}
```

### Script backup.sh
```bash
#!/bin/bash
set -euo pipefail

BACKUP_ROOT="/backups"
DATE=$(date -u +%Y-%m-%d-%H%M%S)
TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

# ─── SurrealKV (copia en frío: parar writes brevemente) ───
# Opción A: snapshot SurrealDB (requiere API) - NO disponible en embedded KV
# Opción B: copia archivo .db con fsync + lock breve (recomendado para embedded)
sqlite3 /app/data/dmart.db ".backup '$TMP_DIR/dmart.db'"
gzip -c "$TMP_DIR/dmart.db" > "$BACKUP_ROOT/surreal/dmart-$DATE.db.gz"

# ─── Valkey (BGSAVE + copia RDB/AOF) ───
redis-cli -h valkey BGSAVE
# Esperar BGSAVE (polling)
while [ "$(redis-cli -h valkey LASTSAVE)" = "$(redis-cli -h valkey LASTSAVE)" ]; do sleep 1; done
cp /data/dump.rdb "$TMP_DIR/dump.rdb"
cp /data/appendonly.aof "$TMP_DIR/appendonly.aof" 2>/dev/null || true
gzip -c "$TMP_DIR/dump.rdb" > "$BACKUP_ROOT/valkey/dump-$DATE.rdb.gz"
[ -f "$TMP_DIR/appendonly.aof" ] && gzip -c "$TMP_DIR/appendonly.aof" > "$BACKUP_ROOT/valkey/appendonly-$DATE.aof.gz"

# ─── Metadata ───
cat > "$BACKUP_ROOT/metadata/backup-$DATE.json" <<EOF
{
  "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "targets": ["surreal", "valkey"],
  "sizes": {
    "surreal": $(stat -c%s "$BACKUP_ROOT/surreal/dmart-$DATE.db.gz"),
    "valkey_rdb": $(stat -c%s "$BACKUP_ROOT/valkey/dump-$DATE.rdb.gz"),
    "valkey_aof": $(stat -c%s "$BACKUP_ROOT/valkey/appendonly-$DATE.aof.gz" 2>/dev/null || echo 0)
  },
  "checksums": {
    "surreal": "$(sha256sum "$BACKUP_ROOT/surreal/dmart-$DATE.db.gz" | cut -d' ' -f1)",
    "valkey_rdb": "$(sha256sum "$BACKUP_ROOT/valkey/dump-$DATE.rdb.gz" | cut -d' ' -f1)"
  }
}
EOF

# ─── Retención 30 días ───
find "$BACKUP_ROOT/surreal" -name "dmart-*.db.gz" -mtime +30 -delete
find "$BACKUP_ROOT/valkey" -name "*.gz" -mtime +30 -delete
find "$BACKUP_ROOT/metadata" -name "backup-*.json" -mtime +30 -delete

# ─── Métricas (pushgateway o file-based) ───
# Para simplicidad: file-based textfile collector
cat > /var/lib/node_exporter/backup.prom <<EOF
backup_success{target="surreal"} 1
backup_success{target="valkey"} 1
backup_duration_seconds{target="surreal"} $SECONDS_SURREAL
backup_duration_seconds{target="valkey"} $SECONDS_VALKEY
backup_size_bytes{target="surreal"} $(stat -c%s "$BACKUP_ROOT/surreal/dmart-$DATE.db.gz")
backup_size_bytes{target="valkey"} $(stat -c%s "$BACKUP_ROOT/valkey/dump-$DATE.rdb.gz")
backup_age_hours{target="surreal"} 0
backup_age_hours{target="valkey"} 0
EOF
```

### systemd timer (en contenedor sidecar o host)
```ini
# /etc/systemd/system/dmart-backup.service
[Unit]
Description=dMart Backup
After=docker.service
Requires=docker.service

[Service]
Type=oneshot
ExecStart=/usr/bin/docker exec dmart-prod-server /app/scripts/backup.sh
EnvironmentFile=/opt/dmart/.env.prod

# /etc/systemd/system/dmart-backup.timer
[Unit]
Description=Daily dMart Backup at 02:00 UTC

[Timer]
OnCalendar=*-*-* 02:00:00
Persistent=true
RandomizedDelaySec=15m

[Install]
WantedBy=timers.target
```

## Edge Cases

| # | Caso | Comportamiento esperado |
|---|------|------------------------|
| 1 | Disco lleno durante backup | Script falla, métrica failed, alerta dispara, no borra backups viejos |
| 2 | Servidor apagado a la hora del backup | `Persistent=true` en timer ejecuta al encender |
| 3 | Valkey BGSAVE tarda > 10 min | Timeout 15 min, alerta si excede |
| 4 | Corrupción DB detectada en restore | Checksum SHA256 en metadata valida integridad |
| 5 | Backup parcial (solo surreal) | Métrica por target; alerta si alguno falla |
| 6 | Restaurar en versión distinta de SurrealDB | Test de compatibilidad en CI (SPEC-007) |

## Security Considerations

### Threat Model (STRIDE)

| Threat | Mitigación |
|--------|------------|
| Spoofing | Backup script solo root/container; checksums verifican integridad |
| Tampering | Backups inmutables (WORM en S3 futuro); SHA256 en metadata |
| Repudiation | Logs de backup en journal/systemd; metadata JSON inmutable |
| Information Disclosure | Backups contienen PHI → cifrado en reposo (LUKS/S3 SSE) |
| Denial of Service | Retención automática evita llenar disco; quota en storage |
| Elevation of Privilege | Script sin privilegios extra; solo read en volúmenes |

### Data Classification
- [x] PHI (datos pacientes en SurrealKV)
- [x] PII (usuarios, sesiones en Valkey)
- [x] Clinical Data (mediciones, escalas, diagnósticos)
- [x] Operational/Metadata (config, auditoría)

### Auth/Autz Requirements
- Acceso a backups: solo rol `backup-operator` (futuro RBAC)
- Cifrado en tránsito (TLS) y en reposo (AES-256)

## Testing Strategy

### Unit Tests
- [ ] `test_backup_script_syntax` (shellcheck)
- [ ] `test_retention_logic` (mock find -mtime)
- [ ] `test_metadata_json_valid` (jq schema)

### Integration Tests
- [ ] Backup + restore en staging (docker-compose)
- [ ] Verificar integridad datos: count pacientes, mediciones, escalas
- [ ] Métricas expuestas en `/metrics` tras backup

### Load Test
- [ ] Backup con 100k pacientes + 1M mediciones < 10 min

### Disaster Recovery Test (trimestral)
- [ ] Restore completo en entorno limpio < 30 min
- [ ] Validar: login, pacientes, mediciones, escalas, FHIR export

## Rollout Plan

### Feature Flag
```bash
# En .env.prod
ENABLE_BACKUP=true
BACKUP_STORAGE_PATH=/backups  # o S3 bucket
```

### Canary Deployment
| Paso | Entorno | Duración | Criterio go/no-go |
|------|---------|----------|-------------------|
| 1 | Staging (volumen local) | 7 días | 7 backups OK, retención funciona |
| 2 | Staging + restore test | 1 día | Restore < 30 min, datos íntegros |
| 3 | Prod (volumen local → S3 futuro) | — | Monitoreo 30 días |

### Rollback Procedure
1. `ENABLE_BACKUP=false` → timer desactivado
2. Backups existentes intactos en `/backups`
3. No impacto en servicio activo

## Definition of Done

- [ ] Spec aprobada en PR (1+ reviewer)
- [ ] `dmart-server/scripts/backup.sh` creado + `shellcheck` limpio
- [ ] `systemd/dmart-backup.service` + `.timer` creados
- [ ] Métricas `backup_*` expuestas en `/metrics` (textfile collector)
- [ ] `docker-compose.prod.yml` incluye volume `/backups` + sidecar cron O documentado systemd en host
- [ ] Variables `BACKUP_STORAGE_PATH`, `ENABLE_BACKUP` en `.env.prod.example`
- [ ] Tests: shellcheck en CI; restore test manual documentado
- [ ] Documentación: procedimiento restore en `docs/BACKUP_RESTORE.md`
- [ ] ROADMAP, CHANGELOG actualizados
- [ ] Deploy staging: backup diario verificado 3 días consecutivos