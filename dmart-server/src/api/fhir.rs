use crate::db::Database;
use anyhow::Error;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
use base64::Engine;
use dmart_shared::models::*;
use image::EncodableLayout;
use image::ImageEncoder;
use qrcode::QrCode;
use qrcode::render::svg;
use serde::Serialize;
use std::io::Cursor;

fn err_to_str(e: Error) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, crate::security::sanitize_internal_error(&e))
}

/// Mapea SeverityLevel al CodeSystem FHIR R4 `condition-severity`.
fn severity_to_fhir(level: &SeverityLevel) -> Option<FhirCodeableConcept> {
    let (code, display) = match level {
        SeverityLevel::Bajo => ("mild", "Mild"),
        SeverityLevel::Moderado => ("moderate", "Moderate"),
        SeverityLevel::Severo => ("severe", "Severe"),
        SeverityLevel::Critico => ("severe", "Severe"),
    };
    Some(FhirCodeableConcept {
        coding: vec![FhirCoding {
            system: "http://terminology.hl7.org/CodeSystem/condition-severity".to_string(),
            code: code.to_string(),
            display: display.to_string(),
        }],
        text: level.label().to_string(),
    })
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
    #[serde(rename = "type")]
    bundle_type: String,
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
    #[serde(rename = "type")]
    bundle_type: String,
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
        bundle_type: "searchset".to_string(),
        total: entries.len(),
        entry: entries,
    };

Ok(Json(serde_json::to_value(bundle).map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, crate::security::sanitize_internal_error(&e))
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
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, crate::security::sanitize_internal_error(&e)))?,
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

#[derive(Serialize)]
struct FhirConditionBundle {
    resource_type: String,
    #[serde(rename = "type")]
    bundle_type: String,
    total: usize,
    entry: Vec<FhirCondition>,
}

#[derive(Serialize)]
struct FhirCondition {
    resource_type: String,
    id: String,
    clinical_status: FhirCodeableConcept,
    category: Vec<FhirCodeableConcept>,
    severity: Option<FhirCodeableConcept>,
    code: FhirCodeableConcept,
    subject: FhirReference,
    recorded_date_time: String,
}

/// Intenta asociar el texto libre de un diagnóstico con el catálogo CIE-10.
/// Devuelve `(code, display)` si encuentra coincidencia por código o descripción.
fn match_cie10(text: &str, catalog: &[Diagnostico]) -> Option<(String, String)> {
    if text.trim().is_empty() {
        return None;
    }
    let lower = text.to_lowercase();
    // Coincidencia por código (p.ej. "A41.9", "J18.9")
    for d in catalog {
        if lower.contains(&d.codigo.to_lowercase()) {
            return Some((d.codigo.clone(), d.descripcion.clone()));
        }
    }
    // Coincidencia por descripción (fragmento >= 5 letras para evitar falsos positivos)
    for d in catalog {
        if d.descripcion.len() >= 5 && lower.contains(&d.descripcion.to_lowercase()) {
            return Some((d.codigo.clone(), d.descripcion.clone()));
        }
    }
    None
}

/// GET /fhir/Patient/{id}/Condition — diagnósticos del paciente como
/// recursos FHIR R4 (Bundle searchset). El texto se enriquece con el
/// código CIE-10 del catálogo cuando hay coincidencia.
pub async fn fhir_condition_list(
    State(db): State<Database>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let patient = crate::db::get_patient(&db, &id)
        .await
        .map_err(err_to_str)?
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Patient {} not found", id)))?;

    let catalog = crate::db::list_diagnosticos(&db)
        .await
        .map_err(err_to_str)?;

    let mut conditions = Vec::new();
    let mut idx = 0usize;

    for text in [&patient.diagnostico_uci, &patient.diagnostico_hospital] {
        if text.trim().is_empty() {
            continue;
        }
        idx += 1;
        let coding = match_cie10(text, &catalog);
        conditions.push(FhirCondition {
            resource_type: "Condition".to_string(),
            id: format!("{}-{}", patient.patient_id, idx),
            clinical_status: FhirCodeableConcept {
                coding: vec![FhirCoding {
                    system: "http://terminology.hl7.org/CodeSystem/condition-clinical".to_string(),
                    code: "active".to_string(),
                    display: "Active".to_string(),
                }],
                text: "Active".to_string(),
            },
            category: vec![FhirCodeableConcept {
                coding: vec![FhirCoding {
                    system: "http://terminology.hl7.org/CodeSystem/condition-category".to_string(),
                    code: "encounter-diagnosis".to_string(),
                    display: "Encounter Diagnosis".to_string(),
                }],
                text: "Diagnóstico".to_string(),
            }],
            severity: severity_to_fhir(&patient.estado_gravedad),
            code: match coding {
                Some((code, display)) => FhirCodeableConcept {
                    coding: vec![FhirCoding {
                        system: "http://hl7.org/fhir/sid/icd-10".to_string(),
                        code,
                        display,
                    }],
                    text: text.clone(),
                },
                None => FhirCodeableConcept {
                    coding: vec![],
                    text: text.clone(),
                },
            },
            subject: FhirReference {
                reference: format!("Patient/{}", patient.patient_id),
            },
            recorded_date_time: patient.fecha_ingreso_uci.clone(),
        });
    }

    let bundle = FhirConditionBundle {
        resource_type: "Bundle".to_string(),
        bundle_type: "searchset".to_string(),
        total: conditions.len(),
        entry: conditions,
    };

    Ok(Json(serde_json::to_value(bundle).map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?))
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
        bundle_type: "searchset".to_string(),
        total: observations.len(),
        entry: observations,
    };

    Ok(Json(serde_json::to_value(bundle).map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?))
}

/// Genera un código QR como SVG inline con la URL del reporte FHIR.
fn generate_qr_code_svg(url: &str) -> anyhow::Result<String> {
    let code = QrCode::new(url)?;
    let svg = code
        .render()
        .min_dimensions(200, 200)
        .dark_color(svg::Color("#000000"))
        .light_color(svg::Color("#ffffff"))
        .build();
    Ok(svg)
}

/// Genera un código QR como PNG base64 para embed en PDF.
fn generate_qr_code_png_base64(url: &str) -> anyhow::Result<String> {
    let code = QrCode::new(url)?;
    let image = code
        .render::<image::Luma<u8>>()
        .min_dimensions(200, 200)
        .dark_color(image::Luma([0u8]))
        .light_color(image::Luma([255u8]))
        .build();

    let mut png_data = Vec::new();
    {
        let mut cursor = Cursor::new(&mut png_data);
        let encoder = image::codecs::png::PngEncoder::new(&mut cursor);
        encoder.write_image(
            image.as_bytes(),
            image.width(),
            image.height(),
            image::ExtendedColorType::L8,
        )?;
    }
    Ok(base64::engine::general_purpose::STANDARD.encode(&png_data))
}

#[derive(Serialize)]
struct FhirDiagnosticReportBundle {
    resource_type: String,
    #[serde(rename = "type")]
    bundle_type: String,
    total: usize,
    entry: Vec<FhirDiagnosticReport>,
}

#[derive(Serialize)]
struct FhirDiagnosticReport {
    resource_type: String,
    id: String,
    status: String,
    category: Vec<FhirCodeableConcept>,
    code: FhirCodeableConcept,
    subject: FhirReference,
    encounter: FhirReference,
    effective_date_time: String,
    issued: String,
    performer: Vec<FhirReference>,
    results_interpreter: Vec<FhirReference>,
    conclusion: String,
    conclusion_code: Vec<FhirCodeableConcept>,
    presented_form: Vec<FhirAttachment>,
}

#[derive(Serialize)]
struct FhirAttachment {
    content_type: String,
    #[serde(rename = "url")]
    url: String,
    title: String,
}

/// GET /fhir/Patient/{id}/DiagnosticReport — reporte diagnóstico FHIR R4
/// con QR code que apunta al PDF del reporte.
pub async fn fhir_diagnostic_report(
    State(db): State<Database>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let patient = crate::db::get_patient(&db, &id)
        .await
        .map_err(err_to_str)?
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Patient {} not found", id)))?;

    let _measurements = crate::db::get_measurements_for_patient(&db, &id)
        .await
        .map_err(err_to_str)?;

    let report_id = format!("diag-report-{}", patient.patient_id);
    let pdf_url = format!("/api/patients/{}/export/pdf", patient.patient_id);
    let qr_data = format!("https://dmart.local{}", pdf_url);

    let _qr_svg = generate_qr_code_svg(&qr_data).unwrap_or_default();

    let mut conclusion_codes = Vec::new();
    if let Some(apache) = patient.ultimo_apache_score {
        conclusion_codes.push(FhirCodeableConcept {
            coding: vec![FhirCoding {
                system: "http://dmart.local/fhir/CodeSystem/scores".to_string(),
                code: "apache2".to_string(),
                display: format!("APACHE II: {}", apache),
            }],
            text: format!("APACHE II: {}", apache),
        });
    }
    if let Some(sofa) = patient.ultimo_sofa_score {
        conclusion_codes.push(FhirCodeableConcept {
            coding: vec![FhirCoding {
                system: "http://dmart.local/fhir/CodeSystem/scores".to_string(),
                code: "sofa".to_string(),
                display: format!("SOFA: {}", sofa),
            }],
            text: format!("SOFA: {}", sofa),
        });
    }
    if let Some(news2) = patient.ultimo_news2_score {
        conclusion_codes.push(FhirCodeableConcept {
            coding: vec![FhirCoding {
                system: "http://dmart.local/fhir/CodeSystem/scores".to_string(),
                code: "news2".to_string(),
                display: format!("NEWS2: {}", news2),
            }],
            text: format!("NEWS2: {}", news2),
        });
    }

    let conclusion = format!(
        "Paciente {} {} — Ingreso UCI: {} — Diagnóstico UCI: {} — Estado: {} — Mortalidad estimada: {}",
        patient.nombre,
        patient.apellido,
        patient.fecha_ingreso_uci,
        patient.diagnostico_uci,
        patient.estado_gravedad.label(),
        patient.estado_gravedad.mortality_estimate()
    );

    let report = FhirDiagnosticReport {
        resource_type: "DiagnosticReport".to_string(),
        id: report_id.clone(),
        status: "final".to_string(),
        category: vec![FhirCodeableConcept {
            coding: vec![FhirCoding {
                system: "http://terminology.hl7.org/CodeSystem/v2-0074".to_string(),
                code: "RAD".to_string(),
                display: "Radiología".to_string(),
            }],
            text: "Resumen Clínico UCI".to_string(),
        }],
        code: FhirCodeableConcept {
            coding: vec![FhirCoding {
                system: "http://loinc.org".to_string(),
                code: "11502-2".to_string(),
                display: "Laboratory report".to_string(),
            }],
            text: "Resumen de Evolución UCI".to_string(),
        },
        subject: FhirReference {
            reference: format!("Patient/{}", patient.patient_id),
        },
        encounter: FhirReference {
            reference: format!("Encounter/{}", patient.patient_id),
        },
        effective_date_time: patient.fecha_ingreso_uci.clone(),
        issued: chrono::Utc::now().to_rfc3339(),
        performer: vec![FhirReference {
            reference: format!("Practitioner/{}", patient.patient_id),
        }],
        results_interpreter: vec![FhirReference {
            reference: format!("Practitioner/{}", patient.patient_id),
        }],
        conclusion,
        conclusion_code: conclusion_codes,
        presented_form: vec![FhirAttachment {
            content_type: "application/pdf".to_string(),
            url: pdf_url,
            title: format!("Reporte UCI - {} {}", patient.nombre, patient.apellido),
        }],
    };

    let bundle = FhirDiagnosticReportBundle {
        resource_type: "Bundle".to_string(),
        bundle_type: "searchset".to_string(),
        total: 1,
        entry: vec![report],
    };

    Ok(Json(serde_json::to_value(bundle).map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?))
}

/// GET /fhir/Patient/{id}/DiagnosticReport/QR — devuelve el QR code SVG
/// para el reporte diagnóstico del paciente.
pub async fn fhir_diagnostic_report_qr(
    State(db): State<Database>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let patient = crate::db::get_patient(&db, &id)
        .await
        .map_err(err_to_str)?
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Patient {} not found", id)))?;

    let pdf_url = format!("/api/patients/{}/export/pdf", patient.patient_id);
    let qr_data = format!("https://dmart.local{}", pdf_url);

    let qr_svg =
        generate_qr_code_svg(&qr_data).unwrap_or_else(|_| "Error generando QR".to_string());
    let qr_png_b64 = generate_qr_code_png_base64(&qr_data).unwrap_or_default();

    Ok(Json(serde_json::json!({
        "patient_id": patient.patient_id,
        "qr_svg": qr_svg,
        "qr_png_base64": qr_png_b64,
        "pdf_url": pdf_url,
        "qr_data": qr_data
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_qr_code_svg() {
        let url = "https://dmart.local/api/patients/test/export/pdf";
        let svg = generate_qr_code_svg(url).expect("QR SVG generation should succeed");
        assert!(svg.contains("<svg"));
        assert!(svg.contains("</svg>"));
        assert!(!svg.is_empty());
    }

    #[test]
    fn test_generate_qr_code_png_base64() {
        let url = "https://dmart.local/api/patients/test/export/pdf";
        let png_b64 =
            generate_qr_code_png_base64(url).expect("QR PNG base64 generation should succeed");
        assert!(!png_b64.is_empty());
        // Verificar que es base64 válido
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(&png_b64)
            .expect("Should be valid base64");
        // PNG magic bytes: 89 50 4E 47 0D 0A 1A 0A
        assert_eq!(
            &decoded[0..8],
            &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
        );
    }

    #[test]
    fn test_qr_code_roundtrip() {
        let url = "https://dmart.local/api/patients/123/export/pdf";
        let svg = generate_qr_code_svg(url).expect("SVG generation");
        let png_b64 = generate_qr_code_png_base64(url).expect("PNG generation");

        // Ambos deberían generar sin error
        assert!(svg.len() > 100);
        assert!(png_b64.len() > 100);
    }
}
