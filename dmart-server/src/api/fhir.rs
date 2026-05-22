#![allow(dead_code)]

use axum::{
    extract::{Path, State, Query},
    http::StatusCode,
    response::Json,
};
use dmart_shared::models::*;
use crate::db::Database;
use serde::Serialize;
use anyhow::Error;

type ApiResult<T> = Result<Json<ApiResponse<T>>, (StatusCode, String)>;

fn err_to_str(e: Error) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

#[derive(Serialize)]
struct FhirPatient {
    resource_type: String,
    id: String,
    name: Vec<FhirHumanName>,
    gender: String,
    birth_date: String,
    identifier: Vec<FhirIdentifier>,
}

#[derive(Serialize)]
struct FhirHumanName {
    use_: String,
    family: String,
    given: Vec<String>,
}

#[derive(Serialize)]
struct FhirIdentifier {
    system: String,
    value: String,
}

#[derive(Serialize)]
struct FhirBundle {
    resource_type: String,
    total: usize,
    entry: Vec<FhirBundleEntry>,
}

#[derive(Serialize)]
struct FhirBundleEntry {
    resource: FhirPatient,
}

#[derive(Serialize)]
struct FhirObservation {
    resource_type: String,
    id: String,
    status: String,
    code: FhirCodeableConcept,
    subject: FhirReference,
    effective_date_time: String,
    value_quantity: Option<FhirQuantity>,
}

#[derive(Serialize)]
struct FhirCodeableConcept {
    coding: Vec<FhirCoding>,
    text: String,
}

#[derive(Serialize)]
struct FhirCoding {
    system: String,
    code: String,
    display: String,
}

#[derive(Serialize)]
struct FhirReference {
    reference: String,
}

#[derive(Serialize)]
struct FhirQuantity {
    value: f32,
    unit: String,
}

fn patient_to_fhir(p: &Patient) -> FhirPatient {
    FhirPatient {
        resource_type: "Patient".to_string(),
        id: p.patient_id.clone(),
        name: vec![FhirHumanName {
            use_: "official".to_string(),
            family: p.apellido.clone(),
            given: vec![p.nombre.clone()],
        }],
        gender: match p.sexo {
            Sexo::Masculino => "male".to_string(),
            Sexo::Femenino => "female".to_string(),
        },
        birth_date: p.fecha_nacimiento.clone(),
        identifier: vec![
            FhirIdentifier {
                system: "http://example.com/cedula".to_string(),
                value: p.cedula.clone(),
            },
            FhirIdentifier {
                system: "http://example.com/historia_clinica".to_string(),
                value: p.historia_clinica.clone(),
            },
        ],
    }
}

#[derive(serde::Deserialize)]
pub struct FhirSearchQuery {
    pub _count: Option<usize>,
    pub name: Option<String>,
    pub identifier: Option<String>,
}

pub async fn fhir_patient_search(
    State(db): State<Database>,
    Query(params): Query<FhirSearchQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let patients = crate::db::list_patients(&db).await.map_err(err_to_str)?;

    let filtered: Vec<&Patient> = if let Some(name) = &params.name {
        let lower = name.to_lowercase();
        patients.iter().filter(|p| {
            p.nombre.to_lowercase().contains(&lower) ||
            p.apellido.to_lowercase().contains(&lower)
        }).collect()
    } else {
        patients.iter().collect()
    };

    let count = params._count.unwrap_or(50);
    let entries: Vec<FhirBundleEntry> = filtered
        .into_iter()
        .take(count)
        .map(|p| FhirBundleEntry {
            resource: patient_to_fhir(p),
        })
        .collect();

    let bundle = FhirBundle {
        resource_type: "Bundle".to_string(),
        total: entries.len(),
        entry: entries,
    };

    Ok(Json(serde_json::to_value(bundle).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?))
}

pub async fn fhir_patient_get(
    State(db): State<Database>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let patient = crate::db::get_patient(&db, &id)
        .await
        .map_err(err_to_str)?
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Patient {} not found", id)))?;

    Ok(Json(serde_json::to_value(patient_to_fhir(&patient))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?))
}
