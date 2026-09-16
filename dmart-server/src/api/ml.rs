//! SPEC-032 + SPEC-033: ML Serving + Patient Similarity API.
//! Router: `.merge(ml::routes())` en build_api_router.
//! Endpoints: /ml/predict, /ml/predict_batch, /ml/models, /ml/models/swap,
//!            /ml/similarity/search, /ml/similarity/explain, /ml/similarity/status,
//!            /ml/similarity/embeddings/regenerate.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use serde::{Deserialize, Serialize};

use crate::auth::Claims;
use crate::db::Database;

use dmart_shared::models::ApiResponse;

#[derive(Deserialize)]
pub struct PredictRequest {
    pub model: String,
    pub features: serde_json::Value,
    #[serde(default)]
    pub patient_id: Option<String>,
}

#[derive(Deserialize)]
pub struct PredictBatchRequest {
    pub model: String,
    pub inputs: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
pub struct ModelSwapRequest {
    pub name: String,
    pub version: String,
}

#[derive(Deserialize)]
pub struct SimilaritySearchRequest {
    pub patient_id: String,
    #[serde(default = "default_k")]
    pub k: usize,
    #[serde(default)]
    pub exclude_patient_ids: Vec<String>,
}

fn default_k() -> usize {
    10
}

#[derive(Serialize)]
pub struct SimilarityStatusResponse {
    pub total_embeddings: usize,
    pub coverage_pct: f64,
    pub model_version: String,
}

fn ok<T: Serialize>(v: T) -> axum::response::Response {
    (StatusCode::OK, Json(ApiResponse::ok(v))).into_response()
}

fn err(status: StatusCode, msg: String) -> axum::response::Response {
    (status, Json(ApiResponse::<serde_json::Value>::err(msg))).into_response()
}

// ── ML Serving (SPEC-032) ────────────────────────────────────────────────

async fn predict_api(
    State(_db): State<Database>,
    Json(req): Json<PredictRequest>,
) -> impl IntoResponse {
    let server = crate::ml_serving::server();
    let features = crate::ml_serving::MlFeatures::from_value(&req.features);
    match server.predict(&req.model, &features) {
        Ok(r) => ok(r),
        Err(e) => {
            let msg = crate::security::sanitize_internal_error(&e);
            err(StatusCode::NOT_FOUND, msg)
        }
    }
}

async fn predict_batch_api(
    State(_db): State<Database>,
    Json(req): Json<PredictBatchRequest>,
) -> impl IntoResponse {
    let server = crate::ml_serving::server();
    let inputs: Vec<_> = req.inputs.iter().map(crate::ml_serving::MlFeatures::from_value).collect();
    match server.predict_batch(&req.model, &inputs) {
        Ok(r) => ok(r),
        Err(e) => {
            let msg = crate::security::sanitize_internal_error(&e);
            err(StatusCode::NOT_FOUND, msg)
        }
    }
}

async fn list_models_api(
    State(_db): State<Database>,
) -> impl IntoResponse {
    let server = crate::ml_serving::server();
    let models = server.registry.all();
    ok(models)
}

async fn swap_model_api(
    State(_db): State<Database>,
    Json(req): Json<ModelSwapRequest>,
) -> impl IntoResponse {
    let server = crate::ml_serving::server();
    match server.swap(&req.name, &req.version) {
        Ok(m) => ok(m),
        Err(e) => {
            let msg = crate::security::sanitize_internal_error(&e);
            err(StatusCode::NOT_FOUND, msg)
        }
    }
}

// ── Patient Similarity (SPEC-033) ────────────────────────────────────────

async fn similarity_search_api(
    State(db): State<Database>,
    claims: Claims,
    Json(req): Json<SimilaritySearchRequest>,
) -> impl IntoResponse {
    let tenant = claims.tenant_id;
    match crate::similarity::search_similar(db.as_ref(), &req.patient_id, &tenant, req.k, &req.exclude_patient_ids).await {
        Ok((hits, total, _)) => {
            #[derive(Serialize)]
            struct Response {
                query_patient: String,
                k: usize,
                results: Vec<crate::similarity::SimilarityHit>,
                total_patients_searched: usize,
            }
            ok(Response {
                query_patient: req.patient_id,
                k: req.k,
                total_patients_searched: total,
                results: hits,
            })
        }
        Err(e) => {
            let msg = crate::security::sanitize_internal_error(&e);
            err(StatusCode::NOT_FOUND, msg)
        }
    }
}

async fn similarity_explain_api(
    State(db): State<Database>,
    Path((p1, p2)): Path<(String, String)>,
) -> impl IntoResponse {
    let emb1: Option<crate::similarity::PatientEmbedding> = db.as_ref()
        .query("SELECT * FROM patient_embedding WHERE patient_id = $pid LIMIT 1")
        .bind(("pid", p1.clone()))
        .await
        .expect("db")
        .take(0)
        .expect("take");
    let emb2: Option<crate::similarity::PatientEmbedding> = db.as_ref()
        .query("SELECT * FROM patient_embedding WHERE patient_id = $pid LIMIT 1")
        .bind(("pid", p2.clone()))
        .await
        .expect("db")
        .take(0)
        .expect("take");
    match (emb1, emb2) {
        (Some(a), Some(b)) => {
            let score = crate::similarity::cosine(&a.embedding, &b.embedding);
            let top = crate::similarity::top_features(&a.embedding, &b.embedding);
            #[derive(Serialize)]
            struct ExplainResponse {
                patient_a: String,
                patient_b: String,
                similarity_score: f32,
                top_features: Vec<String>,
            }
            ok(ExplainResponse {
                patient_a: p1,
                patient_b: p2,
                similarity_score: score,
                top_features: top,
            })
        }
        _ => err(StatusCode::NOT_FOUND, "patient embedding not found".into()),
    }
}

async fn similarity_status_api(
    State(db): State<Database>,
) -> impl IntoResponse {
    let count: Vec<serde_json::Value> = db.as_ref()
        .query("SELECT count() as c FROM patient_embedding GROUP BY c")
        .await
        .expect("db")
        .take(0)
        .expect("take");
    let total = count.first().and_then(|v| v["c"].as_u64()).unwrap_or(0) as usize;
    let patients_count = crate::db::count_patients(db.as_ref()).await.unwrap_or(0) as usize;
    let pct = if patients_count > 0 {
        (total as f64 / patients_count as f64 * 100.0).min(100.0)
    } else {
        0.0
    };
    ok(SimilarityStatusResponse {
        total_embeddings: total,
        coverage_pct: pct,
        model_version: "clinical-encoder-v1".to_string(),
    })
}

async fn regenerate_embeddings_api(
    State(db): State<Database>,
    claims: Claims,
) -> impl IntoResponse {
    let patients: Vec<dmart_shared::models::Patient> = crate::db::list_patients_for_tenant(db.as_ref(), &claims.tenant_id, 1000, 0)
        .await
        .unwrap_or_default();
    let mut count = 0usize;
    for p in patients {
        if crate::similarity::generate_for_patient(db.as_ref(), &p.patient_id).await.is_ok() {
            count += 1;
        }
    }
    #[derive(Serialize)]
    struct RegenResult {
        pub generated: usize,
    }
    ok(RegenResult { generated: count })
}

// ── Router ──────────────────────────────────────────────────────────────

pub fn routes() -> axum::Router<Database> {
    use axum::Router;

    Router::new()
        // ML Serving
        .route("/ml/predict", post(predict_api))
        .route("/ml/predict_batch", post(predict_batch_api))
        .route("/ml/models", get(list_models_api))
        .route("/ml/models/swap", post(swap_model_api))
        // Patient Similarity
        .route("/ml/similarity/search", post(similarity_search_api))
        .route("/ml/similarity/explain/{p1}/{p2}", get(similarity_explain_api))
        .route("/ml/similarity/status", get(similarity_status_api))
        .route("/ml/similarity/embeddings/regenerate", post(regenerate_embeddings_api))
}