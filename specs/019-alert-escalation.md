# SPEC-019: Alert Escalation — Escalamiento y Acuse de Alertas

## Contexto
Las alertas clínicas (EWS, CDS, monitorización) se generan pero no tienen gestión de
seguimiento: nadie garantiza que una alerta crítica sea vista a tiempo. La UCI necesita
una **escalera de alertas configurable por severidad**, con acuse de recepción y
escalamiento a roles superiores si nadie responde, además de un registro temporal
consultable.

## Objetivo
Módulo `escalation` con dos tablas SurrealDB (`escalation_policies`, `escalations`) y
API REST `GET/POST /api/escalation/policies`, `GET /api/escalation/active`,
`POST /api/escalation/{id}/ack`, `POST /api/escalation/{id}/escalate` que dota a cada
alerta de un ciclo de vida `created → acknowledged → escalated → resolved` gobernado
por política por severidad.

## Alcance
- Políticas de escalamiento configurables por severidad (`low/medium/high/critical`)
  con `max_response_minutes` (default razonable fijo), `timeout_minutes`, `target_role`
  (`Dr`/`Enfermera`) y `enabled`. Upsert idempotente por severidad (clave estable, única).
- `create_escalation(db, paciente, alert_type, severity)`: deriva `policy_severity` de la
  política vigente, arranca en nivel 1, persiste en `escalations` y publica el evento en
  el canal realtime `escalation`. Registra opcionalmente un `EventType::Alert` en la
  timeline del paciente (SPEC-015).
- `acknowledge_escalation(id)`: marca `acknowledged` con `acknowledged_at` RFC3339
  (solo si no está ya acusada o resuelta).
- `escalate_escalation(id)`: sube `level` (+1) y marca `escalated` con `escalated_at`
  (no admite resueltas).
- `resolve_escalation(id)`: marca `resolved` con `resolved_at` (fuera de la API, helper
  de Store para pruebas y cierres programáticos).
- `active_escalations`: incidentes con `status != resolved`, ordenados por severidad
  desc / `created_at` asc.

## Fuera de alcance
- Disparo automático por cron del escalamiento (el incremento se dispara por API).
- Integración con notificadores externos (pager, SMS) y asignación real de usuarios.
- Modificación/borrado de escalaciones históricas (append-only + transiciones).

## Definition of Done
- [ ] `specs/019-alert-escalation.md` (este archivo)
- [ ] Migración `dmart-server/migrations/019_alert_escalation.surql` (tablas SCHEMAFULL +
      `DEFINE FIELD` + `DEFINE INDEX`; defaults sembrados por severidad)
- [ ] Modelos `Severity`, `EscalationStatus`, `EscalationPolicy`, `Escalation` en
      `src/escalation.rs` + Store (`list_policies`, `upsert_policy`, `create_escalation`,
      `acknowledge_escalation`, `escalate_escalation`, `resolve_escalation`,
      `active_escalations`)
- [ ] Handlers HTTP en `src/api/escalation.rs` con firmas fijas
- [ ] Eventos realtime en canal `escalation` (crear/acusar/escalar/resolver)
- [ ] Test `dmart-server/tests/escalation.rs` (políticas sembradas, upsert por severidad,
      ciclo de vida HTTP, orden de activos, exclusión de resueltas, 404/409)
- [ ] `cargo fmt` + `cargo clippy -p dmart-server --lib --bin dmart-server -- -D warnings` = 0/0

## Gate CI SPEC-019
```bash
cargo test -p dmart-server --test escalation 2>&1 | grep "test result: ok"
cargo clippy -p dmart-server --lib --bin dmart-server -- -D warnings 2>&1 | grep '^error' | wc -l  # = 0
```

## Métricas
- `active_escalations` p95 < 50 ms (200 escalaciones activas)
- Una única política por severidad garantizada por índice `UNIQUE`
- Cero trabajos de escalamiento manual: nivel inicial 1, +1 por `escalate`