use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Utc, Duration};
use uuid::Uuid;
use dmart_shared::models::*;
use dmart_shared::scales::{calculate_apache_ii_score, calculate_gcs_score};
use crate::db::Database;
use crate::db as db_ops;

// POST /api/patients/:id/measurements
pub async fn create_measurement(
    State(db): State<Database>,
    Path(patient_id): Path<String>,
    Json(body): Json<MeasurementRequest>,
) -> impl IntoResponse {
    // Verificar regla de 24 horas
    if let Ok(Some(last)) = db_ops::get_last_measurement(&db, &patient_id).await {
        if let Ok(last_ts) = DateTime::parse_from_rfc3339(&last.timestamp) {
            let elapsed = Utc::now().signed_duration_since(last_ts.with_timezone(&Utc));
            if elapsed < Duration::hours(24) {
                let remaining = Duration::hours(24) - elapsed;
                let hrs = remaining.num_hours();
                let mins = remaining.num_minutes() % 60;
                return (
                    StatusCode::CONFLICT,
                    Json(ApiResponse::<Measurement>::err(format!(
                        "Debe esperar {}h {}min para la siguiente medición",
                        hrs, mins
                    ))),
                ).into_response();
            }
        }
    }

    // Calcular scores
    let apache_score = calculate_apache_ii_score(&body.apache_data);
    let gcs_score = calculate_gcs_score(&body.gcs_data);
    let severity = SeverityLevel::from_score(apache_score);
    let mortality_risk = dmart_shared::scales::mortality_risk(apache_score);

    let measurement = Measurement {
        id: None,
        measurement_id: Uuid::new_v4().to_string(),
        patient_id: patient_id.clone(),
        timestamp: Utc::now().to_rfc3339(),
        apache_data: body.apache_data,
        gcs_data: body.gcs_data,
        apache_score,
        gcs_score,
        severity: severity.clone(),
        mortality_risk,
        notas: body.notas,
    };

    match db_ops::create_measurement(&db, measurement).await {
        Ok(m) => {
            // Actualizar estado_gravedad del paciente
            if let Ok(Some(mut patient)) = db_ops::get_patient(&db, &patient_id).await {
                patient.estado_gravedad = severity;
                patient.ultimo_apache_score = Some(apache_score);
                patient.ultimo_gcs_score = Some(gcs_score);
                patient.updated_at = Utc::now().to_rfc3339();
                let _ = db_ops::update_patient(&db, &patient_id, patient).await;
            }
            (StatusCode::CREATED, Json(ApiResponse::ok(m))).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<Measurement>::err(e.to_string())),
        ).into_response(),
    }
}

// GET /api/patients/:id/measurements
pub async fn get_measurements(
    State(db): State<Database>,
    Path(patient_id): Path<String>,
) -> impl IntoResponse {
    match db_ops::get_measurements_for_patient(&db, &patient_id).await {
        Ok(ms) => (StatusCode::OK, Json(ApiResponse::ok(ms))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<Vec<Measurement>>::err(e.to_string())),
        ).into_response(),
    }
}

// GET /api/patients/:id/measurements/last
pub async fn get_last_measurement(
    State(db): State<Database>,
    Path(patient_id): Path<String>,
) -> impl IntoResponse {
    match db_ops::get_last_measurement(&db, &patient_id).await {
        Ok(m) => (StatusCode::OK, Json(ApiResponse::ok(m))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<Option<Measurement>>::err(e.to_string())),
        ).into_response(),
    }
}

// ─── DTOs ──────────────────────────────────────────────────────────────────

#[derive(serde::Deserialize)]
pub struct MeasurementRequest {
    pub apache_data: ApacheIIData,
    pub gcs_data: GcsData,
    pub notas: String,
}
