# SPEC-003: HL7 MLLP Integration Tests (tokio-test mock streams)

## Contexto
- **Problema**: Parser HL7 v2 y framer MLLP tienen tests unitarios pero faltan tests de integración end-to-end con streams TCP simulados. Cobertura actual ~60%, objetivo >90% para código crítico de interoperabilidad hospitalaria.
- **Usuario objetivo**: Backend Engineer / QA
- **Métrica de éxito (KPI)**: Cobertura HL7 parser + MLLP framer > 90%, 0 regresiones en CI

## Acceptance Criteria (Gherkin)

```gherkin
Feature: HL7 MLLP Integration Tests
  As a Backend Engineer
  I want integration tests for HL7 parser + MLLP framer
  So that regressions in interoperability are caught early

  Scenario: Valid ORU^R01 message parsed end-to-end
    Given a TCP connection with valid ORU^R01 HL7 message
    When the MLLP framer parses the stream
    And the HL7 parser extracts vitals
    Then vitals contain correct LOINC codes, values, timestamps
    And patient_ref is resolved correctly

  Scenario: Malformed HL7 message rejected gracefully
    Given a TCP connection with malformed HL7 (missing MSH, invalid segment)
    When the MLLP framer + parser process it
    Then error is logged, ACK NAK sent, no panic

  Scenario: Multiple messages in single stream
    Given a TCP stream with 5 concatenated HL7 messages (VT/FS/CR delimited)
    When the framer processes the stream
    Then all 5 messages are parsed independently
    And each produces correct vitals output

  Scenario: Vendor detection (Mindray/Philips) works
    Given HL7 messages with MSH.3 = "MINDRAY" or "PHILIPS"
    When detect_vendor is called
    Then correct MonitorSource enum returned

  Scenario: ACK generation (AA/AR/AE) correct
    Given a valid HL7 message processed
    When ingest_vitals succeeds
    Then ACK AA (accept) returned
    And given invalid message
    Then ACK AR (reject) or AE (error) returned
```

## API Contracts

### Test Infrastructure (no public API changes)

```rust
// test infrastructure in dmart-server/tests/hl7_integration.rs
mod hl7_integration {
    use tokio_test::io::Builder;  // mock TCP stream
    use dmart_server::hl7::{parser, mllp, ingest};
    
    // Helper: build mock stream from HL7 string
    fn mock_stream(hl7_msg: &str) -> Builder;
    
    // Test cases covering:
    // - parse_oru_message + mllp::parse_via_mllp
    // - ingest_vitals with mocked DB
    // - detect_vendor for Mindray/Philips/Unknown
    // - build_ack for AA/AR/AE
}
```

## Data Models
N/A — Solo tests, no cambios de schema.

## Edge Cases
| # | Caso | Comportamiento esperado |
|---|------|------------------------|
| 1 | Stream truncado (conexión cortada) | Framer detecta EOF, error graceful |
| 2 | Mensajes mezclados válidos/inválidos | Cada uno procesado independientemente |
| 3 | Caracteres de escape HL7 (\F\, \R\, \S\, \T\, \E\) | Parsed correctamente |
| 4 | Sub-componentes (OBX-5 con ^~) | Parseados a Vec<Vec<String>> |
| 5 | Timestamps HL7 variados (con/sin zona, milisegundos) | Convertidos a RFC3339 correctamente |

## Security Considerations
- **Input validation**: Tests de fuzzing ya cubren (SPEC-001 fuzz_hl7_parser)
- **DoS via stream**: Test stream muy largo (10MB+) → timeout/limit
- **PHI en logs**: Verificar que tests no logean PHI real

## Testing Strategy

### Integration Tests (tokio-test)
- [ ] `test_valid_oru_end_to_end()` — mock stream → parser → vitals
- [ ] `test_malformed_rejected()` — invalid MSH → ACK AR
- [ ] `test_multiple_messages_stream()` — 5 mensajes en un stream
- [ ] `test_vendor_detection_mindray_philips()` — MSH.3 parsing
- [ ] `test_ack_generation_aa_ar_ae()` — ingest_vitals + build_ack
- [ ] `test_hl7_datetime_conversion_variants()` — formatos fecha
- [ ] `test_escape_sequences()` — \F\ \R\ etc.

### Property-Based (proptest) — ya existe en SPEC-001
- [ ] `parse_malformed_hl7_does_not_panic` — robustez

### Fuzzing (cargo-fuzz) — ya existe en SPEC-001
- [ ] `fuzz_hl7_parser` target

### Coverage Target
- [ ] `cargo llvm-cov --workspace --html` → HL7 modules > 90%

## Rollout Plan
- **Feature Flag**: N/A (tests only)
- **CI**: Añadir job `hl7-integration-test` en `.github/workflows/ci.yml`
- **Gate**: Merge bloqueado si coverage HL7 < 90%

## Definition of Done
- [ ] Spec aprobada
- [ ] `dmart-server/tests/hl7_integration.rs` creado con 7+ tests
- [ ] `cargo test hl7_integration` pasa
- [ ] Coverage HL7 parser + MLLP > 90%
- [ ] CI job añadido y pasando
- [ ] CHANGELOG.md actualizado