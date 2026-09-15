# SPEC-020: Tele-ICU — Sesiones de monitorización remota

## Contexto
La UCI opera con equipos críticos reducidos en horas nocturnas. Un especialista
remoto (tele-ICU) necesita abrir sesiones de telemedicina sobre pacientes
conectados al sistema para **monitorizarlos en vivo**: ver una foto clínica
actual (última medición + timeline reciente), saber la cama donde está el
paciente y poder cerrar la consulta registrando su duración.

## Objetivo
Tabla y API REST de sesiones de tele-ICU:

- `POST /api/teleicu/sessions` — abre una sesión activa (`video`/`chat`) para un paciente.
- `GET /api/teleicu/sessions` — sesiones activas primero, luego recientes por `started_at` desc.
- `POST /api/teleicu/sessions/{id}/end` — cierra la sesión con `ended_at` + duración en minutos.
- `GET /api/teleicu/live/{id}` — snapshot en vivo del paciente (datos, cama, última medición, timeline de 10 eventos).

## Alcance
- Módulo `teleicu` en lib: modelo `TeleIcuSession`/`SessionStatus` + Store `teleicu_sessions` (SCHEMAFULL)
- Campos: `id`, `session_id` (UUID v4 = RecordId, único), `patient_id`, `specialist_id`, `channel`, `status` (`active`/`ended`), `started_at` (RFC3339), `ended_at` (RFC3339, nullable), `notes` (opcional)
- Reutilización real: `db::get_patient`, `db::get_last_measurement`, `patient_timeline::query_timeline` (default 10)
- Eventos realtime en canal `"teleicu"` al abrir/cerrar sesión (`realtime::publish`)
- Rutas ya registradas en `api/mod.rs` (los handlers se montan en dichas rutas)

## Fuera de alcance
- Streaming audiovisual / WebRTC (la API gestiona la sesión, no el media)
- Confirmación de presencia en vivo, SLA de respuesta del especialista (SPEC-xxx futura)
- Historial agregado de duración por especialista (reporting/métricas)

## Decisiones de diseño
- **Sesión activa duplicada → `409 Conflict`**: no se reutiliza la sesión existente; el cliente la reusa explícitamente con el `session_id` devuelto en el `201`.
- **`end_session` sobre sesión ya finalizada → `409 Conflict`** (idempotencia explícita, no silenciosa).
- **`live_view` → `404`** cuando el paciente no existe **o** no tiene una sesión activa (se documenta como requisito: el snapshot solo tiene sentido bajo sesión abierta).
- El `{id}` de `end_session` es el `session_id` (RecordId de la tabla, como `patient_id` en `patients`).
- Errores de body → `400`; paciente inexistente al abrir → `404`.

## Definition of Done
- [ ] `specs/020-teleicu.md` (este archivo)
- [ ] `pub mod teleicu;` en `lib.rs` con `allow(dead_code)` (andamiaje existente)
- [ ] Migración `dmart-server/migrations/020_teleicu.surql` (SCHEMAFULL + índices)
- [ ] Modelo `TeleIcuSession` + `SessionStatus` en `src/teleicu.rs`
- [ ] Store `create_session` / `get_session` / `find_active_session_for_patient` / `list_sessions` / `close_session` / `duration_minutes`
- [ ] Handlers `start_session`, `list_sessions`, `end_session`, `live_view` en `src/api/teleicu.rs` (firmas de andamiaje intactas)
- [ ] Publish realtime `"teleicu"` en open/close
- [ ] Tests `dmart-server/tests/teleicu.rs` (Store + E2E HTTP con auth/RBAC reales)
- [ ] `cargo fmt` + `cargo clippy -p dmart-server --lib --bin dmart-server -- -D warnings` = 0/0

## Gate CI SPEC-020
```bash
cargo check -p dmart-server --lib
cargo test -p dmart-server --test teleicu 2>&1 | grep "test result: ok"
cargo clippy -p dmart-server --lib --bin dmart-server -- -D warnings 2>&1 | grep '^error' | wc -l  # = 0
```

## Métricas
- Latencia de `live_view` < 50 ms (reutiliza lecturas cacheadas de measurements)
- Una sola sesión activa por paciente (índice `patient_id, status`)
- 100 % de sesiones con `started_at`/`ended_at` RFC3339