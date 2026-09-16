# SPEC-032: ML Serving ONNX/WASM — Inferencia en producción

## Contexto

- **Problema a resolver**: Los modelos ML actuales (SPEC-002, SPEC-015) se entrenan en Python pero no hay forma de servirlos en producción de forma eficiente. El backend Rust necesita inferencia ONNX local (sin dependencia de Python runtime) y opcionalmente WASM para edge devices. Esto permite predicciones en tiempo real < 10ms por inferencia.
- **Usuario objetivo**: Backend / ML Engineer
- **Métrica de éxito (KPI)**: Inferencia < 10ms p95; 0 dependencia de Python en producción; throughput > 1000 predictions/s; modelo descargable vía API

## Acceptance Criteria (Gherkin)

```gherkin
Feature: ML Serving ONNX/WASM
  As a backend engineer
  I want servir modelos ONNX directamente en Rust
  So that las predicciones sean rápidas sin dependencia de Python

  Scenario: Carga de modelo ONNX
    Given un modelo entrenado exportado a ONNX (`ews_model.onnx`)
    When el servidor inicia con `DMART_ML_MODEL_PATH=./models/ews_model.onnx`
    Then el modelo se carga en memoria en < 5s
    And métrica ml_model_loaded{model="ews"} = 1
    And el modelo está listo para inferencia

  Scenario: Inferencia individual
    Given modelo ONNX cargado
    When POST /api/ml/predict con input features
    Then recibo predicción en < 10ms
    And la respuesta contiene `prediction`, `confidence`, `model_version`
    And métrica ml_inference_duration_ms actualizada

  Scenario: Batch inference
    Given modelo ONNX cargado
    When POST /api/ml/predict_batch con 100 inputs
    Then recibo 100 predicciones en < 100ms
    And throughput > 1000 predictions/s

  Scenario: Model swap sin downtime
    Given modelo v1 cargado
    When actualizo el modelo a v2 (`models/ews_model_v2.onnx`)
    Then el swap es atómico: v1 sirve hasta que v2 esté listo
    And 0 interrupciones en servicio
    And métrica ml_model_version cambia a v2

  Scenario: Fallback a CPU cuando GPU no disponible
    Given servidor sin GPU
    When cargo modelo ONNX
    Then el modelo se ejecuta en CPU
    And métrica ml_device = "cpu"
    And la inferencia es < 50ms (aceptable)

  Scenario: Modelo corrupto
    Given archivo ONNX corrupto en path configurado
    When el servidor intenta cargar
    Then el modelo NO se carga
    And error claro en logs
    And el servidor continúa sin ML (degraded mode)
```

## API Contracts

### Endpoints nuevos

| Método | Path | Auth | Request | Response | Errores |
|--------|------|------|---------|----------|---------|
| POST | `/api/ml/predict` | Bearer + medico | `PredictRequest` | `PredictResponse` | 400, 404 (no model), 422, 503 |
| POST | `/api/ml/predict_batch` | Bearer + medico | `PredictBatchRequest` | `PredictBatchResponse` | 400, 404, 422, 503 |
| GET | `/api/ml/models` | Bearer + admin | — | `[ModelInfo]` | 401, 403 |
| POST | `/api/ml/models/swap` | Bearer + admin | `ModelSwapRequest` | `200 {status}` | 400, 404, 503 |

### Request Schema

```json
// POST /api/ml/predict
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "required": ["model", "features"],
  "properties": {
    "model": { "type": "string", "enum": ["ews", "severity", "readmission"] },
    "features": {
      "type": "object",
      "description": "Feature vector específico del modelo"
    },
    "patient_id": { "type": "string" }
  }
}
```

### Response Schema

```json
{
  "prediction": 0.85,
  "confidence": 0.92,
  "model": "ews",
  "model_version": "v1.2.0",
  "latency_ms": 4.2,
  "explanation": {
    "top_features": [
      { "name": "heart_rate", "importance": 0.35 },
      { "name": "respiratory_rate", "importance": 0.28 }
    ]
  }
}
```

### Batch Request

```json
// POST /api/ml/predict_batch
{
  "model": "ews",
  "inputs": [
    { "features": {...}, "patient_id": "p1" },
    { "features": {...}, "patient_id": "p2" }
  ]
}
```

## Data Models

### ONNX Runtime Config

```rust
struct MlServingConfig {
    model_path: PathBuf,
    model_name: String,
    device: Device,           // CPU | CUDA
    num_threads: usize,       // default: num_cpus
    memory_limit_mb: usize,   // default: 512
    timeout_ms: u64,          // default: 100
}

enum Device {
    Cpu,
    Cuda { device_id: i32 },
}
```

### Model Registry

```sql
-- SurrealDB: registro de modelos
DEFINE TABLE ml_model SCHEMAFULL;
DEFINE FIELD name ON ml_model TYPE string;
DEFINE FIELD version ON ml_model TYPE string;
DEFINE FIELD format ON ml_model TYPE string;  -- "onnx" | "wasm"
DEFINE FIELD path ON ml_model TYPE string;
DEFINE FIELD loaded ON ml_model TYPE bool;
DEFINE FIELD loaded_at ON ml_model TYPE option<datetime>;
DEFINE FIELD metrics ON ml_model TYPE object;
DEFINE INDEX idx_ml_model_name ON ml_model COLUMNS name, version UNIQUE;
```

### Prometheus Metrics

```prometheus
# Model status
ml_model_loaded{name, version}                         # Gauge: 1 si cargado
ml_model_load_duration_seconds{name}                   # Histogram

# Inference
ml_inference_duration_ms{name, device}                 # Histogram
ml_inference_total{name, result="ok|error|timeout"}    # Counter
ml_inference_batch_size{name}                          # Histogram

# Device
ml_device{name}                                        # Gauge: "cpu"|"cuda"
ml_memory_usage_bytes{name}                            # Gauge
```

## Edge Cases

| # | Caso | Comportamiento esperado |
|---|------|------------------------|
| 1 | Modelo no configurado | Server funciona sin ML; /api/ml/* retorna 503 |
| 2 | Input features faltantes | 422 con lista de features requeridas |
| 3 | Input fuera de rango del modelo | Predicción con confidence bajo; warning |
| 4 | GPU OOM | Fallback automático a CPU |
| 5 | Modelo incompatible (ONNX version) | Error claro en startup; no crash |
| 6 | Concurrent predictions (1000+) | Thread pool; queue con timeout |
| 7 | Model swap durante inference activa | Drain actual requests; swap atómico |

## Security Considerations

### Threat Model (STRIDE)

| Threat | Mitigación |
|--------|------------|
| Spoofing | Auth required para predicciones |
| Tampering | Modelo firmado (checksum); solo admin puede swap |
| Repudiation | Audit log de cada predicción (patient_id, model, result) |
| Information Disclosure | Predicciones no exponen datos de entrenamiento |
| Denial of Service | Rate limit predictions; memory limit por modelo |
| Elevation of Privilege | Solo medico+ puede predecir; solo admin puede gestionar modelos |

### Model Integrity

```rust
// Verificación de integridad del modelo
fn verify_model_checksum(path: &Path, expected: &str) -> Result<()> {
    let sha256 = compute_sha256(path);
    if sha256 != expected {
        return Err(anyhow!("Model checksum mismatch: possible tampering"));
    }
    Ok(())
}
```

### Data Classification
- [x] Clinical Data (features de entrada)
- [x] Operational/Metadata (modelo, métricas)
- [ ] PHI (no se almacenan predicciones con patient_id en logs)

## Testing Strategy

### Unit Tests
- [ ] Carga de modelo ONNX exitosa
- [ ] Predicción con features válidas retorna resultado
- [ ] Predicción con features inválidas retorna error
- [ ] Model swap es atómico
- [ ] Checksum verification funciona

### Integration Tests
- [ ] Modelo ONNX cargado + predicción vía HTTP
- [ ] Batch prediction funciona
- [ ] Model swap sin downtime
- [ ] Fallback a CPU cuando no hay GPU

### Benchmarks
- [ ] Single prediction < 10ms p95
- [ ] Batch 100 predictions < 100ms
- [ ] Throughput > 1000 predictions/s
- [ ] Memory usage < 512MB por modelo

### Load Test (k6)
- [ ] 100 VUs × 10 predictions/s = 1000/s sostenido
- [ ] Latencia estable bajo carga
- [ ] 0 errores de timeout

## Rollout Plan

### Feature Flag
```rust
// config.rs
ml_serving: {
    enabled: false,
    model_path: "./models/",
    device: "cpu",
    num_threads: 4,
    memory_limit_mb: 512,
}
```

### Canary Deployment
| Paso | Tráfico | Duración | Criterio go/no-go |
|------|---------|----------|-------------------|
| 1 | Modelo cargado, 0 tráfico | 10 min | Checksum OK, memory < limit |
| 2 | 5% predicciones | 1h | Latencia < 10ms, 0 errores |
| 3 | 100% predicciones | — | Modelo servido en producción |

### Rollback Procedure
1. Model swap de vuelta a versión anterior
2. Si modelo corrupto: `ml_serving.enabled = false`
3. Server continúa sin ML (degraded mode)

## Definition of Done

- [ ] `specs/032-ml-serving-onnx.md` (este archivo)
- [ ] ONNX Runtime integrado en `dmart-server` (ort crate)
- [ ] Endpoint `/api/ml/predict` funcional
- [ ] Endpoint `/api/ml/predict_batch` funcional
- [ ] Model swap atómico sin downtime
- [ ] Métricas `ml_*` en Prometheus
- [ ] Benchmarks: < 10ms p95, > 1000/s throughput
- [ ] Tests: unit + integration + benchmarks
- [ ] Documentación: `docs/ML_SERVING.md`
- [ ] ROADMAP, CHANGELOG actualizados

## Gate CI SPEC-032
```bash
cargo test -p dmart-server --lib -- ml::tests 2>&1 | grep "test result: ok"
cargo bench -p dmart-server -- ml_inference 2>&1 | grep "p95"  # < 10ms
```

## Métricas
- Inference latency: < 10ms p95
- Throughput: > 1000 predictions/s
- Model load time: < 5s
- Memory per model: < 512MB
- Uptime: 100% (degraded mode si modelo falla)
