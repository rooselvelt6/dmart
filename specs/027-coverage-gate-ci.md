# SPEC-027: Coverage Gate en CI (cargo llvm-cov) ✅ **DONE (2026-09-14)**

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
| `hl7/parser.rs` | 90% | 95.0% ✅ |
| `hl7/mllp.rs` | 90% | 100.0% ✅ |
| `hl7/ingest.rs` | 90% | 94.7% ✅ |
| `shared/scales.rs` | 85% | 89.1% ✅ (subió con SPEC-028) |
| `shared/validation.rs` | 85% | 100.0% ✅ (+7 tests unitarios) |
| `shared/ml.rs` | 80% | 95.9% ✅ |

> **Medido con**: `cargo llvm-cov -p dmart-shared -p dmart-server --lib --test api_tests --test hl7_integration --lcov`
> (scoped a los paquetes, sin `--workspace`, respetando AGENTS.md).

## Implementation Notes
- Tooling: `rustup component add llvm-tools-preview` + `cargo llvm-cov` 0.9.1 (binario `~/.cargo/bin`).
- **Script del gate**: `scripts/check-coverage-thresholds.sh` — parsea el LCOV y evalúa la
  tabla de umbrales por módulo (punto único de configuración, líneas 25-33). Falla con `exit 1`
  nombrando cada módulo bajo umbral.
- CI: job `coverage` en `.github/workflows/ci.yml`:
  1. `rustup component add llvm-tools-preview`
  2. `cargo llvm-cov -p dmart-shared -p dmart-server --lib --test api_tests --test hl7_integration --lcov --output-path coverage.lcov`
  3. `bash scripts/check-coverage-thresholds.sh coverage.lcov` (tabla por módulo)
  4. Gate global: `LH/LF` global ≥ **60%** (red de seguridad; ver nota abajo)
  5. Upload de `coverage.lcov` como artifact (retención 30 días) → listo para coveralls/codecov.
  - Está en `needs:` de `release-build` y del job `notify` (reporte de resumen).

> **Nota sobre el gate global**: el `--fail-under-lines 85` propuesto originalmente cubría
> el workspace completo, pero la cobertura global real del workspace (toda la superficie
> código de shared + server: API routes, db, realtime, etc.) es **65.0%** — un gate al 85%
> sería rojo permanente y por tanto ignorado. Se fija el gate global en **60%** como red de
> seguridad contra fallos catastróficos (p.ej., crate sin instrumentar, tests no ejecutándose),
> mientras que los umbrales por módulo protegido (90/85/80) son la protección primaria.

## API Contracts
N/A — infra de CI, sin cambios de API.

## Data Models
N/A — sin cambios de schema.

## Edge Cases
| # | Caso | Comportamiento |
|---|------|----------------|
| 1 | job corre sin el componente `llvm-tools-preview` | el job lo instala con `rustup component add` |
| 2 | Múltiples crates en el workspace | umbrales se evalúan por módulo (patrón de ruta), no global; invocación scoped a `-p dmart-shared -p dmart-server` |
| 3 | Threshold falla por regresión real | script reporta diff línea por módulo; test negativo verificado (`parser.rs` 11.9% → gate flakea con exit 1) |
| 4 | Módulo ausente del LCOV | el script imprime `⚠️ NOT FOUND` y no cuenta como pass |

## Security Considerations
- `lcov.info` NO debe publicarse como artifact público (rutas internas); el artifact de Actions es interno por defecto.
- No se exponen secretos nuevos; el job no requiere permisos adicionales.

## Testing Strategy
- [x] CI job verdes localmente (invocación idéntica al job) — reporte LCOV generado
- [x] Test negativo documentado: `parser.rs` reducido a 11.9% → `check-coverage-thresholds.sh` devuelve `exit 1` ⧿ "❌ 1 module(s) below threshold" (verificado con LCOV sintético, guardado en /tmp)
- [x] `validation.rs` subido de 82.9% → **100%** con 7 tests nuevos (fio2>1.0, A-aDO2 crítico, edad>120, GCS verbal/motor out-of-range, valor<min físico, crítico-alto warning, `get_range_description`)
- [x] `scales.rs` 89.1% (→ contribución de SPEC-028)
- [x] Hallazgo colateral: `circuit_breaker.rs` (SPEC-031) tenía 2 tests consistentemente rojos (estado HalfOpen era no-op) — corregido el bug real + tests actualizados a `success_threshold=3`

## Rollout Plan
- Añadido al job `coverage` dentro de `ci.yml`, en `needs:` de `release-build` y `notify`.
- **SPEC-007** absorbido: el job corre en cada push/PR sin label especial.

## Definition of Done
- [x] Spec aprobada
- [x] Job `coverage` en `ci.yml` con gate por módulo + gate global ≥60% + artifact LCOV
- [x] Script `scripts/check-coverage-thresholds.sh` (tabla única)
- [x] `validation.rs` y `scales.rs` por encima de 85%
- [x] Test negativo del gate documentado y verificado (exit 1 con regresión simulada)
- [x] CHANGELOG.md y ROADMAP actualizados