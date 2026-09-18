//! SPEC-037: Feature Store Versionado — features reproductibles y versionadas para ML
//! Prerrequisito para SPEC-036 (LOS-NN) y SPEC-038 (mortality-NN).

# SPEC-037: Feature Store Versionado

## Contexto
Los modelos de ML requieren features consistentes y reproducibles. Actualmente `MlFeatures` en `ml_serving.rs` es un `HashMap<String, f32>` sin versionado ni validación de esquema. Para modelos de deep learning (LOS-NN, mortality-NN) necesitamos:
- Mismo conjunto de features ↔ mismo hash (reproducibilidad)
- Versionado semántico de feature sets (v1, v2, ...)
- Validación de esquema en tiempo de compilación y ejecución
- Feature engineering temporal automatizado (ventanas rolling, deltas, tendencias)
- Lineaje: feature set → modelo (trazabilidad completa)

## Objetivo
Implementar un feature store versionado en `dmart-shared/src/ml_features.rs` que:
- Defina `FeatureSet` con versión semántica y hash de integridad
- Proporcione `FeatureExtractor` trait para extracción determinística desde datos clínicos
- Incluya feature engineering temporal (rolling windows, deltas, trend features)
- Genere hash SHA-256 del feature set para verificación de reproducibilidad
- Exporte esquema JSON para validación en serving

## Alcance

### 3.1 FeatureSet Definition
```rust
pub struct FeatureSet {
    pub version: semver::Version,      // ej: "1.0.0"
    pub name: String,                   // ej: "los_nn_v1", "mortality_v2"
    pub features: Vec<FeatureDef>,      // definiciones ordenadas
    pub hash: String,                   // SHA-256 del contenido serializado
    pub created_at: DateTime<Utc>,
    pub description: String,
}

pub struct FeatureDef {
    pub name: String,                   // ej: "heart_rate_rolling_mean_6h"
    pub dtype: FeatureDtype,            // Float32, Int32, Bool
    pub description: String,
    pub transformation: Option<String>, // ej: "rolling_mean(window=6h)"
    pub stats: Option<FeatureStats>,    // mean, std, min, max para normalización
}
```

### 3.2 FeatureExtractor Trait
```rust
pub trait FeatureExtractor: Send + Sync {
    fn feature_set(&self) -> &FeatureSet;
    fn extract(&self, patient_id: &str, window: TimeWindow) -> Result<MlFeatures>;
    fn extract_batch(&self, patient_ids: &[&str], window: TimeWindow) -> Result<Vec<MlFeatures>>;
}
```

### 3.3 Feature Engineering Temporal
- **Rolling windows**: mean, std, min, max, slope (últimas 1h, 6h, 12h, 24h)
- **Deltas**: diff(t, t-1h), diff(t, t-6h) para cada vital sign
- **Trend features**: linear regression slope over window
- **Gap features**: missingness ratio, last observation carried forward
- **Cyclical encoding**: hour-of-day sin/cos, day-of-week sin/cos

### 3.4 Normalización
- `FeatureStats` con mean/std por feature (computados en training set)
- `Normalizer` struct: fit() en train, transform() en train/val/test
- Persistencia de normalizador junto al modelo

### 3.5 Hash de Reproducibilidad
- `FeatureSet::compute_hash()` serializa (version, name, features ordenados) → SHA-256
- Mismo feature set → mismo hash (determinístico)
- Verificación en serving: `assert_eq!(model.feature_hash, expected_hash)`

### 3.6 Feature Sets Iniciales
| Nombre | Versión | Target | Features aprox |
|--------|---------|--------|----------------|
| `los_nn_v1` | 1.0.0 | SPEC-036 LOS-NN | 120 (30 base × 4 ventanas) |
| `mortality_v2` | 2.0.0 | SPEC-038 Ensemble | 45 (Apache II + vitals + labs) |
| `ews_v1` | 1.0.0 | SPEC-014 EWS streaming | 15 (vitals crudos + scores) |

## Fuera de Alcance
- Feature store distribuido (Feast, etc.) — solo local/in-memory
- Feature computation online/streaming (batch offline por ahora)
- Feature monitoring / drift detection (SPEC-042)

## Definition of Done
- [ ] `specs/037-feature-store.md` (este archivo)
- [ ] `dmart-shared/src/ml_features.rs` — FeatureSet, FeatureDef, FeatureExtractor, Normalizer
- [ ] `dmart-shared/src/ml_features.rs` — Feature sets iniciales: `los_nn_v1`, `mortality_v2`, `ews_v1`
- [ ] Test `ml_features.rs`: hash determinístico, extracción batch, normalización fit/transform
- [ ] Integración con `ml_serving.rs`: `MlFeatures` usa `FeatureSet` para validación
- [ ] `cargo fmt` + `clippy -p dmart-shared --lib -- -D warnings` = 0/0
- [ ] Cobertura SPEC-027: ml_features ≥ 85%

## Gate CI SPEC-037
```bash
cargo test -p dmart-shared --lib ml_features 2>&1 | grep "test result: ok"
cargo clippy -p dmart-shared --lib -- -D warnings 2>&1 | grep '^error' | wc -l  # = 0
```

## Referencias
- SPEC-002: ML model persistence (hash de integridad)
- SPEC-032: ML serving architecture (MlFeatures input)
- SPEC-036: LOS-NN (consumidor principal)
- SPEC-038: Ensemble mortalidad (consumidor secundario)