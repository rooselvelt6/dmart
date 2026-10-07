use crate::auth::Claims;
use crate::db as db_ops;
use crate::db::Database;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::Utc;
use dmart_shared::models::*;
use dmart_shared::scales::{
    ALGO_VERSION, SCORE_ALGO_MULTISCALE, calculate_apache_ii_score, calculate_gcs_score,
    calculate_news2_score, calculate_saps_iii_score, calculate_sofa_score, mortality_risk,
    saps_iii_mortality_prediction, score_fingerprint, sofa_mortality_estimate,
};
use uuid::Uuid;

// POST /api/patients/:id/measurements — registro completo de todas las escalas
pub async fn create_measurement(
    State(db): State<Database>,
    Path(patient_id): Path<String>,
    claims: Claims,
    Json(body): Json<MeasurementRequest>,
) -> impl IntoResponse {
    // SPEC-025: la medición solo se puede asociar a un paciente del propio
    // tenant (verificado antes de escribir; el tenant se hereda del registro).
    let tenant_id = match db_ops::get_patient(&db, &patient_id).await {
        Ok(Some(p)) if p.tenant_id == claims.tenant_id => p.tenant_id.clone(),
        Ok(Some(_)) | Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ApiResponse::<Measurement>::err("Patient not found")),
            )
                .into_response();
        }
        Err(e) => {
            let msg = crate::security::sanitize_internal_error(&e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<Measurement>::err(msg)),
            )
                .into_response();
        }
    };

    // Validación clínica de rangos físicos antes de calcular scores
    // (dmart-shared/src/validation.rs): valores físicamente imposibles se
    // rechazan con 400; los críticos posibles solo generan warnings.
    let v_apache = dmart_shared::validation::validate_apache_measurement(&body.apache_data);
    let v_gcs = dmart_shared::validation::validate_gcs_measurement(&body.gcs_data);
    for v in [&v_apache, &v_gcs] {
        if let Some(err) = v.errors.first() {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<Measurement>::err(&err.message)),
            )
                .into_response();
        }
    }

    // Calcular todos los scores
    let apache_score = calculate_apache_ii_score(&body.apache_data);
    let gcs_score = calculate_gcs_score(&body.gcs_data);
    let severity = SeverityLevel::from_score(apache_score);
    let mort = mortality_risk(apache_score);

    let saps3_score = calculate_saps_iii_score(&body.apache_data);
    let saps3_mort = saps_iii_mortality_prediction(saps3_score);
    let news2_score = calculate_news2_score(&body.apache_data);
    let news2_level = News2Level::from_score(news2_score);
    let sofa_score = calculate_sofa_score(&body.apache_data);
    let sofa_mort = sofa_mortality_estimate(sofa_score);

    // SPEC-029: versionado + fingerprint de los inputs ya normalizados.
    let algorithm_version = ALGO_VERSION.to_string();
    let normalized_inputs =
        serde_json::to_value(&body.apache_data).unwrap_or(serde_json::Value::Null);
    let fingerprint = score_fingerprint(SCORE_ALGO_MULTISCALE, ALGO_VERSION, &normalized_inputs);

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
        mortality_risk: mort,
        saps3_score: Some(saps3_score),
        saps3_mortality: Some(saps3_mort),
        news2_score: Some(news2_score),
        news2_level,
        sofa_score: Some(sofa_score),
        sofa_mortality: Some(sofa_mort),
        algorithm_version,
        fingerprint,
        notas: body.notas,
        tenant_id,
    };

    match db_ops::create_measurement(&db, measurement).await {
        Ok(m) => {
            crate::metrics::scale_calculated("apache");
            crate::metrics::ml_prediction("apache_mortality_v1");
            // Actualizar estado_gravedad del paciente.
            //
            // Se persisten **todas** las escalas calculadas, no solo APACHE y
            // GCS: el listado lee `ultimo_news2_score`, `ultimo_sofa_score`,
            // `ultimo_saps3_score` y `mortality_risk` del paciente, así que
            // dejarlos en `None` los dejaba permanentemente a `null` en la UI
            // aunque la Medición sí los tuviera.
            if let Ok(Some(mut patient)) = db_ops::get_patient(&db, &patient_id).await {
                patient.estado_gravedad = severity;
                patient.ultimo_apache_score = Some(apache_score);
                patient.ultimo_gcs_score = Some(gcs_score);
                patient.ultimo_news2_score = Some(news2_score);
                patient.ultimo_sofa_score = Some(sofa_score);
                patient.ultimo_saps3_score = Some(saps3_score);
                patient.mortality_risk = Some(mort);
                patient.updated_at = Utc::now().to_rfc3339();
                let _ = db_ops::update_patient(&db, &patient_id, patient).await;
            }
            // Publicar evento en tiempo real (SSE), acotado al tenant del
            // paciente (SPEC-025): el hub es compartido por toda la instancia.
            crate::realtime::publish_for_tenant(
                &m.tenant_id,
                "measurement",
                serde_json::json!({
                    "patient_id": patient_id,
                    "measurement_id": m.measurement_id,
                    "apache_score": m.apache_score,
                    "gcs_score": m.gcs_score,
                    "severity": m.severity.label(),
                    "mortality_risk": m.mortality_risk,
                    "timestamp": m.timestamp,
                }),
            );
            (StatusCode::CREATED, Json(ApiResponse::ok(m))).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<Measurement>::err(
                crate::security::sanitize_internal_error(&e),
            )),
        )
            .into_response(),
    }
}

// GET /api/patients/:id/measurements
pub async fn get_measurements(
    State(db): State<Database>,
    Path(patient_id): Path<String>,
    claims: Claims,
) -> impl IntoResponse {
    if !patient_belongs_to_tenant(&db, &patient_id, &claims).await {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<Vec<Measurement>>::err("Patient not found")),
        )
            .into_response();
    }
    match db_ops::get_measurements_for_patient(&db, &patient_id).await {
        Ok(ms) => (StatusCode::OK, Json(ApiResponse::ok(ms))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<Vec<Measurement>>::err(
                crate::security::sanitize_internal_error(&e),
            )),
        )
            .into_response(),
    }
}

// GET /api/patients/:id/measurements/last
pub async fn get_last_measurement(
    State(db): State<Database>,
    Path(patient_id): Path<String>,
    claims: Claims,
) -> impl IntoResponse {
    if !patient_belongs_to_tenant(&db, &patient_id, &claims).await {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<Option<Measurement>>::err("Patient not found")),
        )
            .into_response();
    }
    match db_ops::get_last_measurement(&db, &patient_id).await {
        Ok(m) => (StatusCode::OK, Json(ApiResponse::ok(m))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<Option<Measurement>>::err(
                crate::security::sanitize_internal_error(&e),
            )),
        )
            .into_response(),
    }
}

// TRUE si existe un paciente con ese ID dentro del tenant del JWT.
async fn patient_belongs_to_tenant(db: &Database, patient_id: &str, claims: &Claims) -> bool {
    matches!(
        db_ops::get_patient(db, patient_id).await,
        Ok(Some(p)) if p.tenant_id == claims.tenant_id
    )
}

// ─── DTOs ──────────────────────────────────────────────────────────────────

#[derive(serde::Deserialize)]
pub struct MeasurementRequest {
    pub apache_data: ApacheIIData,
    pub gcs_data: GcsData,
    pub notas: String,
}
