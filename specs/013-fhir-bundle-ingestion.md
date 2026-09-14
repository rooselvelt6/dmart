# SPEC-013: FHIR R4 Bundle ingestion + mapping HL7 v2 (SPEC-028/031)

## Contexto
El ICU UCI recibe ahora bundles FHIR R4 (expedientes, observaciones, recursos Patient/Observation)
desde sistemas EHR externos. El pipeline HL7 legacy (SPEC-028/032) ya ingiere ORU^R01 vía
MLLP; SPEC-013 añade la **ruta FHIR-native** sin romper la superficie legacy.

## Objetivo
Endpoint `POST /api/fhir/Bundle` que:
- Valida bundle R4 (tipo `transaction` | `batch` | `collection`).
- Convierte `Observation` (vital signs) → `VitalsMessage` del parser HL7 (SPEC-028).
- Reutiliza `ingest_vitals` + `parse_oru_message` flow → métricas SPEC-031 idénticas.
- Devuelve `Bundle` de respuesta con `OperationOutcome` por recurso.

## Alcance
- Módulo `fhir_bundle` en lib (gate SPEC-031: `#[allow(dead_code)]` documentado; ejercitado
  por test de integración + conformance FHIR).
- Parser `Bundle` → `Vec<VitalsMessage>` usando `dmart_shared::models` + `scales`.
- Endpoint en `api/fhir.rs` (ya existe esqueleto en 004/027) extendido.
- Test de conformance FHIR en `tests/fhir_conformance.rs` (≥ 10 recursos válidos).

## Fuera de alcance
- Suscripciones FHIR / websockets (SPEC-014 cubre streaming).
- Mapeo inverso FHIR→HL7 outbound (backlog).

## Definition of Done
- [ ] `specs/013-fhir-bundle-ingestion.md` (este archivo).
- [ ] Módulo `pub mod fhir_bundle;` en `lib.rs` con allow SPEC-031 documentado.
- [ ] `api/fhir.rs`: `POST /api/fhir/Bundle` + validación R4 + respuesta `OperationOutcome`.
- [ ] Conversión `Observation` (LOINC 8867-4 HR, 9279-1 RR, 2708-6 SpO2, 8310-5 Temp) →
      `VitalsMessage` idéntico al path HL7 MLLP (reuso `ingest_vitals`).
- [ ] Test `dmart-server/tests/fhir_conformance.rs` (≥ 10 bundles válidos, recursos
      Patient+Observation; gate = test verde + clippy -D 0/0).
- [ ] `cargo fmt` + `clippy -p dmart-server --lib -- -D warnings` = 0 + `--bin` = 0.
- [ ] Cobertura llvm-cov (SPEC-027) no decrece global 65.0.

## Gate CI SPEC-013
```bash
cargo test -p dmart-server --test fhir_conformance --test hl7_integration 2>&1 | grep "test result: ok"
cargo clippy -p dmart-server --lib --bin dmart-server -- -D warnings 2>&1 | grep '^error' | wc -l  # = 0
```

## Métricas
- Latencia p95 < 200 ms por bundle (≤ 100 recursos).
- 100% recursos Observation mapeados a VitalsMessage (0 pérdida).
- Fingerprint SPEC-029 en cada Observation persistida.