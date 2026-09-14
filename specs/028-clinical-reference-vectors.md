# SPEC-028: Suite de Casos de Referencia Clínica (Test Vectors) ✅ **DONE (2026-09-14)**

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

> **Lección reportada durante implementación**: la suite de conformidad detectó 3 errores
> aritméticos propios en los vectores calculados a mano (pH=7.20 → 3 pts no 2; creatinina
> con falla renal aguda duplica; T=31.0 → 3 pts no 2). El gate funciona como red de
> seguridad incluso sobre los propios fixtures.

## Data de Referencia (fuentes)

| Escala | Fuente principal | Fixtures mínimos | Fixtures actuales |
|--------|------------------|------------------|-------------------|
| APACHE II | Knaus et al., Crit Care Med 1985;13:818-29 | 6 | ✅ 7 (001-007) |
| GCS | Teasdale & Jennett 1974 (case workups) | 5 | ✅ 6 (001-006) |
| NEWS2 | RCP London 2017, Appendix (ej. clínicos validados) | 5 | ⏳ pendiente |
| SOFA | Vincent et al. 1996 (validación cohorte) | 4 | ⏳ pendiente |
| SAPS III | Moreno et al. 2005, tabla de calibración | 4 | ⏳ pendiente |

## Formato de Fixtures

`dmart-shared/testdata/scales/<scale>/<NNN>-<nombre>.json`:

> **Nota (decisión de implementación)**: las claves de `inputs` usan los nombres de campo
> canónicos de `ApacheIIData`/`GcsData` (`temperatura`, `presion_arterial_media`, ...) para
> deserializar directo con serde y eliminar la capa de traducción de unidades (que
> introduciría su propio sesgo). Las unidades canónicas quedan documentadas en los
> comentarios de los campos del DTO.

```json
{
  "id": "apache_ii_001_healthy_adult_knaus1985",
  "source": "Knaus et al. 1985, Table N (cita completa obligatoria)",
  "comment": "Descripción clínica del paciente de referencia",
  "inputs": { "...ApacheIIData..." },
  "expected": {
    "apache_ii": 7,
    "subscores": { "temperatura": 0, "...", "aps_total": 7, "edad_pts": 0, "cronicas_pts": 0, "total": 7 }
  }
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
| 1 | Inputs del paper en unidades distintas (mg/dL vs mmol/L) | el fixture indica unidades canónicas (campos del DTO); el loader deserializa directo |
| 2 | Sub-score fuera de rango clínico | 0-point como ya hace `scales` — el fixture lo refleja |
| 3 | Fixture editado sin volver a correr el test de schema | el loader valida invariantes estructurales en cada carga (serde estricto) |
| 4 | Nueva escala sin fixtures | gate de conformidad falla (0 vectores): ninguna escala DONE sin vectores |

## Testing Strategy
- [x] `fn test_conformance_apache_ii()` — itera fixtures `apache_ii/*` (match exacto total + breakdown)
- [x] `fn test_conformance_gcs()` — match exacto + interpretación clínica
- [x] Loader con validación de invariantes (`testdata.rs`: sub-scores ≤ max, suma = total, GCS consistente)
- [x] `fn test_expected_subscores()` — diff granular al fallar (`diff_apache_ii_subscores`)
- [x] proptest adicional: ningún fixture viola invariantes (integr. en `test_conformance_loader_rejects_invariant_violations`)
- [ ] NEWS2 / SOFA / SAPS III (pendientes hasta que sus vectores con cita se construyan)

## Rollout Plan
- Fase 1: fixtures APACHE II + GCS (ya implementados) → gate verde. ✅
- Fase 2: añadir NEWS2/SOFA/SAPS III cuando se construyan los vectores con cita (obligatorio antes de marcar DONE).

## Definition of Done
- [x] Spec aprobada
- [x] 7 fixtures APACHE II con cita Knaus 1985, 6 fixtures GCS con cita Teasdale & Jennett 1974
- [x] Loader + validación de invariantes de fixtures (`dmart-shared/src/testdata.rs`)
- [x] Tests de conformidad verdes con 100% match (`tests/conformance.rs`, 6 tests)
- [x] Gate: ninguna escala DONE (en ROADMAP) sin suite de referencia
- [x] CHANGELOG.md y ROADMAP actualizados