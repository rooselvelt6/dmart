//! SPEC-038: Ensemble Mortalidad (mortality-NN) — ensemble de modelos para predicción de mortalidad UCI
//! Extiende SPEC-002 (ML model persistence) + SPEC-032 (ML serving) + SPEC-037 (feature store).

# SPEC-038: Ensemble Mortalidad (mortality-NN)

## Contexto
El modelo actual de mortalidad (`MortalityModel` en `dmart-shared/src/ml.rs`) usa solo un `DecisionTree` entrenado con datos sintéticos. Para alcanzar AUROC > 0.90 con IC 95% (objetivo Fase 2), se requiere un ensemble robusto que combine múltiples algoritmos y use datos reales de la cohorte UCI.

## Objetivo
Implementar un ensemble de mortalidad que combine:
- **Decision Tree** (actual, interpretable)
- **Logistic Regression** (baseline calibrado, coeficientes clínicos)
- **Gradient Boosting** (XGBoost/LightGBM via linfa o candle, mayor performance)
- **Opcional**: Red neuronal simple (MLP) como meta-learner

Con validación rigurosa: split temporal, estratificación por hospital, bootstrap CI 95% para AUROC.

## Alcance

### 3.1 Ensemble Architecture
```rust
pub struct MortalityEnsemble {
    pub models: Vec<Box<dyn MortalityPredictor>>,
    pub weights: Vec<f32>,           // pesos para voting/stacking
    pub meta_learner: Option<Box<dyn MortalityPredictor>>, // stacking
    pub feature_set: FeatureSet,     // SPEC-037 mortality_v2
    pub calibration: CalibrationModel, // isotonic/Platt
}
```

### 3.2 Base Predictors (trait común)
```rust
pub trait MortalityPredictor: Send + Sync {
    fn predict_proba(&self, features: &MortalityFeatures) -> Result<f32>; // 0.0-1.0
    fn predict(&self, features: &MortalityFeatures) -> Result<u8>;         // 0/1
    fn feature_importance(&self) -> Vec<(String, f32)>;
}
```

**Implementaciones:**
1. `DecisionTreePredictor` — wrapper sobre `linfa_trees::DecisionTree` (existente)
2. `LogisticRegressionPredictor` — `linfa_linear::LogisticRegression` con L2
3. `GradientBoostingPredictor` — `linfa_trees::GradientBoostedTrees` o `candle` MLP
4. `MLPPredictor` (opcional) — `candle-nn` 2-layer MLP para stacking

### 3.3 Ensemble Strategies
- **Soft Voting**: promedio ponderado de probabilidades
- **Stacking**: meta-learner (LogisticRegression) sobre predicciones base
- **Selección**: validación en val set, elegir mejor estrategia

### 3.4 Entrenamiento y Validación
- **Datos**: cohortes reales UCI (SurrealDB `patient_events` + outcomes)
- **Split temporal**: train (meses 1-6), val (mes 7), test (mes 8) — no leakage
- **Estratificación**: por hospital (site) + outcome (mortality rate ~15-20%)
- **Validación cruzada**: 5-fold temporal en train para hiperparámetros
- **Métricas**: AUROC (primary), AUPRC, accuracy, F1, Brier score, calibration slope
- **Bootstrap CI**: 1000 resamples para AUROC, AUPRC, Brier score
- **Tamaño muestra**: n reportado por split (train/val/test)

### 3.5 Calibración (requisito clínico)
- **Platt scaling** (sigmoid) sobre val set
- **Isotonic regression** (no paramétrica, más flexible)
- **Reliability diagram**: 10 bins, ECE (Expected Calibration Error) < 0.05
- **Hosmer-Lemeshow test**: p > 0.05 (buena calibración)

### 3.6 Integración ML Serving
- Nuevo modelo `mortality_ensemble` en `MlRegistry` (format "stat" o "onnx")
- Endpoint `/api/v1/ml/predict/mortality` reusa infra SPEC-032
- Hot-swap versiones vía `MlServer::swap()`
- Explicabilidad: SHAP values (SPEC-041) o feature importance agregada

### 3.7 Persistencia
- Serialización bincode (existente) + export ONNX para portabilidad
- Artefactos: `mortality_ensemble_vX.Y.Z.bin` + `.onnx` + `.sha256` + `normalizer.json`
- Versionado semántico: v2.0.0 (ensemble), v2.1.0 (nuevo modelo base), etc.

## Fuera de Alcance
- A/B testing framework (SPEC-040)
- SHAP explanations (SPEC-041)
- Retraining automático / drift detection (SPEC-042)
- Datos sintéticos — solo datos reales con consentimiento (SPEC-034 ethics)

## Definition of Done
- [ ] `specs/038-mortality-ensemble.md` (este archivo)
- [ ] `dmart-shared/src/ml_ensemble.rs` — MortalityEnsemble, predictores base, stacking
- [ ] `dmart-shared/src/ml.rs` — refactor MortalityModel → DecisionTreePredictor
- [ ] `scripts/train_mortality.rs` — entrenamiento offline con datos reales
- [ ] Test `mortality_ensemble.rs`: AUROC > 0.90 en test set, CI 95% no incluye 0.85
- [ ] Calibración: ECE < 0.05, Hosmer-Lemeshow p > 0.05
- [ ] Integración `ml_serving.rs`: registro `mortality_ensemble` + endpoint
- [ ] `cargo fmt` + `clippy -p dmart-shared --lib -- -D warnings` = 0/0
- [ ] `cargo fmt` + `clippy -p dmart-server --lib --bin dmart-server -- -D warnings` = 0/0
- [ ] Cobertura SPEC-027: ml_ensemble ≥ 80%

## Gate CI SPEC-038
```bash
cargo test -p dmart-shared --lib ml_ensemble 2>&1 | grep "test result: ok"
cargo test -p dmart-server --test mortality_ensemble 2>&1 | grep "test result: ok"
cargo clippy -p dmart-shared --lib -- -D warnings 2>&1 | grep '^error' | wc -l  # = 0
```

## Métricas Objetivo (Test Set Temporal)
| Métrica | Objetivo | IC 95% |
|---------|----------|--------|
| AUROC | > 0.90 | lower bound > 0.85 |
| AUPRC | > 0.40 | (prevalencia ~15%) |
| Brier Score | < 0.12 | |
| ECE | < 0.05 | |
| Hosmer-Lemeshow p | > 0.05 | |

## Referencias
- [1] Knaus et al. APACHE II (1985) — baseline clínico
- [2] SPEC-028 clinical reference vectors (validación contra Knaus/GCS)
- [3] SPEC-032 ML serving architecture
- [4] SPEC-037 Feature store (mortality_v2 feature set)
- [5] Steyerberg "Clinical Prediction Models" (2019) — calibración, validación