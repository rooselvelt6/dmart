# SPEC-011: Runbook / On-Call operativo oficial

## Contexto
SPEC-008=alertas operativas, SPEC-009=backup, SPEC-010=restore/DR. Faltan **quién** y
**cómo** cuando suena una alerta: el runbook de on-call que convierte un `pager` en un
paso accionable, con SLA de respuesta y camino de escalado. Sin runbook, una alerta es
ruido; con runbook, es un checklist.

## Objetivo
Runbook oficial por incidente con **gate por incidente** (qué verificar, qué medir, qué
comando ejecutar, cuándo escalar) que la CI valida que existe para TODOS los incidentes
del compendio.

## Finalidad
- Reducir el MTTR a < 15 min por incidente tipificado (mismo RTO que SPEC-010).
- Que el on-call NO improvise bajo presión: cada alerta tiene un camino numerado,
  verificado y versionado.

## Alcance
- Incidentes a cubrir (los tipificados por SPEC-008/009/010/031):
  1. MLLP/Hl7 caído (`hl7_integration` rojos).
  2. Backpressure SPEC-031 activado (cola ingest = full, `ingest_error_avg_set` > 0).
  3. Disco > 80% / backups fallando (SPEC-009).
  4. Restore en curso (SPEC-010) — paso a paso DR.
  5. MFA/on-auth lockout manual (SPEC-004) — desbloqueo controlado.
- Formato: un `.md` por incidente + índice `docs/runbook/README.md`, cada uno con
  secciones: **Síntoma · Comando · Verificar · Escalar**.

## Fuera de alcance
- Implementar la corrección automática (eso lo hace el código). DR multicluster
  (backlog).

## Definition of Done
- [ ] `specs/011-runbook-oncall.md` (este archivo, numerada y finalizada).
- [ ] `docs/runbook/incidents/{mllp-down,backpressure,disk-backup,dr-restore,auth-lockout}.md`
      — 5 incidentes, cada uno con síndrome/comando/verificación/escalado.
- [ ] Test `dmart-server/tests/runbook_check.rs`: lee el índice y **valida que los 5
      ficheros existen Y cada uno contiene las 4 secciones** (gate por incidente).
- [ ] Enlazado desde `docs/runbook/README.md` (índice) — integra con SPEC-010.
- [ ] `cargo fmt` + `cargo clippy -p dmart-server --bin dmart-server -- -D warnings`
      = 0 (mismo gate SPEC-031).
- [ ] `cargo test -p dmart-server --test runbook_check` → **test result: ok**.

## Criterios de aceptación (gate CI SPEC-011)
```bash
cargo test -p dmart-server --test runbook_check --test dr_restore 2>&1 | grep "test result: ok"
cargo clippy -p dmart-server --bin dmart-server -- -D warnings 2>&1 | grep '^error' | wc -l   # = 0
```

## Métricas
- MTTR tipificado ≤ 15 min (RTO SPEC-010).
- 5/5 incidentes con runbook validado por CI (gate por incidente 100%).

## Grafo
`SPEC-008 → SPEC-009 → SPEC-010 → SPEC-011` (serie operativa; DR+on-call en paralelo,
ambas dependen solo de 009/008 que ya están DONE).
