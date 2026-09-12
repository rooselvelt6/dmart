use crate::db::Database;
use anyhow::Error;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
use dmart_shared::models::*;
use serde::Serialize;

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
struct FhirObservationBundle {
    resource_type: String,
    total: usize,
    entry: Vec<FhirObservation>,
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
    let fhir_count = params._count.unwrap_or(50).min(200) as u32;
    let patients = crate::db::list_patients(&db, fhir_count, 0)
        .await
        .map_err(err_to_str)?;

    let entries: Vec<FhirBundleEntry> = if let Some(name) = &params.name {
        let lower = name.to_lowercase();
        patients
            .iter()
            .filter(|p| {
                p.nombre.to_lowercase().contains(&lower)
                    || p.apellido.to_lowercase().contains(&lower)
            })
            .take(fhir_count as usize)
            .map(|p| FhirBundleEntry {
                resource: patient_to_fhir(p),
            })
            .collect()
    } else {
        patients
            .iter()
            .map(|p| FhirBundleEntry {
                resource: patient_to_fhir(p),
            })
            .collect()
    };

    let bundle = FhirBundle {
        resource_type: "Bundle".to_string(),
        total: entries.len(),
        entry: entries,
    };

    Ok(Json(serde_json::to_value(bundle).map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?))
}

pub async fn fhir_patient_get(
    State(db): State<Database>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let patient = crate::db::get_patient(&db, &id)
        .await
        .map_err(err_to_str)?
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Patient {} not found", id)))?;

    Ok(Json(
        serde_json::to_value(patient_to_fhir(&patient))
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?,
    ))
}

fn score_observation(
    patient_id: &str,
    code: &str,
    display: &str,
    value: u32,
    unit: &str,
    effective: &str,
) -> FhirObservation {
    FhirObservation {
        resource_type: "Observation".to_string(),
        id: format!("{}-{}", patient_id, code),
        status: "final".to_string(),
        code: FhirCodeableConcept {
            coding: vec![FhirCoding {
                system: "http://dmart.local/fhir/CodeSystem/scores".to_string(),
                code: code.to_string(),
                display: display.to_string(),
            }],
            text: display.to_string(),
        },
        subject: FhirReference {
            reference: format!("Patient/{}", patient_id),
        },
        effective_date_time: effective.to_string(),
        value_quantity: Some(FhirQuantity {
            value: value as f32,
            unit: unit.to_string(),
        }),
    }
}

/// GET /fhir/Patient/{id}/Observation — escalas y scores del paciente como
/// recursos FHIR R4 (Bundle de tipo searchset).
pub async fn fhir_observation_list(
    State(db): State<Database>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let patient = crate::db::get_patient(&db, &id)
        .await
        .map_err(err_to_str)?
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Patient {} not found", id)))?;

    let effective = patient.updated_at.clone();
    let mut observations = Vec::new();

    if let Some(gcs) = patient.ultimo_gcs_score {
        observations.push(score_observation(
            &patient.patient_id,
            "gcs",
            "Escala de coma de Glasgow (GCS)",
            gcs as u32,
            "puntos",
            &effective,
        ));
    }
    if let Some(apache) = patient.ultimo_apache_score {
        observations.push(score_observation(
            &patient.patient_id,
            "apache2",
            "APACHE II score",
            apache,
            "puntos",
            &effective,
        ));
    }
    if let Some(sofa) = patient.ultimo_sofa_score {
        observations.push(score_observation(
            &patient.patient_id,
            "sofa",
            "SOFA score",
            sofa,
            "puntos",
            &effective,
        ));
    }

    let bundle = FhirObservationBundle {
        resource_type: "Bundle".to_string(),
        total: observations.len(),
        entry: observations,
    };

    Ok(Json(serde_json::to_value(bundle).map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?))
}
