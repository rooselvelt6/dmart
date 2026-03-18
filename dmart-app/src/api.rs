/// HTTP client — communicates with the Axum backend API
use dmart_shared::models::*;
use gloo_net::http::Request;
use serde_json::Value;

const API_BASE: &str = "/api";

pub type ApiResult<T> = Result<T, String>;

// ─── Patients ──────────────────────────────────────────────────────────────

pub async fn list_patients(query: Option<&str>) -> ApiResult<Vec<PatientListItem>> {
    let url = match query {
        Some(q) if !q.is_empty() => format!("{}/patients?q={}", API_BASE, q),
        _ => format!("{}/patients", API_BASE),
    };
    let resp: ApiResponse<Vec<PatientListItem>> =
        Request::get(&url).send().await
            .map_err(|e| e.to_string())?
            .json().await
            .map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn get_patient(id: &str) -> ApiResult<Patient> {
    let resp: ApiResponse<Patient> =
        Request::get(&format!("{}/patients/{}", API_BASE, id))
            .send().await.map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn create_patient(patient: &Patient) -> ApiResult<Patient> {
    let resp: ApiResponse<Patient> =
        Request::post(&format!("{}/patients", API_BASE))
            .json(patient).map_err(|e| e.to_string())?
            .send().await.map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn update_patient(id: &str, patient: &Patient) -> ApiResult<Patient> {
    let resp: ApiResponse<Patient> =
        Request::put(&format!("{}/patients/{}", API_BASE, id))
            .json(patient).map_err(|e| e.to_string())?
            .send().await.map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn delete_patient(id: &str) -> ApiResult<()> {
    Request::delete(&format!("{}/patients/{}", API_BASE, id))
        .send().await.map_err(|e| e.to_string())?;
    Ok(())
}

// ─── Measurements ──────────────────────────────────────────────────────────

pub async fn get_measurements(patient_id: &str) -> ApiResult<Vec<Measurement>> {
    let resp: ApiResponse<Vec<Measurement>> =
        Request::get(&format!("{}/patients/{}/measurements", API_BASE, patient_id))
            .send().await.map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn get_last_measurement(patient_id: &str) -> ApiResult<Option<Measurement>> {
    let resp: ApiResponse<Option<Measurement>> =
        Request::get(&format!("{}/patients/{}/measurements/last", API_BASE, patient_id))
            .send().await.map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;
    Ok(resp.data.flatten())
}

pub async fn create_measurement(
    patient_id: &str,
    apache: ApacheIIData,
    gcs: GcsData,
    notas: String,
) -> ApiResult<Measurement> {
    use serde_json::json;
    let body = json!({
        "apache_data": apache,
        "gcs_data": gcs,
        "notas": notas,
    });
    let resp: ApiResponse<Measurement> =
        Request::post(&format!("{}/patients/{}/measurements", API_BASE, patient_id))
            .json(&body).map_err(|e| e.to_string())?
            .send().await.map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

// ─── Export helpers ─────────────────────────────────────────────────────────

pub fn export_csv_url(patient_id: &str) -> String {
    format!("{}/patients/{}/export/csv", API_BASE, patient_id)
}

pub fn export_pdf_url(patient_id: &str) -> String {
    format!("{}/patients/{}/export/pdf", API_BASE, patient_id)
}
