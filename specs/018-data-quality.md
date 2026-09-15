# SPEC-018: Data Quality — Validación de Calidad del Dato Vital

## Contexto
Los monitores de cama (Mindray, Philips) envían decenas de mensajes HL7 por
minuto, y parte del flujo contiene **artefactos** (rangos fisiológicos
imposibles, valores NaN/Inf, timestamps corruptos, referencias de paciente
vacías o secuencias fuera de orden). Ingerir esos datos sin control contamina
el registro clínico y los scores de severidad.

## Objetivo
Endpoint `POST /api/data-quality/validate` que valida un `VitalsMessage`,
persiste los issues detectados en la tabla append-only `quality_events` y los
expone mediante reporte (`GET /api/data-quality/report`) y agregación
(`GET /api/data-quality/summary`).

## Alcance
- Módulo `data_quality` en lib con validadores por mensaje:
  - Rango fisiológico por vital (HR 20-250 bpm, SpO2 50-100 %, Temp 30-45 °C,
    RR 4-80, BP sistólica 40-300, más PAD/PAM) → issue **alta**
  - `value` no finito (NaN/Inf) → issue **alta**
  - Timestamp en el futuro o demasiado viejo (>10 min) → issue **media**
  - `patient_ref` vacío → issue **alta**
  - `sequence_number` duplicado/retrocediendo por sender → issue **media**
- Issue = `id, message_id, patient_ref, severity (low/medium/high), code, detail, timestamp`
- Migración `018_data_quality.surql`: tabla `quality_events` SCHEMAFULL + índices
- Handlers `validate_message`, `quality_report`, `quality_summary` (ruta `/api/data-quality/*`)
- `validate_message` NO ingesta measurements ni altera patients

## Fuera de alcance
- Reparación/reintento de los mensajes con issues (acción correctiva)
- Deduplicación cross-sender o detector de gaps persistido (longitudinal)
- Configuración dinámica de rangos (hardcoded por ahora)

## Definition of Done
- [ ] `specs/018-data-quality.md` (este archivo)
- [ ] `pub mod data_quality;` en `lib.rs`
- [ ] Migración `018_data_quality.surql` (tabla SCHEMAFULL + `DEFINE INDEX IF NOT EXISTS`)
- [ ] Modelo `QualityIssue` + validadores de rango/no-finito/timestamp/patient_ref/secuencia
- [ ] Store `quality_events` (persistencia append-only)
- [ ] Handler `POST /api/data-quality/validate` con `ApiResponse::ok(issues)` (o err si no parseable)
- [ ] Handler `GET /api/data-quality/report` (últimos issues, `timestamp` desc, default 100, paginado)
- [ ] Handler `GET /api/data-quality/summary` (`GROUP BY severity` y por `code`)
- [ ] Test `dmart-server/tests/data_quality.rs` (validadores, persistencia, reporte, agregación, HTTP)
- [ ] `cargo fmt` + `cargo clippy -p dmart-server --lib --bin dmart-server -- -D warnings` = 0/0
- [ ] Cobertura SPEC-027 no decrece

## Gate CI SPEC-018
```bash
cargo test -p dmart-server --test data_quality 2>&1 | grep "test result: ok"
cargo clippy -p dmart-server --lib --bin dmart-server -- -D warnings 2>&1 | grep '^error' | wc -l  # = 0
```

## Métricas
- Latencia de validación + persistencia p95 < 20 ms / mensaje
- 100 % de mensajes validados en el flujo HTTP
- Reporte y resumen consultables de forma determinista por `timestamp`