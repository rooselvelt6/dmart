# SPEC-028: Suite de Casos de Referencia Clínica (Test Vectors)

## Contexto
- **Problema**: los tests actuales prueban *propiedades* (bounds, monotonicidad) pero **ningún test valida que un score coincida con el valor publicado de referencia**. Cobertura ≠ corrección clínica. Para una UCI, el error en un score tiene consecuencias médico-legales.
- **Usuario objetivo**: Médico / QA clínico / Backend
- **Métrica de éxito (KPI)**: 100% de conformidad con los vectores de referencia suministrados; ≥5 vectores por escala citados de literatura.

## Acceptance Criteria (Gherkin)

```gherkin
Feature: Conformance clínica con vectores publicados
  As a médico de la UCI
  I want que cada score coincida con los casos de referencia validados
  So that las decisiones clínicas se basen en valores auditables y correctos

  Scenario: Vector de referencia APACHE II
    Given un fixture de entrada publicado (Knaus 1985 caso de calibración)
    When se calcula APACHE II
    Then el resultado coincide EXACTO con el valor esperado del fixture
    And el nombre del fixture referencia la fuente bibliográfica

  Scenario: Escalas sin cobertura documentada
    Given una escala nueva (ej: NEWS2)
    When se crea la escala
    Then no se marca DONE hasta tener ≥5 vectores con cita

  Scenario: Fallo en un vector
    Given un vector que no coincide
    When corre el test de conformidad
    Then el test falla nombrando escala, fixture y diff de sub-score
```

## Data de Referencia (fuentes)

| Escala | Fuente principal | Fixtures mínimos |
|--------|------------------|------------------|
| APACHE II | Knaus et al., Crit Care Med 1985;13:818-29 | 6 |
| GCS | Teasdale & Jennett 1974 (case workups) | 5 |
| NEWS2 | RCP London 2017, Appendix (ej. clínicos validados) | 5 |
| SOFA | Vincent et al. 1996 (validación cohorte) | 4 |
| SAPS III | Moreno et al. 2005, tabla de calibración | 4 |

## Formato de Fixtures

`dmart-shared/testdata/scales/<scale>/<NNN>-<nombre>.json`:
```json
{
  "id": "apache_ii_001_knaus1985",
  "source": "Knaus et al. 1985, Table N",
  "comment": "Paciente de calibración original",
  "inputs": {
    "age": 55,
    "temperature_c": 37.1,
    "map_mmhg": 85,
    "heart_rate_bpm": 88,
    "resp_rate_bpm": 12,
    "pao2_mmhg": 95,
    "ph": 7.42,
    "na_mmol_l": 140,
    "k_mmol_l": 4.0,
    "creatinine_mg_dl": 1.0,
    "hct_pct": 42,
    "wbc_x10e9_l": 9.0,
    "gcs": 15,
    "fiO2": 0.21,
    "chronic_conditions": [],
    "source": "arterial"
  },
  "expected": { "apache_ii": 7 }
}
```
- **Canónicos**: max/mínimos, redondeos y normalización idénticos a `scales.rs` (mismos cero-clips de sub-scores por fuera de rango).
- Un vector puede declarar `expected.subscores` para diff granular en el fallo.

## API Contracts
N/A — fixtures cargados por tests, no hay endpoints nuevos.

## Data Models
N/A — los fixtures se versionan en el repo (no en DB).

## Edge Cases
| # | Caso | Comportamiento |
|---|------|----------------|
| 1 | Inputs del paper en unidades distintas (mg/dL vs mmol/L) | el fixture indica unidades canónicas; el loader normaliza como los schemas |
| 2 | Sub-score fuera de rango clínico | 0-point como ya hace `scales` — el fixture lo refleja |
| 3 | Fixture editado sin volver a correr el test de schema | CI valida JSON Schema de fixtures |
| 4 | Nueva escala sin fixtures | bloqueo DoD: gate `conformance` falla (0 vectores) |

## Testing Strategy
- [ ] `fn test_conformance_apache_ii()` — itera fixtures `apache_ii/*`
- [ ] `fn test_conformance_gcs()` / `new_news2` / `sofa` / `saps3`
- [ ] Loader con validación JSON Schema (`jsonschema` crate o `serde` estricto)
- [ ] `fn test_expected_subscores()` — diff granular al fallar
- [ ] proptest adicional: ningún fixture viola invariantes (sub-score ≤ max y ≥ 0)

## Rollout Plan
- Fase 1: fixtures APACHE II + GCS (ya implementados) → gate verde.
- Fase 2: añadir NEWS2/SOFA/SAPS III cuando se implementen esas escalas (por obligatorio).

## Definition of Done
- [ ] Spec aprobada
- [ ] ≥5 fixtures APACHE II con cita, ≥5 GCS
- [ ] Loader + validación schema de fixtures
- [ ] Tests de conformidad verdes con 100% match
- [ ] Gate: ninguna escala DONE (en ROADMAP) sin suite de referencia
- [ ] CHANGELOG.md y ROADMAP actualizados