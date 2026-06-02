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
use crate::auth::Claims;

#[derive(Deserialize)]
pub struct ListPatientsQuery {
    pub q: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

// GET /api/patients?q=<search>&limit=50&offset=0
pub async fn list_patients(
    State(db): State<Database>,
    Query(params): Query<ListPatientsQuery>,
) -> impl IntoResponse {
    let pagination = PaginationParams { limit: params.limit, offset: params.offset };
    let limit = pagination.limit();
    let offset = pagination.offset();

    if let Some(q) = params.q.filter(|s| !s.is_empty()) {
        let result = db_ops::search_patients(&db, &q, limit, offset).await;
        let total = db_ops::search_patients_count(&db, &q).await.unwrap_or(0);
        match result {
            Ok(patients) => {
                let items = patients_to_list_items(&patients);
                (StatusCode::OK, Json(ApiResponse::ok(PaginatedResponse { items, total, limit, offset }))).into_response()
            }
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<PaginatedResponse<PatientListItem>>::err(e.to_string())),
            ).into_response(),
        }
    } else {
        let result = db_ops::list_patients(&db, limit, offset).await;
        let total = db_ops::count_patients(&db).await.unwrap_or(0);
        match result {
            Ok(patients) => {
                let items = patients_to_list_items(&patients);
                (StatusCode::OK, Json(ApiResponse::ok(PaginatedResponse { items, total, limit, offset }))).into_response()
            }
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<PaginatedResponse<PatientListItem>>::err(e.to_string())),
            ).into_response(),
        }
    }
}

fn patients_to_list_items(patients: &[Patient]) -> Vec<PatientListItem> {
    patients.iter().map(|p| {
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
    }).collect()
}

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
}

#[derive(Deserialize)]
pub struct CreatePatientRequest {
    #[serde(flatten)]
    pub patient: Patient,
    #[serde(default)]
    pub equipos_ids: Vec<String>,
}

// POST /api/patients
pub async fn create_patient(
    claims: Claims,
    State(db): State<Database>,
    Json(req): Json<CreatePatientRequest>,
) -> impl IntoResponse {
    let patient = req.patient;
    let equipos_ids = req.equipos_ids;
    let pid = patient.patient_id.clone();

    if let (Some(cama_id), Some(_pnombre)) = (&patient.cama_id, &patient.cama_numero) {
        let nombre_completo = patient.nombre_completo();
        if let Err(e) = db_ops::asignar_cama_paciente(&db, cama_id, &patient.patient_id, &nombre_completo).await {
            return (StatusCode::BAD_REQUEST, Json(ApiResponse::<Patient>::err(format!("Error asignando cama: {}", e)))).into_response();
        }

        for equipo_id in &equipos_ids {
            let _ = db_ops::asignar_equipo_cama(&db, equipo_id, cama_id).await;
        }
    }

    match db_ops::create_patient(&db, patient).await {
        Ok(p) => {
            if let Some(audit) = crate::audit::audit() {
                let _ = audit.log_patient_access(&claims.sub, &claims.username, &pid, crate::audit::AuditAction::Create, true).await;
            }
            (StatusCode::CREATED, Json(ApiResponse::ok(p))).into_response()
        }
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
    claims: Claims,
) -> impl IntoResponse {
    if let Some(audit) = crate::audit::audit() {
        let _ = audit.log_patient_access(&claims.sub, &claims.username, &id, crate::audit::AuditAction::Read, true).await;
    }
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
    claims: Claims,
    Json(patient): Json<Patient>,
) -> impl IntoResponse {
    if let Some(audit) = crate::audit::audit() {
        let _ = audit.log_patient_access(&claims.sub, &claims.username, &id, crate::audit::AuditAction::Update, true).await;
    }
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
    claims: Claims,
) -> impl IntoResponse {
    if let Some(audit) = crate::audit::audit() {
        let _ = audit.log_patient_access(&claims.sub, &claims.username, &id, crate::audit::AuditAction::Delete, true).await;
    }
    match db_ops::delete_patient(&db, &id).await {
        Ok(_) => (StatusCode::OK, Json(ApiResponse::ok(()))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()>::err(e.to_string())),
        ).into_response(),
    }
}

// POST /api/patients/:id/egreso - Liberar cama al egresar paciente
pub async fn egreso_paciente(
    State(db): State<Database>,
    Path(id): Path<String>,
    claims: Claims,
) -> impl IntoResponse {
    if let Some(audit) = crate::audit::audit() {
        let _ = audit.log_patient_access(&claims.sub, &claims.username, &id, crate::audit::AuditAction::Update, true).await;
    }
    let paciente = db_ops::get_patient(&db, &id).await;
    
    match paciente {
        Ok(Some(p)) => {
            if let Some(cama_id) = &p.cama_id {
                let _ = db_ops::liberar_equipos_de_cama(&db, cama_id).await;
                let _ = db_ops::liberar_cama(&db, cama_id).await;
            }
            (StatusCode::OK, Json(ApiResponse::ok("Paciente egresado, cama y equipos liberados"))).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<String>::err("Paciente no encontrado")),
        ).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<String>::err(e.to_string())),
        ).into_response(),
    }
}

pub fn calculate_age(fecha_nacimiento: &str) -> u8 {
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
