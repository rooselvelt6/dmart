# SPEC-033: Patient Similarity Engine — Búsqueda por embeddings clínicos

## Contexto

- **Problema a resolver**: Los médicos necesitan encontrar pacientes con patrones clínicos similares para decisiones terapéuticas (evidence-based medicine). Actualmente la búsqueda es manual por diagnóstico oScore. Se necesita un motor de similaridad que convierta el perfil clínico completo de un paciente en un vector (embedding) y busque los K más similares en < 100ms.
- **Usuario objetivo**: Médico / Investigador
- **Métrica de éxito (KPI)**: Búsqueda de K=10 similares en < 100ms; resultados clínicamente relevantes (validación médica); cobertura de 100% de pacientes activos

## Acceptance Criteria (Gherkin)

```gherkin
Feature: Patient Similarity Engine
  As a médico
  I want encontrar pacientes con patrones clínicos similares
  So that pueda tomar decisiones basadas en evidencia de casos previos

  Scenario: Generación de embedding clínico
    Given paciente "P-001" con vitales, scores y diagnósticos
    When se genera su embedding clínico
    Then el embedding es un vector de 128 dimensiones
    And se almacena en la tabla `patient_embedding`
    And el embedding se actualiza con cada nueva medición

  Scenario: Búsqueda de K pacientes similares
    Given 1000 pacientes con embeddings generados
    When médico busca pacientes similares a "P-001" con K=10
    Then recibo 10 pacientes ordenados por similitud (cosine)
    And cada resultado incluye: patient_id, similarity_score, resumen clínico
    And la búsqueda toma < 100ms

  Scenario: Filtro por criterios clínicos
    Given búsqueda de similares a "P-001"
    When filtro por: edad > 60, diagnóstico = "sepsis"
    Then solo se retornan pacientes que cumplen filtros
    And el ranking de similitud se mantiene dentro del subset

  Scenario: Actualización de embedding
    Given "P-001" con embedding generado
    When recibe nueva medición vital
    Then el embedding se recalcula en background (< 5s)
    And la búsqueda de similares usa el embedding actualizado

  Scenario: Paciente nuevo sin embedding
    Given paciente recién ingresado sin mediciones suficientes (< 3)
    When médico busca similares excluyendo al paciente nuevo
    Then el paciente nuevo no aparece en resultados
    And métrica ml_similarity_skipped_patients incrementa

  Scenario: Explainability de similaridad
    Given búsqueda de similares con K=5
    When explico por qué "P-002" es similar a "P-001"
    Then recibo las features más contribuyentes (top-5)
    And cada feature tiene: nombre, peso, valor en P-001, valor en P-002
```

## API Contracts

### Endpoints nuevos

| Método | Path | Auth | Request | Response | Errores |
|--------|------|------|---------|----------|---------|
| POST | `/api/ml/similarity/search` | Bearer + medico | `SimilarityRequest` | `SimilarityResponse` | 400, 404, 422 |
| GET | `/api/ml/similarity/explain/{p1}/{p2}` | Bearer + medico | — | `ExplainResponse` | 404 |
| POST | `/api/ml/similarity/embeddings/regenerate` | Bearer + admin | — | `202 {job_id}` | 401, 403 |
| GET | `/api/ml/similarity/status` | Bearer + admin | — | `SimilarityStatus` | 401, 403 |

### Request Schema

```json
// POST /api/ml/similarity/search
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "required": ["patient_id", "k"],
  "properties": {
    "patient_id": { "type": "string" },
    "k": { "type": "integer", "minimum": 1, "maximum": 100, "default": 10 },
    "filters": {
      "type": "object",
      "properties": {
        "min_age": { "type": "integer" },
        "max_age": { "type": "integer" },
        "diagnoses": { "type": "array", "items": { "type": "string" } },
        "exclude_patient_ids": { "type": "array", "items": { "type": "string" } }
      }
    },
    "include_summary": { "type": "boolean", "default": true }
  }
}
```

### Response Schema

```json
{
  "query_patient": "P-001",
  "k": 10,
  "results": [
    {
      "patient_id": "P-042",
      "similarity_score": 0.94,
      "summary": {
        "age": 67,
        "diagnoses": ["sepsis", "ARDS"],
        "latest_score": 8.2,
        "los_days": 12
      },
      "matching_features": ["heart_rate_pattern", "wbc_trend", "o2_sat"]
    }
  ],
  "latency_ms": 45,
  "total_patients_searched": 1000
}
```

### Explainability Response

```json
// GET /api/ml/similarity/explain/P-001/P-042
{
  "patient_a": "P-001",
  "patient_b": "P-042",
  "similarity_score": 0.94,
  "top_features": [
    { "feature": "heart_rate_mean", "weight": 0.35, "value_a": 95, "value_b": 92, "contribution": 0.33 },
    { "feature": "wbc_trend_slope", "weight": 0.28, "value_a": 2.1, "value_b": 1.8, "contribution": 0.26 },
    { "feature": "o2_sat_min", "weight": 0.22, "value_a": 88, "value_b": 90, "contribution": 0.21 }
  ],
  "clinical_note": "Ambos pacientes muestran taquicardia sostenida con tendencia leucocitaria y desaturación parcial"
}
```

## Data Models

### SurrealDB Schema

```sql
-- Embedding clínico por paciente
DEFINE TABLE patient_embedding SCHEMAFULL;
DEFINE FIELD patient_id ON patient_embedding TYPE string;
DEFINE FIELD tenant_id ON patient_embedding TYPE string;
DEFINE FIELD embedding ON patient_embedding TYPE array;  -- vector 128d
DEFINE FIELD model_version ON patient_embedding TYPE string;
DEFINE FIELD features_used ON patient_embedding TYPE array;
DEFINE FIELD generated_at ON patient_embedding TYPE datetime;
DEFINE FIELD updated_at ON patient_embedding TYPE datetime;
DEFINE INDEX idx_emb_patient ON patient_embedding COLUMNS patient_id UNIQUE;
DEFINE INDEX idx_emb_tenant ON patient_embedding COLUMNS tenant_id;
```

### Feature Extraction Pipeline

```rust
struct ClinicalFeatures {
    // Demographics
    age: f32,
    sex: f32,                    // 0/1 encoded
    bmi: f32,

    // Vital signs (últimos 24h + tendencia)
    heart_rate_mean: f32,
    heart_rate_trend: f32,       // slope
    systolic_bp_mean: f32,
    diastolic_bp_mean: f32,
    respiratory_rate_mean: f32,
    temperature_mean: f32,
    o2_sat_mean: f32,
    o2_sat_min: f32,

    // Lab values
    wbc_mean: f32,
    wbc_trend: f32,
    lactate_mean: f32,
    creatinine_mean: f32,

    // Scores
    news2_latest: f32,
    news2_trend: f32,
    sofa_latest: f32,
    gcs_latest: f32,

    // Temporal patterns
    los_days: f32,               // length of stay
    vital_measurement_count: f32,
    score_calculation_count: f32,
}
```

### Embedding Generation Config

```rust
struct EmbeddingConfig {
    dimensions: usize,           // 128
    model: EmbeddingModel,       // custom clinical encoder
    feature_columns: Vec<String>,
    normalization: Normalization, // z-score per feature
}

enum EmbeddingModel {
    // Entrenado offline en Python, exportado a ONNX
    ClinicalEncoder { model_path: PathBuf },
    // Fallback: weighted average manual
    WeightedAverage { weights: HashMap<String, f32> },
}
```

### Vector Search (HNSW index in-memory)

```rust
struct VectorIndex {
    dimension: usize,
    ef_construction: usize,      // 200
    m: usize,                    // 16
    max_elements: usize,         // 100000
    // hnswlib-rs o similar
}
```

### Prometheus Metrics

```prometheus
# Embedding generation
ml_embedding_generated_total{model_version}                    # Counter
ml_embedding_generation_duration_ms{model_version}             # Histogram
ml_embedding_updated_total{reason="new_measurement|scheduled"} # Counter

# Similarity search
ml_similarity_search_duration_ms{k, filters}                   # Histogram
ml_similarity_search_total{k}                                  # Counter
ml_similarity_skipped_patients{reason}                         # Counter

# Vector index
ml_vector_index_size                                           # Gauge
ml_vector_index_memory_bytes                                   # Gauge
```

## Edge Cases

| # | Caso | Comportamiento esperado |
|---|------|------------------------|
| 1 | Paciente sin mediciones suficientes | Excluido de resultados; 404 si se busca directamente |
| 2 | Todos los pacientes son idénticos | Scores de similitud = 1.0; top-K retorna K primeros |
| 3 | Embedding desactualizado | Recalculado en background; search usa último disponible |
| 4 | Búsqueda con K > total pacientes | Retorna todos los disponibles con K real |
| 5 | Filtros eliminan todos los resultados | Array vacío; 0 resultados no es error |
| 6 | Modelo ONNX de embedding corrupto | Fallback a weighted average; WARNING en logs |
| 7 | Memoria index insuficiente | HNSW con `max_elements` limitado; oldest evicted |

## Security Considerations

### Threat Model (STRIDE)

| Threat | Mitigación |
|--------|------------|
| Spoofing | Auth required; medico+ role |
| Tampering | Embeddings firmados con model_version; regeneración validada |
| Repudiation | Audit log de cada búsqueda (who, patient, K, timestamp) |
| Information Disclosure | Embeddings no contienen PHI directamente (numéricos) |
| Denial of Service | Rate limit búsqueda; max K=100; timeout 500ms |
| Elevation of Privilege | Solo medico+ puede buscar; solo admin puede regenerar |

### PHI Protection

```rust
// Patient summary en resultados NO incluye nombre, MRN, etc.
struct PatientSummary {
    age: u8,
    diagnoses: Vec<String>,    // códigos, no nombres
    latest_score: f32,
    los_days: u32,
    // NO: name, MRN, room, etc.
}
```

### Data Classification
- [x] Clinical Data (features numéricas)
- [x] Operational/Metadata (embeddings, scores)
- [ ] PHI (no se expone en resultados de similaridad)

## Testing Strategy

### Unit Tests
- [ ] Feature extraction genera vector correcto
- [ ] Embedding generation produce vector 128d
- [ ] Cosine similarity calcula correctamente
- [ ] HNSW index search retorna K más cercanos
- [ ] Filtro reduce búsqueda correctamente

### Integration Tests
- [ ] Generación de embedding para paciente existente
- [ ] Búsqueda de similares con K=10 retorna 10 resultados
- [ ] Explainability retorna features contribuyentes
- [ ] Regeneración de embeddings funciona
- [ ] Búsqueda con filtros funciona

### Benchmarks
- [ ] Generación de embedding < 50ms
- [ ] Búsqueda K=10 en 1000 pacientes < 100ms
- [ ] Búsqueda K=10 en 10000 pacientes < 200ms
- [ ] Throughput > 100 búsquedas/s

### Clinical Validation
- [ ] Médico valida top-5 similares para 10 pacientes de referencia
- [ ] Accuracy de similaridad > 80% (agreement médico)
- [ ] Explainability es clínicamente interpretable

## Rollout Plan

### Feature Flag
```rust
// config.rs
ml_similarity: {
    enabled: false,
    embedding_dimensions: 128,
    index_max_elements: 100000,
    search_timeout_ms: 500,
}
```

### Canary Deployment
| Paso | Tráfico | Duración | Criterio go/no-go |
|------|---------|----------|-------------------|
| 1 | Embeddings generados, 0 búsquedas | 1h | Todos los pacientes activos tienen embedding |
| 2 | Búsqueda habilitada, 1 médico piloto | 1 semana | Resultados validados clínicamente |
| 3 | Todos los médicos | — | Throughput y latencia estables |

### Rollback Procedure
1. `ml_similarity.enabled = false` → búsquedas deshabilitadas
2. Server continúa sin similaridad
3. Embeddings preservados para re-activación

## Definition of Done

- [ ] `specs/033-patient-similarity.md` (este archivo)
- [ ] Feature extraction pipeline en `dmart-shared/src/ml_similarity.rs`
- [ ] Embedding generation (ONNX o weighted average)
- [ ] HNSW vector index en memoria
- [ ] Endpoint `/api/ml/similarity/search` funcional
- [ ] Endpoint `/api/ml/similarity/explain` funcional
- [ ] Embeddings generados para pacientes existentes
- [ ] Métricas `ml_similarity_*` en Prometheus
- [ ] Benchmarks: < 100ms search, > 100/s throughput
- [ ] Validación clínica con 10 casos de referencia
- [ ] Tests: unit + integration + benchmarks + clinical validation
- [ ] Documentación: `docs/PATIENT_SIMILARITY.md`
- [ ] ROADMAP, CHANGELOG actualizados

## Gate CI SPEC-033
```bash
cargo test -p dmart-server --lib -- ml::similarity::tests 2>&1 | grep "test result: ok"
cargo bench -p dmart-server -- similarity_search 2>&1 | grep "p95"  # < 100ms
```

## Métricas
- Search latency: < 100ms p95 (K=10, 1000 pacientes)
- Embedding generation: < 50ms
- Throughput: > 100 searches/s
- Clinical relevance: > 80% agreement médico
- Coverage: 100% pacientes activos con embedding
