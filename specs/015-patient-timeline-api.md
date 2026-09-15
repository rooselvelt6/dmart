# SPEC-015: Patient Timeline API — Historial Longitudinal Event-Sourced

## Contexto
El modelo `Patient` actual es estático (CRUD). La UCI requiere **timeline completa**: ingresos, eventos vitales, scores, intervenciones, medicación, procedimientos, notas, resultados — todo ordenado temporalmente y consultable por rango.

## Objetivo
API REST `GET /api/patients/{id}/timeline` que devuelve **stream ordenado de eventos** (event sourcing) con:
- Filtros por tipo, rango temporal, severidad
- Paginación cursor-based
- Fingerprint SPEC-029 por evento
- Integración FHIR `Patient.observation`, `Condition`, `Procedure`, `MedicationStatement`

## Alcance
- Módulo `patient_timeline` en lib (allow SPEC-031 documentado)
- Tabla `patient_event` en SurrealDB (append-only, índice temporal)
- Tipos de evento: `Admission`, `VitalSigns`, `ScoreCalculated`, `Intervention`, `Medication`, `Procedure`, `Note`, `Discharge`, `Alert`
- Endpoint `GET /api/patients/{id}/timeline?since=&until=&type=&severity=&cursor=&limit=`
- Serialización FHIR `Bundle` (type=history) opcional via `Accept: application/fhir+json`

## Fuera de alcance
- Edición/borrado de eventos históricos (append-only)
- Proyecciones ML sobre timeline (SPEC-032)

## Definition of Done
- [ ] `specs/015-patient-timeline-api.md` (este archivo)
- [ ] `pub mod patient_timeline;` en `lib.rs` con allow SPEC-031
- [ ] Migración `DMART_015_patient_events.surql` (tabla + índices)
- [ ] Modelo `PatientEvent` + `EventType` enum + serialización FHIR
- [ ] Handler `GET /api/patients/{id}/timeline` con filtros + paginación cursor
- [ ] Test `dmart-server/tests/patient_timeline.rs` (≥ 10 eventos, filtros, cursor, FHIR bundle)
- [ ] `cargo fmt` + `cargo clippy -p dmart-server --lib --bin dmart-server -- -D warnings` = 0/0
- [ ] Cobertura SPEC-027 no decrece

## Gate CI SPEC-015
```bash
cargo test -p dmart-server --test patient_timeline --test api_tests 2>&1 | grep "test result: ok"
cargo clippy -p dmart-server --lib --bin dmart-server -- -D warnings 2>&1 | grep '^error' | wc -l  # = 0
```

## Métricas
- Latencia p95 < 50 ms (timeline 100 eventos)
- Cursor estable bajo concurrencia
- 100 % eventos con fingerprint SPEC-029