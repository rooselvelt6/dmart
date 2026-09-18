//! SPEC-036: Length of Stay (LOS) Neural Network — predicción de estancia UCI con red neuronal
//! Extiende SPEC-002 (ML model persistence) + SPEC-032 (ML serving) + SPEC-028 (clinical vectors).

# SPEC-036: Predicción de Estancia (LOS) con Red Neuronal

## Contexto
La predicción de la duración de estancia en UCI (Length of Stay - LOS) es crítica para:
- Planificación de camas y recursos
- Gestión de alta y transferencias
- Identificación temprana de estancias prolongadas (outliers)

Actualmente existe un predictor estadístico simple (SPEC-032 `StatPredictor`) pero no un modelo de deep learning que capture dependencias temporales en series de signos vitales y eventos clínicos.

## Objetivo
Implementar un modelo de red neuronal para predicción de LOS que:
- Use series temporales de `measurement` (vital signs) y `patient_events`
- Backend `candle` (Rust-native) o `ort` (ONNX Runtime) vía trait `Predictor` pluggable
- Arquitectura: MLP ventana fija + LSTM/GRU opcional
- Split temporal estricto (entrenamiento: meses pasados; validación: mes reciente; test: mes actual)
- Validación por hospital (site stratification) para generalizabilidad
- Métricas: MAE, RMSE, MAPE + binned accuracy (±24h, ±48h)
- Calibración isotónica + CI bootstrap (1000 resamples)
- Registro en `MlRegistry` con SHA-256 (integridad modelo)

## Alcance

### 2.1 Feature Store Versionado (prerrequisito)
- Mismo hash de features ↔ mismo modelo, reproducible
- `MlFeatures` extendido con features temporales (ventanas rolling, delta, tendencias)
- Versionado semántico de feature sets (v1, v2, ...)

### 2.2 Modelo LOS-NN
- **Entrada**: Secuencia temporal (T pasos, F features) → tensor [batch, T, F]
- **Arquitectura base**: MLP con ventana fija (T=24h, F≈30)
- **Arquitectura avanzada**: LSTM/GRU (opcional, feature-gated)
- **Salida**: LOS en horas (regresión) + distribución predictiva (quantiles)
- **Pérdida**: Huber loss (robusto a outliers) + calibration loss

### 2.3 Validación y Métricas
- Split temporal: train/val/test por fechas (no random)
- Estratificación por hospital (site)
- Métricas reportadas: MAE, RMSE, MAPE, binned acc (±24h/±48h)
- Calibración: isotonic regression + reliability diagram
- Bootstrap CI (1000 iteraciones) para todas las métricas
- n (tamaño muestra) reportado por split

### 2.4 Integración ML Serving
- Nuevo modelo `los_nn` en `MlRegistry` con format "onnx" o "candle"
- Endpoint `/api/v1/ml/predict/los` reusa infra SPEC-032
- Hot-swap de versiones vía `MlServer::swap()`
- Métricas Prometheus: `ml_los_mae`, `ml_los_mape`, `ml_inference_duration_seconds{model="los_nn"}`

### 2.5 Entrenamiento (Offline)
- Script `scripts/train_los.rs` o binary separado
- Datos desde SurrealDB (measurements + outcomes)
- Feature engineering temporal automatizado
- Logging de experimentos (MLflow-style local)
- Artefacto: `los_nn_vX.Y.Z.onnx` + `los_nn_vX.Y.Z.sha256`

## Fuera de Alcance
- Entrenamiento online / continual learning (SPEC-018)
- A/B testing framework (SPEC-016)
- SHAP explanations (SPEC-017)
- Drift detection automático (SPEC-018)

## Definition of Done
- [ ] `specs/036-los-neural-network.md` (este archivo)
- [ ] `dmart-shared/src/ml_features.rs` — feature store versionado con hash
- [ ] `dmart-shared/src/ml_los.rs` — modelo LOS-NN (candle backend)
- [ ] `dmart-server/src/ml_serving.rs` — registro `los_nn` + endpoint predict
- [ ] `scripts/train_los.rs` — entrenamiento offline + export ONNX
- [ ] Test `los_nn.rs`: MAE < 48h en test set sintético, MAPE < 20%
- [ ] Métricas bootstrap CI 95% calculadas y exportables
- [ ] `cargo fmt` + `clippy -p dmart-server --lib --bin dmart-server -- -D warnings` = 0/0
- [ ] Cobertura SPEC-027 no decrece; ml_los ≥ 80%

## Gate CI SPEC-036
```bash
cargo test -p dmart-server --test los_nn 2>&1 | grep "test result: ok"
cargo clippy -p dmart-shared --lib -- -D warnings 2>&1 | grep '^error' | wc -l  # = 0
cargo clippy -p dmart-server --lib --bin dmart-server -- -D warnings 2>&1 | grep '^error' | wc -l  # = 0
```

## Métricas Objetivo (Test Set)
| Métrica | Objetivo |
|---------|----------|
| MAE | < 48 horas |
| RMSE | < 72 horas |
| MAPE | < 20 % |
| Binned acc ±24h | > 60 % |
| Binned acc ±48h | > 80 % |
| Calibración (ECE) | < 0.05 |

## Referencias
- [1] Knaus et al. APACHE II (1985) — baseline clínico
- [2] Lehman et al. "LOS prediction in ICU using LSTM" (2020)
- [3] SPEC-028 clinical reference vectors (Knaus/GCS validados)
- [4] SPEC-032 ML serving architecture