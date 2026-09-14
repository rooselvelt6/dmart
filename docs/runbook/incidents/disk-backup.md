# Incidente Disco > 80% / Backups SPEC-009 fallando

## Síntoma
Alerta disco (SPEC-008) o `backup_failure` = 1.

## Comando
```bash
df -h / ; du -sh dmart-server/target 2>/dev/null
systemctl status dmart-backup.timer   # SPEC-009
ls -lt backups/ | head   # dump de ayer con fingerprint SPEC-029
```

## Verificar
- Dump del día con fingerprint OK (SPEC-029) y `bsuccess` contador.
- Cleanup de `target/` + WAL (SPEC-030 downsampling) libera ≥ 15%.

## Escalar
- < 10% libres → pausar ingest (SPEC-031) + notify ops; RTO 15 min (SPEC-010).
