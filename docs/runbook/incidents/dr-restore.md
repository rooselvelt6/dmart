# Incidente Restore / DR (SPEC-010)

## Síntoma
Incidente mayor: datos perdidos o bin corrupto; se ejecuta el plan de DR.

## Comando
```bash
# pasar a modo mantenimiento (SPEC-004 RBAC on-call)
dmart_ops dr run --plan dr_plan.json          # 6 pasos del runbook
cargo test -p dmart-server --test dr_restore  # gate de integridad post-restore
```

## Verificar
- Dump restaurado: `fingerprint` post-restore === `fingerprint` SPEC-029 (RPO 0).
- `/obs/health` 200 + 63 tests verdes + clippy -D 0 (SPEC-031).

## Escalar
- RTO excedido 15 min → incidente auto-declarado mayor, págines a 2× nivel.
