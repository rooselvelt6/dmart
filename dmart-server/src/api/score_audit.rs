//! SPEC-029: Auditoría de fingerprints de scores (GET /admin/audit/scores).
//!
//! Para cada `measurement` persistido se recomputa el fingerprint de APACHE II
//! a partir de los inputs ya normalizados (`apache_data`) y se compara contra
//! el fingerprint guardado en el momento de la escritura. Los registros
//! históricos sin fingerprint (previos a la migración 029) se marcan como
//! "sin fingerprint" (`reproducible=false`, `algorithm_version="<legacy>"`).

use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Router,
};
use dmart_shared::models::{ApiResponse, Measurement};
use dmart_shared::scales::score_fingerprint;
use serde::Serialize;
use surrealdb::Surreal;
use surrealdb::engine::local::Db;

use crate::db::Database;

/// Versión por defecto de mediciones históricas sin fingerprint (migración 029).
const LEGACY_VERSION: &str = "<legacy>";
/// Algoritmo sobre el que se calcula/reproduce el fingerprint.
const ALGO: &str = "apache_ii";

#[derive(Debug, Clone, Serialize)]
pub struct ScoreAuditEntry {
    pub patient_id: String,
    pub measurement_id: String,
    pub timestamp: String,
    pub algorithm_version: String,
    pub fingerprint_guardado: String,
    pub fingerprint_recalculado: String,
    pub reproducible: bool,
}

async fn load_measurements(db: &Surreal<Db>) -> Result<Vec<Measurement>, String> {
    let measurements: Vec<Measurement> = db
        .query("SELECT * FROM measurements ORDER BY timestamp DESC")
        .await
        .map_err(|e| e.to_string())?
        .take(0)
        .map_err(|e| e.to_string())?;
    Ok(measurements)
}

fn verify_measurement(m: &Measurement) -> ScoreAuditEntry {
    let algorithm_version = if m.algorithm_version.is_empty() || m.algorithm_version == LEGACY_VERSION
    {
        LEGACY_VERSION.to_string()
    } else {
        m.algorithm_version.clone()
    };

    let has_snapshot = !m.fingerprint.is_empty();
    let fingerprint_recalculado = if has_snapshot {
        let inputs = serde_json::to_value(&m.apache_data).unwrap_or(serde_json::Value::Null);
        score_fingerprint(ALGO, &algorithm_version, &inputs)
    } else {
        String::new()
    };
    let reproducible = has_snapshot && m.fingerprint == fingerprint_recalculado;

    ScoreAuditEntry {
        patient_id: m.patient_id.clone(),
        measurement_id: m.measurement_id.clone(),
        timestamp: m.timestamp.clone(),
        algorithm_version,
        fingerprint_guardado: m.fingerprint.clone(),
        fingerprint_recalculado,
        reproducible,
    }
}

/// GET /admin/audit/scores (admin) — lista de verificación de reproducibilidad.
pub async fn verify_scores(State(db): State<Database>) -> impl IntoResponse {
    match load_measurements(&db).await {
        Ok(measurements) => {
            let entries: Vec<ScoreAuditEntry> =
                measurements.iter().map(verify_measurement).collect();
            (StatusCode::OK, Json(ApiResponse::ok(entries))).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<Vec<ScoreAuditEntry>>::err(e)),
        )
            .into_response(),
    }
}

/// Router independiente con la ruta de auditoría (mismo path que el montado en
/// `api::mod` para que el handler sea testeable en aislamiento). Comparte el
/// prefix `/admin` con el resto de rutas admin del router global plano.
pub fn routes() -> Router<Database> {
    Router::new().route("/admin/audit/scores", get(verify_scores))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use dmart_shared::models::{ApacheIIData, GcsData};
    use dmart_shared::scales::ALGO_VERSION;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    async fn audit_db() -> (Database, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("audit.db");
        let path_str = path.to_str().expect("path").to_string();
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::SurrealKv>(&path_str)
            .await
            .expect("connect");
        db.use_ns("dmart").use_db("icu").await.expect("ns");
        (std::sync::Arc::new(db), dir)
    }

    #[tokio::test]
    async fn verify_scores_reports_reproducible_for_new_measurements() {
        let (db, _dir) = audit_db().await;
        let measurement = Measurement::new(
            "patient-029",
            ApacheIIData::default(),
            GcsData {
                apertura_ocular: 4,
                respuesta_verbal: 5,
                respuesta_motora: 6,
            },
        );
        let saved = crate::db::create_measurement(&db, measurement)
            .await
            .expect("create measurement");
        assert_eq!(saved.algorithm_version, ALGO_VERSION);
        assert_eq!(saved.fingerprint.len(), 64);

        let app = routes().with_state(db);
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/admin/audit/scores")
                    .body(Body::from(""))
                    .expect("request"),
            )
            .await
            .expect("oneshot");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).expect("json");

        let entries = json["data"].as_array().expect("data array");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["measurement_id"], saved.measurement_id);
        assert_eq!(
            entries[0]["fingerprint_guardado"],
            saved.fingerprint,
            "fingerprint must round-trip"
        );
        assert_eq!(
            entries[0]["fingerprint_recalculado"],
            saved.fingerprint,
            "recomputed fingerprint must match the stored one"
        );
        assert_eq!(entries[0]["reproducible"], true);
    }

    #[tokio::test]
    async fn legacy_measurements_are_marked_without_fingerprint() {
        let (db, _dir) = audit_db().await;

        // Inserta en bruto un registro "histórico" (previo a la migración 029)
        // que NO incluye algorithm_version ni fingerprint. El parseo a
        // `Measurement` debe rellenar los defaults ('<legacy>' y "").
        let record = serde_json::json!({
            "measurement_id": "m-legacy-001",
            "patient_id": "patient-legacy",
            "timestamp": "2020-01-01T00:00:00Z",
            "apache_data": serde_json::to_value(ApacheIIData::default()).expect("apache"),
            "gcs_data": serde_json::to_value(GcsData {
                apertura_ocular: 4,
                respuesta_verbal: 5,
                respuesta_motora: 6,
            }).expect("gcs"),
            "apache_score": 0,
            "gcs_score": 15,
            "severity": "Bajo",
            "mortality_risk": 0.0,
            "saps3_score": null,
            "saps3_mortality": null,
            "news2_score": null,
            "news2_level": "Bajo",
            "sofa_score": null,
            "sofa_mortality": null,
            "notas": "",
        });
        db.query("CREATE type::thing('measurements', $id) CONTENT $rec")
            .bind(("id", "m-legacy-001"))
            .bind(("rec", record))
            .await
            .expect("insert legacy row");

        let app = routes().with_state(db);
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/admin/audit/scores")
                    .body(Body::from(""))
                    .expect("request"),
            )
            .await
            .expect("oneshot");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).expect("json");

        let entries = json["data"].as_array().expect("data array");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["algorithm_version"], LEGACY_VERSION);
        assert_eq!(entries[0]["fingerprint_guardado"], "");
        assert_eq!(entries[0]["fingerprint_recalculado"], "");
        assert_eq!(entries[0]["reproducible"], false);
    }
}