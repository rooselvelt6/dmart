# SPEC-002: ML Model Persistence (Serializar DecisionTree real)

## Contexto
- **Problema**: `MortalityModel::load()` re-entrena el modelo en lugar de deserializar el árbol entrenado. Pérdida de tiempo en arranque, no determinístico si cambia seed, no versionable.
- **Usuario objetivo**: ML Engineer / Backend Developer
- **Métrica de éxito (KPI)**: Arranque modelo < 100ms (vs ~2s re-entrenando), modelo idéntico bit-a-bit tras save/load, versionado en SurrealDB.

## Acceptance Criteria (Gherkin)

```gherkin
Feature: ML Model Persistence
  As a ML Engineer
  I want to save and load trained DecisionTree models
  So that production restarts are fast and reproducible

  Scenario: Train → Save → Load produces identical predictions
    Given a trained MortalityModel with known accuracy
    When I call model.save("/tmp/model.bin")
    And then MortalityModel::load("/tmp/model.bin")
    Then loaded model produces identical predictions for 1000 test samples
    And load time < 100ms

  Scenario: Model versioned in SurrealDB
    Given a trained model
    When I call model.save_to_db("mortality_v1")
    Then record exists in SurrealDB with fields: version, created_at, accuracy, tree_bytes
    And I can load it by version

  Scenario: Graceful fallback if model file missing
    Given no saved model exists
    When application starts
    Then it trains new model (current behavior) and saves it
    And logs warning "No persisted model found, training new one"
```

## API Contracts

### Nuevos métodos en `MortalityModel`

```rust
impl MortalityModel {
    /// Serializa el árbol entrenado a bytes (bincode + serde)
    pub fn save(&self, path: &str) -> Result<(), Box<dyn Error>>;

    /// Deserializa árbol desde bytes
    pub fn load(path: &str) -> Result<Self, Box<dyn Error>>;

    /// Guarda en SurrealDB con metadatos
    pub async fn save_to_db(&self, version: &str, db: &Database) -> Result<(), Box<dyn Error>>;

    /// Carga desde SurrealDB por versión
    pub async fn load_from_db(version: &str, db: &Database) -> Result<Self, Box<dyn Error>>;
}
```

### SurrealDB Schema
```sql
DEFINE TABLE ml_model SCHEMAFULL;
DEFINE FIELD version ON ml_model TYPE string;
DEFINE FIELD created_at ON ml_model TYPE datetime;
DEFINE FIELD accuracy ON ml_model TYPE float;
DEFINE FIELD tree_bytes ON ml_model TYPE blob;
DEFINE FIELD feature_names ON ml_model TYPE array<string>;
DEFINE INDEX idx_version ON ml_model COLUMNS version UNIQUE;
```

## Data Models

### Estructura serializada
```rust
#[derive(Serialize, Deserialize)]
pub struct PersistedModel {
    pub version: String,
    pub created_at: DateTime<Utc>,
    pub accuracy: f32,
    pub feature_names: Vec<String>,
    pub tree_bytes: Vec<u8>,  // bincode serialized DecisionTree
}
```

## Edge Cases
| # | Caso | Comportamiento esperado |
|---|------|------------------------|
| 1 | Archivo corrupto/truncado | Error claro, fallback a re-entrenar |
| 2 | Versión incompatible (schema cambiado) | Error con migración sugerida, fallback |
| 3 | DB no disponible al guardar | Log error, continuar en memoria |
| 3 | Modelo cargado pero accuracy degradada | Alertar métrica, no bloquear |

## Security Considerations
- **Model poisoning**: Validar checksum SHA256 al cargar
- **Data leakage**: No persistir datos de entrenamiento, solo el árbol
- **Integrity**: Firmar modelo con HMAC (clave en `DMART_ML_KEY`)

## Testing Strategy

### Unit Tests
- [ ] `test_save_load_roundtrip()` — predicciones idénticas 1000 samples
- [ ] `test_load_corrupted_file()` — error graceful + fallback
- [ ] `test_version_mismatch()` — error claro

### Integration Tests
- [ ] `test_save_load_surrealdb()` — persistencia real en DB
- [ ] `test_startup_load_performance()` — < 100ms load time

### Property-Based (proptest)
- [ ] Round-trip serialización preserva predicciones para inputs aleatorios

### Performance
- [ ] Benchmark: train (2s) vs load (50ms) — 40x speedup

## Rollout Plan
- **Feature Flag**: `ML_PERSISTENCE=true` (default ON en release)
- **Migración**: Primer arranque entrena + guarda; subsiguientes cargan
- **Rollback**: Borrar archivo/registro DB → re-entrena automáticamente

## Definition of Done
- [ ] Spec aprobada
- [ ] `save()` / `load()` implementados con bincode
- [ ] SurrealDB persistence async
- [ ] Tests unit + integration pasando
- [ ] Benchmark: load < 100ms
- [ ] CHANGELOG.md actualizado
- [ ] Documentación en README (sección ML)