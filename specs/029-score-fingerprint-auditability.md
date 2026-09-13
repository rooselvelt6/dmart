# SPEC-029: Fingerprint y Versionado del Cálculo de Scores (Auditabilidad)

## Contexto
- **Problema**: el resultado de un score no guarda qué versión del algoritmo y qué entradas lo produjeron. Ante una auditoría o disputa médico-legal, no se puede demostrar *con qué versión se calculó un número en una fecha dada*.
- **Usuario objetivo**: Auditor clínico / Médico / Backend
- **Métrica de éxito (KPI)**: 100% de mediciones de score llevan `fingerprint` reproducible; 0 consultas de auditoría sin respuesta.

## Acceptance Criteria (Gherkin)

```gherkin
Feature: Fingerprint de scores auditable
  As a auditor de la institución
  I want reproducer exactamente el score histórico con versión del algoritmo
  So that se pueda validar cualquier decisión clínica años después

  Scenario: Score persistido con fingerprint
    Given un paciente con mediciones de escala (APACHE II)
    When se persiste el measurement
    Then el registro incluye fingerprint = SHA-256(canonical(inputs) + algo + version)
    And los bins de la canónica ignoran el orden/espacios del JSON

  Scenario: Versionado semver del algoritmo
    Given un cambio en scales.apache_ii (ej: corrección de redondeo)
    When se incrementa ALGO_VERSION (minor/patch según semver)
    Then los fingerprints nuevos difieren de los viejos
    And el CHANGELOG documenta el cambio de versión

  Scenario: Reproducción auditada
    Given un measurement histórico con fingerprint
    When se recalculan los inputs guardados con el mismo ALGO_VERSION
    Then el hash coincide (igual a fingerprint)
    And si no coincide, el informe marca "no reproducible (input mutado)"

  Scenario: Endpoint de auditoría
    Given un measurement
    When GET /api/measurements/:id
    Then la respuesta incluye fingerprint, algo y algorithm_version
```

## Design

### Versionado
- `pub const ALGO_VERSION: &str = env!("CARGO_PKG_VERSION");` en `dmart-shared`.
- Mantener **semver**: `major` = cambio que rompe cálculo (incompatibilidad de scores), `minor` = corrección que altera valores en >1 pt, `patch` = refactor sin cambio de valor.
- Preservar compatibilidad: si se corrige el algoritmo, subir `minor` y **documentar en CHANGELOG** tabla `version → cambio`.

### Fingerprint
- Entrada canónica = JSON canónico (claves ordenadas, sin espacios) de `ScaledInputs { apache, gcs }` + `"algo"` + `"version"`.
- `SHA-256` → hex (64 chars). Sin HMAC: integridad/determinismo, no confidencialidad.
- Función en `dmart-shared`:
  ```rust
  pub fn score_fingerprint(algo: ScaleId, version: &str, inputs: &ScaledInputs) -> String
  ```

### Persistencia
- Tabla `measurement` (o registro de score si aplica) añade:
  - `algorithm_version string` (de `ALGO_VERSION`)
  - `fingerprint string` (64 hex chars)

## API Contracts

| Método | Path | Cambio | Response adiciona |
|--------|------|--------|-------------------|
| GET | `/api/measurements/:id` | mod | `score.fingerprint`, `score.algorithm_version` |
| GET | `/api/patients/:id/scores` | mod | idem por cada score |
| GET | `/api/audit/scores` | nuevo (admin) | reproducer de hash + marca reproducible/no |

Response de score añade:
```json
{
  "value": 12,
  "algo": "apache_ii",
  "algorithm_version": "1.3.0",
  "fingerprint": "9f2c...a1"
}
```

## Data Models / Migraciones
```sql
-- migrations/XXX_measurement_fingerprint.surql
DEFINE FIELD algorithm_version ON measurement TYPE string;
DEFINE FIELD fingerprint ON measurement TYPE string;
-- valor histórico sin fingerprint: algorithm_version='<legacy>'
```

## Edge Cases
| # | Caso | Comportamiento |
|---|------|----------------|
| 1 | Measurement previo a migración | `algorithm_version='<legacy>'`, `fingerprint=''` — endpoint marca "sin fingerprint" |
| 2 | Input desconocido en escala | fingerprint se calcula sobre los inputs ya normalizados (0-clip), igual que el score |
| 3 | Dos pacientes con mismos inputs | mismo hash (esperado, no revela PHI de forma reversible) |
| 4 | Cambio de versión en producción | fingerprints coexisten; el reporte filtra por `algorithm_version` |

## Security Considerations
- No exponer inputs crudos en el hash que añadan información innecesaria: solo los ya normalizados mostrados en el response.
- Audit log: añadir `fingerprint` a `audit_log` de lectura de score (append-only) para trazabilidad de consultas.

## Testing Strategy
- [ ] `test_fingerprint_stable_same_inputs()` — determinismo (2 cálculos, mismo hash)
- [ ] `test_fingerprint_changes_with_version()` — versión distinta → hash distinto
- [ ] `test_fingerprint_input_order_insensitive()` — claves reordenadas → hash igual
- [ ] `test_fingerprint_roundtrip_audit()` — recalc histórico == fingerprint guardado
- [ ] proptest: `sha256` distintos para inputs distintos en dominio válido

## Rollout Plan
- Feature flag `SCORE_FINGERPRINT=true` → aplicar en escritura de measurements.
- Migración sin backfill (legacy queda marcado); los endpoints exponen ambos.
- Compatibilidad: FHIR/Observation añade `extension` con fingerprint (campo nuevo, no rompe consumidores).

## Definition of Done
- [ ] Spec aprobada
- [ ] `score_fingerprint()` + `ALGO_VERSION` en `dmart-shared`
- [ ] Migraciones SurrealQL aplicadas
- [ ] Endpoints de score con fingerprint
- [ ] Tests unit + proptest verdes
- [ ] Config `SCORE_FINGERPRINT` flag
- [ ] CHANGELOG.md (tabla version→cambio) y ROADMAP actualizados