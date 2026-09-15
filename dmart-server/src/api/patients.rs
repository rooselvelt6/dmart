use crate::auth::Claims;
use crate::db as db_ops;
use crate::db::Database;
use crate::patient_timeline::{
    TimelineQuery, TimelineResponse, query_timeline, to_fhir_history_bundle,
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
};
use dmart_shared::models::*;
use serde::Deserialize;

#[derive(Deserialize, Default)]
pub struct DesenlaceQuery {
    /// Resultado clínico del egreso: `Mejorado`, `Trasladado`, `Fallecido`.
    pub desenlace: Option<String>,
}

// GET /api/patients?q=<search>&limit=50&offset=0

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
    let pagination = PaginationParams {
        limit: params.limit,
        offset: params.offset,
    };
    let limit = pagination.limit();
    let offset = pagination.offset();

    if let Some(q) = params.q.filter(|s| !s.is_empty()) {
        let result = db_ops::search_patients(&db, &q, limit, offset).await;
        let total = db_ops::search_patients_count(&db, &q).await.unwrap_or(0);
        match result {
            Ok(patients) => {
                let items = patients_to_list_items(&patients);
                (
                    StatusCode::OK,
                    Json(ApiResponse::ok(PaginatedResponse {
                        items,
                        total,
                        limit,
                        offset,
                    })),
                )
                    .into_response()
            }
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<PaginatedResponse<PatientListItem>>::err(
                    e.to_string(),
                )),
            )
                .into_response(),
        }
    } else {
        let result = db_ops::list_patients(&db, limit, offset).await;
        let total = db_ops::count_patients(&db).await.unwrap_or(0);
        match result {
            Ok(patients) => {
                let items = patients_to_list_items(&patients);
                (
                    StatusCode::OK,
                    Json(ApiResponse::ok(PaginatedResponse {
                        items,
                        total,
                        limit,
                        offset,
                    })),
                )
                    .into_response()
            }
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<PaginatedResponse<PatientListItem>>::err(
                    e.to_string(),
                )),
            )
                .into_response(),
        }
    }
}

fn patients_to_list_items(patients: &[Patient]) -> Vec<PatientListItem> {
    patients
        .iter()
        .map(|p| {
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
        })
        .collect()
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

    match db_ops::create_patient_with_assignments(&db, patient, &equipos_ids).await {
        Ok(p) => {
            if let Some(audit) = crate::audit::audit() {
                let _ = audit
                    .log_patient_access(
                        &claims.sub,
                        &claims.username,
                        &pid,
                        crate::audit::AuditAction::Create,
                        true,
                    )
                    .await;
            }
            (StatusCode::CREATED, Json(ApiResponse::ok(p))).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<Patient>::err(e.to_string())),
        )
            .into_response(),
    }
}

// GET /api/patients/:id
pub async fn get_patient(
    State(db): State<Database>,
    Path(id): Path<String>,
    claims: Claims,
) -> impl IntoResponse {
    if let Some(audit) = crate::audit::audit() {
        let _ = audit
            .log_patient_access(
                &claims.sub,
                &claims.username,
                &id,
                crate::audit::AuditAction::Read,
                true,
            )
            .await;
    }
    match db_ops::get_patient(&db, &id).await {
        Ok(Some(p)) => (StatusCode::OK, Json(ApiResponse::ok(p))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<Patient>::err("Patient not found")),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<Patient>::err(e.to_string())),
        )
            .into_response(),
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
        let _ = audit
            .log_patient_access(
                &claims.sub,
                &claims.username,
                &id,
                crate::audit::AuditAction::Update,
                true,
            )
            .await;
    }
    match db_ops::update_patient(&db, &id, patient).await {
        Ok(Some(p)) => (StatusCode::OK, Json(ApiResponse::ok(p))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<Patient>::err("Patient not found")),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<Patient>::err(e.to_string())),
        )
            .into_response(),
    }
}

// DELETE /api/patients/:id
pub async fn delete_patient(
    State(db): State<Database>,
    Path(id): Path<String>,
    claims: Claims,
) -> impl IntoResponse {
    if let Some(audit) = crate::audit::audit() {
        let _ = audit
            .log_patient_access(
                &claims.sub,
                &claims.username,
                &id,
                crate::audit::AuditAction::Delete,
                true,
            )
            .await;
    }
    match db_ops::delete_patient(&db, &id).await {
        Ok(_) => (StatusCode::OK, Json(ApiResponse::ok(()))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()>::err(e.to_string())),
        )
            .into_response(),
    }
}

// POST /api/patients/:id/egreso - Liberar cama al egresar paciente
pub async fn egreso_paciente(
    State(db): State<Database>,
    Path(id): Path<String>,
    Query(params): Query<DesenlaceQuery>,
    claims: Claims,
) -> impl IntoResponse {
    let desenlace = params.desenlace.unwrap_or_else(|| "Mejorado".to_string());
    if let Some(audit) = crate::audit::audit() {
        let _ = audit
            .log_patient_access(
                &claims.sub,
                &claims.username,
                &id,
                crate::audit::AuditAction::Update,
                true,
            )
            .await;
    }
    let paciente = db_ops::get_patient(&db, &id).await;

    match paciente {
        Ok(Some(p)) => match db_ops::egresar_paciente(&db, &p, &desenlace).await {
            Ok(_) => (
                StatusCode::OK,
                Json(ApiResponse::ok(
                    "Paciente egresado, cama y equipos liberados",
                )),
            )
                .into_response(),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<String>::err(e.to_string())),
            )
                .into_response(),
        },
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<String>::err("Paciente no encontrado")),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<String>::err(e.to_string())),
        )
            .into_response(),
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

// GET /api/patients/:id/timeline
pub async fn patient_timeline(
    State(db): State<Database>,
    Path(id): Path<String>,
    Query(query): Query<TimelineQuery>,
    headers: HeaderMap,
    claims: Claims,
) -> impl IntoResponse {
    if let Some(audit) = crate::audit::audit() {
        let _ = audit
            .log_patient_access(
                &claims.sub,
                &claims.username,
                &id,
                crate::audit::AuditAction::Read,
                true,
            )
            .await;
    }

    let wants_fhir = headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.contains("application/fhir+json"))
        .unwrap_or(false);

    match query_timeline(&db, &id, query).await {
        Ok(response) => {
            if wants_fhir {
                return (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, "application/fhir+json")],
                    Json(to_fhir_history_bundle(&response.events)),
                )
                    .into_response();
            }
            (
                StatusCode::OK,
                Json(dmart_shared::models::ApiResponse::ok(response)),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(dmart_shared::models::ApiResponse::<TimelineResponse>::err(
                e.to_string(),
            )),
        )
            .into_response(),
    }
}
