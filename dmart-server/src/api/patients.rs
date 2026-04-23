use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use dmart_shared::models::*;
use crate::db::Database;
use crate::db as db_ops;

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
}

// GET /api/patients?q=<search>
pub async fn list_patients(
    State(db): State<Database>,
    Query(params): Query<SearchQuery>,
) -> impl IntoResponse {
    let result = if let Some(q) = params.q.filter(|s| !s.is_empty()) {
        db_ops::search_patients(&db, &q).await
    } else {
        db_ops::list_patients(&db).await
    };

match result {
        Ok(patients) => {
            let items: Vec<PatientListItem> = patients.iter().map(|p| {
                let edad = calculate_age(&p.fecha_nacimiento);
                PatientListItem {
                    id: p.patient_id.clone(),
                    nombre_completo: p.nombre_completo(),
                    cedula: p.cedula.clone(),
                    historia_clinica: p.historia_clinica.clone(),
                    edad,
                    sexo: p.sexo.clone(),
                    fecha_ingreso_uci: p.fecha_ingreso_uci.clone(),
                    estado_gravedad: p.estado_gravedad.clone(),
                    ultimo_apache_score: p.ultimo_apache_score,
                    ultimo_gcs_score: p.ultimo_gcs_score,
                    ultimo_sofa_score: p.ultimo_sofa_score,
                    ultimo_saps3_score: p.ultimo_saps3_score,
                    ultimo_news2_score: p.ultimo_news2_score,
                    mortality_risk: p.mortality_risk,
                }
            }).collect();
            (StatusCode::OK, Json(ApiResponse::ok(items))).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<Vec<PatientListItem>>::err(e.to_string())),
        ).into_response(),
    }
}

// POST /api/patients
pub async fn create_patient(
    State(db): State<Database>,
    Json(patient): Json<Patient>,
) -> impl IntoResponse {
    match db_ops::create_patient(&db, patient).await {
        Ok(p) => (StatusCode::CREATED, Json(ApiResponse::ok(p))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<Patient>::err(e.to_string())),
        ).into_response(),
    }
}

// GET /api/patients/:id
pub async fn get_patient(
    State(db): State<Database>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match db_ops::get_patient(&db, &id).await {
        Ok(Some(p)) => (StatusCode::OK, Json(ApiResponse::ok(p))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<Patient>::err("Patient not found")),
        ).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<Patient>::err(e.to_string())),
        ).into_response(),
    }
}

// PUT /api/patients/:id
pub async fn update_patient(
    State(db): State<Database>,
    Path(id): Path<String>,
    Json(patient): Json<Patient>,
) -> impl IntoResponse {
    match db_ops::update_patient(&db, &id, patient).await {
        Ok(Some(p)) => (StatusCode::OK, Json(ApiResponse::ok(p))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<Patient>::err("Patient not found")),
        ).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<Patient>::err(e.to_string())),
        ).into_response(),
    }
}

// DELETE /api/patients/:id
pub async fn delete_patient(
    State(db): State<Database>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match db_ops::delete_patient(&db, &id).await {
        Ok(_) => (StatusCode::OK, Json(ApiResponse::ok(()))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()>::err(e.to_string())),
        ).into_response(),
    }
}

fn calculate_age(fecha_nacimiento: &str) -> u8 {
    use chrono::{NaiveDate, Utc};
    if fecha_nacimiento.is_empty() {
        return 0;
    }
    if let Ok(dob) = NaiveDate::parse_from_str(fecha_nacimiento, "%Y-%m-%d") {
        let today = Utc::now().date_naive();
        let years = today.years_since(dob).unwrap_or(0);
        years.min(150) as u8
    } else {
        0
    }
}
