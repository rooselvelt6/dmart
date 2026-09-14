# SPEC-010: Restore / Disaster Recovery (DR) — heredero de SPEC-009

## Contexto
SPEC-009 fijó el **backup automático** (compose + fixture de dump + alerta `backup_failure`
verde, commit SPEC-009 DONE). Falta la otra mitad del ciclo: **restaurar** y demostrar que
el dump sirve para recuperar un sistema en DR. Sin restore probado, un backup es solo
cumplir un checklist en papel.

## Objetivo
Un procedimiento de **restore idempotente y verificable** tras SPEC-009, con integridad
comprobada por los fingerprints de SPEC-029 y un runbook de recuperación (DR) oficial.

## Finalidad
Cumplir la Ley de Buckley-Koff / RTO (objetivo de tiempo de recuperación) declarado:
**RTO ≤ 15 min** para el wire-format HL7/MLLP (SPEC-028/032) y **RPO = 0** de pérdida de
datos post-restore para el dump del día.

## Alcance
- Restore del dump producido por SPEC-009 (fixture `backup_dump.json`, fecha + fingerprint
  SPEC-029).
- Verificación **post-restore**: el fingerprint del dump restaurado === el fingerprint
  indexado en el backup (SPEC-029 `score_fingerprint`).
- Runbook DR oficial en `docs/runbook/dr.md` (RTO/RPO, pasos 1..N, verificación).

## Fuera de alcance
- Backup (SPEC-009, ya cerrado). Transición de esquema (SPEC-024+ futuro). Réplicas
  multi-cluster (espec propia en backlog).

## Definition of Done
- [ ] `specs/010-restore-dr.md` con contexto/objetivo/finalidad/alcance/DoD (este archivo).
- [ ] Fixture `dmart-server/tests/fixtures/dr_plan.json`: {backup_ref, fingerprint,
      rto_minutos:15, rpo:true, destino}
- [ ] Test `dmart-server/tests/dr_restore.rs` (SPEC-010): carga `dr_plan.json`, valida
      esquema y que fingerprint === el del SPEC-029 indexado (0 perdida).
- [ ] Runbook `docs/runbook/dr.md`: pasos de restore + verificación con fingerprints.
- [ ] Índice de runbook enlazado desde `docs/runbook/README.md`.
- [ ] Gate operativo: `cargo fmt` + `cargo clippy -p dmart-server -- -D warnings` **0/0**
      (mismo umbral que SPEC-031) + test **verde**.
- [ ] Kick-off de `cargo test -p dmart-server --test dr_restore`.

## Criterios de aceptación (gate CI SPEC-010)
```bash
cargo test -p dmart-server --test dr_restore --test runbook_check  2>&1 | grep "test result: ok"
cargo clippy -p dmart-server --lib -- -D warnings;  echo "gate=$?"
```
Umbral test result *ok* + clippy -D = 0.

## Métricas de éxito
- Tiempo de restore declarado en `dr_plan.json` (15 min) verificado por test.
- Fingerprint post-restore 100% === pre-backup (RPO 0).

## Grafo de dependencias
`SPEC-009 → SPEC-010 → (SPEC-029 fingerprint) → SPEC-011 (runbook on-call)`
