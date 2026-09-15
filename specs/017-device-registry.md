# SPEC-017: Device Registry — Registro y Estado de Dispositivos Clínicos

## Contexto
La UCI conecta dispositivos clínicos (monitores de cabecera, ventiladores, bombas de infusión) que emiten datos vía HL7/MLLP (SPEC-003) y streamings (SPEC-014). Hoy no existe un inventario centralizado que diga *qué* dispositivo está *dónde*, *en qué firmware* y *si está vivo*. El equipo clínico y biomédico necesita saber en tiempo real qué dispositivos están operativos, en mantenimiento o caídos para asignar camas/equipos y responder a fallos.

## Objetivo
API REST de *Device Registry*: registrar dispositivos clínicos y seguir su ciclo de vida de conectividad (heartbeat → online/offline con ventana de tiempo) y operatividad (mantenimiento), con resumen agregado por estado y por tipo.

## Alcance
- Módulo `device_registry` en lib (modelos + Store de la tabla `device_registry`)
- Tabla `device_registry` en SurrealDB (SCHEMAFULL): tipo, fabricante, modelo, firmware, serial (único), cama opcional (`cama_id` ↔ tabla `camas`), ubicación libre, estado (`online`/`offline`/`mantenimiento`), `registered_at`, `last_seen_at`, `heartbeat_interval_secs`
- Heartbeat: actualiza `last_seen_at` y marca `online`/`offline` según la ventana de tiempo (3× el intervalo nominal, mínimo 60s); mantenimiento preserva su estado
- Endpoints:
  - `GET /api/devices?estado=online|offline|mantenimiento` — listado con filtro opcional
  - `POST /api/devices` — registro (valida campos requeridos y enum de estado)
  - `GET /api/devices/{id}` — detalle (404 si no existe)
  - `POST /api/devices/{id}/heartbeat` — renueva `last_seen_at` y publica evento en realtime
  - `GET /api/devices/status` — summary agregado (`GROUP BY` estado y tipo + total)
- Publicación en realtime de eventos `device` (registro y heartbeat) vía hub SSE
- Tests de integración sobre store y sobre el router HTTP autenticado

## Fuera de alcance
- Escritura/lectura de señales fisiológicas desde los monitores (SPEC-003/SPEC-014)
- Inventario biomédico completo (mantenimiento preventivo, historial de partes, alarmas propias de cada equipo)
- Autenticación por dispositivo (certificados); los endpoints usan la autenticación RBAC existente
- HIPAA/auditoría por dispositivo (opcional, no bloqueante)

## Definition of Done
- [ ] `specs/017-device-registry.md` (este archivo)
- [ ] `src/device_registry.rs`: modelos `ClinicalDevice`, `RegisterDeviceInput`, `DeviceState`, summary (`por_estado`/`por_tipo`)
- [ ] Migración `017_device_registry.surql`: tabla SCHEMAFULL + campos con TYPE + índices `DEFINE INDEX IF NOT EXISTS` (estado, tipo, serial UNIQUE, cama, last_seen)
- [ ] Store: `register`, `get`, `list` (filtro estado), `heartbeat`, `status_summary`
- [ ] Heartbeat con ventana: `reconcile_stale_devices` marca `offline` por `last_seen_at < now − 3×interval` (respetando `mantenimiento`)
- [ ] Handlers HTTP en `src/api/registry.rs` con firmas fijadas: `list_devices`, `register_device`, `get_device`, `heartbeat_device`, `device_status_summary`
- [ ] Realtime `publish("device", …)` en registro y heartbeat
- [ ] Test `dmart-server/tests/device_registry.rs` (store + HTTP: registro, get, heartbeat, 404, 400, filtro, summary, auth)
- [ ] `cargo fmt -p dmart-server` + `cargo clippy -p dmart-server --lib --bin dmart-server -- -D warnings` = 0 warnings

## Gate CI SPEC-017
```bash
cargo test -p dmart-server --test device_registry 2>&1 | grep "test result: ok"
cargo clippy -p dmart-server --lib --bin dmart-server -- -D warnings 2>&1 | grep '^error' | wc -l  # = 0
```

## Métricas
- Listado/summary < 50 ms p95 (tabla indexada por estado/tipo)
- Reconciliación offline idempotente y batch (una sola actualización por pasada)
- 100 % de dispositivos con serial único y ventana de heartbeat configurable