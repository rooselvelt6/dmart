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

// ─── Scales ─────────────────────────────────────────────────────────────────

pub async fn calc_apache(patient_id: &str, data: ApacheIIData, notas: Option<String>) -> ApiResult<Value> {
    let body = serde_json::json!({ "data": data, "notas": notas });
    let resp: ApiResponse<Value> = Request::post(&format!("{}/patients/{}/scales/apache", API_BASE, patient_id))
        .json(&body).map_err(|e| e.to_string())?
        .send().await.map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn calc_gcs(patient_id: &str, apertura: u8, verbal: u8, motora: u8, notas: Option<String>) -> ApiResult<Value> {
    let body = serde_json::json!({ "apertura_ocular": apertura, "respuesta_verbal": verbal, "respuesta_motora": motora, "notas": notas });
    let resp: ApiResponse<Value> = Request::post(&format!("{}/patients/{}/scales/gcs", API_BASE, patient_id))
        .json(&body).map_err(|e| e.to_string())?
        .send().await.map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn calc_news2(patient_id: &str, fr: f32, spo2: f32, o2: bool, pas: f32, fc: f32, temp: f32, alert: bool, notas: Option<String>) -> ApiResult<Value> {
    let body = serde_json::json!({ "frecuencia_respiratoria": fr, "spo2": spo2, "o2_suplementario": o2, "presion_sistolica": pas, "frecuencia_cardiaca": fc, "temperatura": temp, "alerta": alert, "notas": notas });
    let resp: ApiResponse<Value> = Request::post(&format!("{}/patients/{}/scales/news2", API_BASE, patient_id))
        .json(&body).map_err(|e| e.to_string())?
        .send().await.map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn calc_sofa(patient_id: &str, pao2: f32, fio2: f32, plq: f32, bili: f32, pam: f32, vasopresores: bool, dosis: f32, gcs: u8, creat: f32, diuresis: u32, notas: Option<String>) -> ApiResult<Value> {
    let body = serde_json::json!({ "pao2": pao2, "fio2": fio2, "plaquetas": plq, "bilirrubina": bili, "presion_arterial_media": pam, "vasopresores": vasopresores, "dosis_vasopresor": dosis, "gcs_total": gcs, "creatinina": creat, "diuresis_diaria": diuresis, "notas": notas });
    let resp: ApiResponse<Value> = Request::post(&format!("{}/patients/{}/scales/sofa", API_BASE, patient_id))
        .json(&body).map_err(|e| e.to_string())?
        .send().await.map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn calc_saps3(patient_id: &str, edad: u8, dias: u8, tipo: Option<String>, fuente: Option<String>, notas: Option<String>) -> ApiResult<Value> {
    let body = serde_json::json!({ "edad": edad, "dias_pre_uci": dias, "tipo_admision": tipo, "fuente_admision": fuente, "presion_sistolica": 120.0, "frecuencia_cardiaca": 80.0, "gcs_total": 15, "bilirrubina": 0.8, "creatinina": 1.0, "plaquetas": 250.0, "ph_arterial": 7.4, "ventilacion_mecanica": false, "vasopresores": false, "notas": notas });
    let resp: ApiResponse<Value> = Request::post(&format!("{}/patients/{}/scales/saps3", API_BASE, patient_id))
        .json(&body).map_err(|e| e.to_string())?
        .send().await.map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn get_scales_history(patient_id: &str) -> ApiResult<Vec<Value>> {
    let resp: ApiResponse<Vec<Value>> = Request::get(&format!("{}/patients/{}/scales/history", API_BASE, patient_id))
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
