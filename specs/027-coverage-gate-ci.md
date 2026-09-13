# SPEC-027: Coverage Gate en CI (cargo llvm-cov)

> Elevada a **Crítica** (2026-09-13): sin este gate, cada commit futuro puede
> degradar la cobertura de interoperabilidad/clínica sin que CI lo detecte.

## Contexto
- **Problema**: se midió cobertura HL7 localmente (parser 96.5%, mllp 93.8%, ingest 91.8%) pero **no hay gate en CI**. El DoD de SPEC-003 pide ">90%" pero nadie lo vuela en cada push.
- **Usuario objetivo**: Backend Engineer / QA
- **Métrica de éxito (KPI)**: CI falla si cobertura de módulos protegidos cae bajo umbral; cobertura documentada en cada pipeline.

## Acceptance Criteria (Gherkin)

```gherkin
Feature: Coverage Gate en CI
  As a QA Engineer
  I want coverage thresholds enforced automatically on every push/PR
  So that regressions en interoperabilidad y lógica clínica se detecten al instante

  Scenario: Regresión de cobertura HL7
    Given un cambio que baja parser.rs por debajo del 90%
    When el job `coverage` corre en CI
    Then el job falla con reporte de líneas descubiertas
    And el PR queda bloqueado hasta cubrir el umbral

  Scenario: Cobertura dentro de umbral
    Given un cambio sin impacto en cobertura
    When el job `coverage` corre
    Then pasa y sube reporte LCOV como artifact

  Scenario: umbrales configurados por módulo
    Given la tabla de umbrales definida en `ci.yml`
    When cambia la tolerancia
    Then se modifica en un solo punto de configuración
```

## Umbrales por módulo (tabla única)

| Módulo | Cobertura de línea mínima | Estado actual |
|--------|---------------------------|---------------|
| `hl7/parser.rs` | 90% | 96.5% ✅ |
| `hl7/mllp.rs` | 90% | 93.8% ✅ |
| `hl7/ingest.rs` | 90% | 91.8% ✅ |
| `shared/scales.rs` | 85% | 83.4% ⚠️ (añadir tests) |
| `shared/validation.rs` | 85% | 81.3% ⚠️ (añadir tests) |
| `shared/ml.rs` | 80% | 94.6% ✅ |

## Implementation Notes
- Tooling: `rustup component add llvm-tools-preview` + `cargo llvm-cov` 0.9.1 (binario `~/.cargo/bin`). Ya validado localmente.
- Set local: `source scripts/coverage.sh` con alias `cov` (lib + `--test hl7_integration` + `--test api_tests`).
- CI: job `coverage` que corre:
  ```bash
  cargo llvm-cov --workspace --test hl7_integration --test api_tests --lib --fail-under-lines 85
  ```
  y un script `scripts/check-coverage-thresholds.sh` con la tabla de umbrales por módulo
  (parsea el reporte LCOV; falla con `exit 1` si algún módulo baja del umbral).
- Subir `lcov.info` como artifact → útil para coveralls/codecov si se añade después.

## API Contracts
N/A — infra de CI, sin cambios de API.

## Data Models
N/A — sin cambios de schema.

## Edge Cases
| # | Caso | Comportamiento |
|---|------|----------------|
| 1 | job corre sin el componente `llvm-tools-preview` | `actions-rs`/`rustup component add` antes de correr |
| 2 | Múltiples crates en el workspace | umbrales se evalúan por crate/módulo, no global |
| 3 | Threshold flaky por merges de tests | usar `--fail-under-lines` global como red de seguridad + tabla explícita |

## Security Considerations
- `lcov.info` NO debe publicarse como artifact público (rutas internas); mantener en artifact interno de Actions.

## Testing Strategy
- [ ] CI job verde tras SPEC-003
- [ ] Test negativo: bajar temporalmente una cobertura → job falla
- [ ] `scales.rs` y `validation.rs` llegan a 85% (expectativa: +8 tests unitarios)

## Rollout Plan
- Añadir al job `coverage` dentro de `ci.yml`, en `needs:` de `notify` y `release-build`.
- **SPEC-007** lo absorbe como parte del pipeline completo.

## Definition of Done
- [ ] Spec aprobada
- [ ] Job `coverage` en `ci.yml` con `--fail-under-lines` + script de umbrales por módulo
- [ ] `scales.rs`/`validation.rs` por encima de 85%
- [ ] Test negativo del gate (reducción artificial → fail) documentado
- [ ] CHANGELOG.md y ROADMAP actualizados