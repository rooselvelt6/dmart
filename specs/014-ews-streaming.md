//! SPEC-014: Clinical Early-Warning Streaming (EWS) — real-time vital signs SSE/WebSocket
//! Extiende SPEC-005 (Prometheus) + SPEC-013 (FHIR) + SPEC-028 (HL7 vitals).

# SPEC-014: Early-Warning Streaming (EWS) — real-time vital signs

## Contexto
Los monitores ICU (MLLP HL7 + FHIR bundles SPEC-013) ingieren vital signs cada 1–5 s.
El clínico necesita **streaming en tiempo real** de scores (Apache II, SOFA, NEWS2) y
alertas de deterioro (EWS ≥ 5) sin polling. El módulo `realtime` ya existe (SSE hub).

## Objetivo
Pipeline streaming que:
- Consume `VitalsMessage` de `ingest_vitals` (HL7) y `fhir_bundle` (SPEC-013).
- Calcula **scores en memoria** (Apache II, SOFA, NEWS2) con `dmart_shared::scales`.
- Publica eventos SSE (`/api/realtime/stream`) por paciente + alerta EWS ≥ threshold.
- Idéntico backpressure SPEC-031 (cola bounded, métricas `hl7_messages_processed`).

## Alcance
- Módulo `pub mod ews_stream;` en lib.rs (allow SPEC-031 doc).
- `EwsEngine` : `VitalsMessage → ScoreEvent { patient_id, score, algo, timestamp, fingerprint }`.
- Integración con `realtime::RealtimeHub` (SSE) + métricas Prometheus `ews_score_published_total`.
- Config: thresholds EWS (NEWS2 ≥ 5, Apache II cambio ≥ 5 pts) vía env.
- Test `dmart-server/tests/ews_streaming.rs`: ingiere 100 vitals → 100 ScoreEvent SSE.

## Fuera de alcance
- Notificaciones push móvil / webhook (backlog).
- ML predictivo (SPEC-002 model persistence separado).

## Definition of Done
- [ ] `specs/014-ews-streaming.md` (este archivo).
- [ ] Módulo `ews_stream` en lib con `EwsEngine` + `pub fn spawn_engine(...) → JoinHandle<()>`.
- [ ] Endpoint SSE reusa `/api/realtime/stream` (ya en `api/realtime.rs`).
- [ ] Métricas Prometheus: `ews_score_published_total{algo,severity}`.
- [ ] Test `ews_streaming.rs`: 100 vitals → 100 ScoreEvent SSE recibidos < 5 s.
- [ ] `cargo fmt` + `clippy -p dmart-server --lib --bin dmart-server -- -D warnings` = 0/0.
- [ ] Cobertura SPEC-027 no decrece; score_parser escala ≥ 90% (SPEC-027).

## Gate CI SPEC-014
```bash
cargo test -p dmart-server --test ews_streaming --test fhir_conformance 2>&1 | grep "test result: ok"
cargo clippy -p dmart-server --lib --bin dmart-server -- -D warnings 2>&1 | grep '^error' | wc -l  # = 0
```

## Métricas
- Latencia ingest→SSE < 100 ms p95.
- 0 pérdida de eventos (backpressure SPEC-031 + bounded channel).
- Alertas EWS ≥ 5 disparan runbook SPEC-011 `mllp-down`/`backpressure`.