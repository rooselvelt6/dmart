/// HTTP client — communicates with the Axum backend API
use dmart_shared::models::*;
use gloo_net::http::Request;
use gloo_storage::{LocalStorage, Storage};
use serde::Deserialize;
use serde_json::Value;

const API_BASE: &str = "/api";

pub type ApiResult<T> = Result<T, String>;

fn authed_get(url: &str) -> gloo_net::http::RequestBuilder {
    let req = Request::get(url);
    if let Ok(token) = LocalStorage::get::<String>("dmart_auth") {
        req.header("Authorization", &format!("Bearer {}", token))
    } else {
        req
    }
}

fn authed_post(url: &str) -> gloo_net::http::RequestBuilder {
    let req = Request::post(url);
    if let Ok(token) = LocalStorage::get::<String>("dmart_auth") {
        req.header("Authorization", &format!("Bearer {}", token))
    } else {
        req
    }
}

fn authed_put(url: &str) -> gloo_net::http::RequestBuilder {
    let req = Request::put(url);
    if let Ok(token) = LocalStorage::get::<String>("dmart_auth") {
        req.header("Authorization", &format!("Bearer {}", token))
    } else {
        req
    }
}

fn authed_delete(url: &str) -> gloo_net::http::RequestBuilder {
    let req = Request::delete(url);
    if let Ok(token) = LocalStorage::get::<String>("dmart_auth") {
        req.header("Authorization", &format!("Bearer {}", token))
    } else {
        req
    }
}

// ─── Auth ───────────────────────────────────────────────────────────────────

pub async fn login(username: &str, password: &str) -> ApiResult<LoginResponse> {
    let body = serde_json::json!({ "username": username, "password": password });
    let resp: ApiResponse<LoginResponse> =
        authed_post(&format!("{}/auth/login", API_BASE))
            .json(&body).map_err(|e| e.to_string())?
            .send().await.map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

// ─── Patients ──────────────────────────────────────────────────────────────

pub async fn list_patients(query: Option<&str>) -> ApiResult<Vec<PatientListItem>> {
    let url = match query {
        Some(q) if !q.is_empty() => format!("{}/patients?q={}", API_BASE, q),
        _ => format!("{}/patients", API_BASE),
    };
    let resp: ApiResponse<Vec<PatientListItem>> =
        authed_get(&url).send().await
            .map_err(|e| e.to_string())?
            .json().await
            .map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

#[derive(Debug, Clone, Deserialize)]
pub struct UciStatsResponse {
    pub total_pacientes: usize,
    pub pacientes_activos: usize,
    pub por_gravedad: GravedadStats,
    pub promedios: PromedioScores,
    pub reciente: Vec<PatientListItem>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GravedadStats {
    pub criticos: usize,
    pub severos: usize,
    pub moderados: usize,
    pub bajos: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PromedioScores {
    pub apache_promedio: f32,
    pub gcs_promedio: f32,
    pub sofa_promedio: f32,
    pub saps3_promedio: f32,
    pub news2_promedio: f32,
}

pub async fn get_stats() -> ApiResult<UciStatsResponse> {
    let url = format!("{}/stats", API_BASE);
    let resp = authed_get(&url).send().await
        .map_err(|e| format!("Request failed: {}", e))?;

    let status = resp.status();
    if status < 200 || status >= 300 {
        return Err(format!("API error: status {}", status));
    }

    let resp: ApiResponse<UciStatsResponse> = resp.json().await
        .map_err(|e| format!("JSON parse error: {}", e))?;

    resp.data.ok_or_else(|| resp.error.unwrap_or_else(|| "No data".to_string()))
}

pub async fn get_patient(id: &str) -> ApiResult<Patient> {
    let resp: ApiResponse<Patient> =
        authed_get(&format!("{}/patients/{}", API_BASE, id))
            .send().await.map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn create_patient(patient: &Patient) -> ApiResult<Patient> {
    let resp: ApiResponse<Patient> =
        authed_post(&format!("{}/patients", API_BASE))
            .json(patient).map_err(|e| e.to_string())?
            .send().await.map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn update_patient(id: &str, patient: &Patient) -> ApiResult<Patient> {
    let resp: ApiResponse<Patient> =
        authed_put(&format!("{}/patients/{}", API_BASE, id))
            .json(patient).map_err(|e| e.to_string())?
            .send().await.map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn delete_patient(id: &str) -> ApiResult<()> {
    authed_delete(&format!("{}/patients/{}", API_BASE, id))
        .send().await.map_err(|e| e.to_string())?;
    Ok(())
}

// ─── Measurements ──────────────────────────────────────────────────────────

pub async fn get_measurements(patient_id: &str) -> ApiResult<Vec<Measurement>> {
    let resp: ApiResponse<Vec<Measurement>> =
        authed_get(&format!("{}/patients/{}/measurements", API_BASE, patient_id))
            .send().await.map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn get_last_measurement(patient_id: &str) -> ApiResult<Option<Measurement>> {
    let resp: ApiResponse<Option<Measurement>> =
        authed_get(&format!("{}/patients/{}/measurements/last", API_BASE, patient_id))
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
        authed_post(&format!("{}/patients/{}/measurements", API_BASE, patient_id))
            .json(&body).map_err(|e| e.to_string())?
            .send().await.map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

// ─── Scales ─────────────────────────────────────────────────────────────────

pub async fn calc_apache(patient_id: &str, data: ApacheIIData, notas: Option<String>) -> ApiResult<Value> {
    let body = serde_json::json!({ "data": data, "notas": notas });
    let resp: ApiResponse<Value> = authed_post(&format!("{}/patients/{}/scales/apache", API_BASE, patient_id))
        .json(&body).map_err(|e| e.to_string())?
        .send().await.map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn calc_gcs(patient_id: &str, apertura: u8, verbal: u8, motora: u8, notas: Option<String>) -> ApiResult<Value> {
    let body = serde_json::json!({ "apertura_ocular": apertura, "respuesta_verbal": verbal, "respuesta_motora": motora, "notas": notas });
    let resp: ApiResponse<Value> = authed_post(&format!("{}/patients/{}/scales/gcs", API_BASE, patient_id))
        .json(&body).map_err(|e| e.to_string())?
        .send().await.map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn calc_news2(patient_id: &str, fr: f32, spo2: f32, o2: bool, pas: f32, fc: f32, temp: f32, alert: bool, notas: Option<String>) -> ApiResult<Value> {
    let body = serde_json::json!({ "frecuencia_respiratoria": fr, "spo2": spo2, "o2_suplementario": o2, "presion_sistolica": pas, "frecuencia_cardiaca": fc, "temperatura": temp, "alerta": alert, "notas": notas });
    let resp: ApiResponse<Value> = authed_post(&format!("{}/patients/{}/scales/news2", API_BASE, patient_id))
        .json(&body).map_err(|e| e.to_string())?
        .send().await.map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn calc_sofa(patient_id: &str, pao2: f32, fio2: f32, plq: f32, bili: f32, pam: f32, vasopresores: bool, dosis: f32, gcs: u8, creat: f32, diuresis: u32, notas: Option<String>) -> ApiResult<Value> {
    let body = serde_json::json!({ "pao2": pao2, "fio2": fio2, "plaquetas": plq, "bilirrubina": bili, "presion_arterial_media": pam, "vasopresores": vasopresores, "dosis_vasopresor": dosis, "gcs_total": gcs, "creatinina": creat, "diuresis_diaria": diuresis, "notas": notas });
    let resp: ApiResponse<Value> = authed_post(&format!("{}/patients/{}/scales/sofa", API_BASE, patient_id))
        .json(&body).map_err(|e| e.to_string())?
        .send().await.map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn calc_saps3(patient_id: &str, edad: u8, dias: u8, tipo: Option<String>, fuente: Option<String>, notas: Option<String>) -> ApiResult<Value> {
    let body = serde_json::json!({ "edad": edad, "dias_pre_uci": dias, "tipo_admision": tipo, "fuente_admision": fuente, "presion_sistolica": 120.0, "frecuencia_cardiaca": 80.0, "gcs_total": 15, "bilirrubina": 0.8, "creatinina": 1.0, "plaquetas": 250.0, "ph_arterial": 7.4, "ventilacion_mecanica": false, "vasopresores": false, "notas": notas });
    let resp: ApiResponse<Value> = authed_post(&format!("{}/patients/{}/scales/saps3", API_BASE, patient_id))
        .json(&body).map_err(|e| e.to_string())?
        .send().await.map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn get_scales_history(patient_id: &str) -> ApiResult<Vec<Value>> {
    let resp: ApiResponse<Vec<Value>> = authed_get(&format!("{}/patients/{}/scales/history", API_BASE, patient_id))
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

// ─── Admin ──────────────────────────────────────────────────────────

pub async fn get_admin_stats() -> ApiResult<AdminStats> {
    let resp: ApiResponse<AdminStats> =
        authed_get(&format!("{}/admin/stats", API_BASE)).send().await
            .map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn init_camas(cantidad: u8) -> ApiResult<Vec<Cama>> {
    #[derive(serde::Serialize)]
    struct Req { cantidad: u8 }
    let resp: ApiResponse<Vec<Cama>> =
        authed_post(&format!("{}/admin/camas/init", API_BASE))
            .json(&Req { cantidad }).map_err(|e| e.to_string())?
            .send().await.map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn list_camas() -> ApiResult<Vec<Cama>> {
    let resp: ApiResponse<PaginatedResponse<Cama>> =
        authed_get(&format!("{}/admin/camas?limit=200", API_BASE)).send().await
            .map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;
    Ok(resp.data.map(|p| p.items).unwrap_or_default())
}

pub async fn get<T: for<'de> serde::Deserialize<'de>>(path: &str) -> ApiResult<T> {
    let resp: ApiResponse<T> =
        authed_get(&format!("{}{}", API_BASE, path)).send().await
            .map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct InitCamasRequest {
    pub cantidad: u8,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct InitResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CreateCamaRequest {
    pub numero: u8,
    pub estado: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct UpdateCamaRequest {
    pub numero: Option<u8>,
    pub estado: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CreateEquipoRequest {
    pub nombre: String,
    pub tipo: String,
    pub modelo: String,
    pub serie: String,
    pub estado: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct UpdateEquipoRequest {
    pub nombre: Option<String>,
    pub tipo: Option<String>,
    pub modelo: Option<String>,
    pub serie: Option<String>,
    pub estado: Option<String>,
}

pub async fn post<T: serde::Serialize, R: for<'de> serde::Deserialize<'de>>(path: &str, body: T) -> ApiResult<R> {
    let resp: ApiResponse<R> =
        authed_post(&format!("{}{}", API_BASE, path))
            .json(&body).map_err(|e| e.to_string())?
            .send().await.map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn put<T: serde::Serialize, R: for<'de> serde::Deserialize<'de>>(path: &str, body: T) -> ApiResult<R> {
    let resp: ApiResponse<R> =
        authed_put(&format!("{}{}", API_BASE, path))
            .json(&body).map_err(|e| e.to_string())?
            .send().await.map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn delete_cama(id: &str) -> ApiResult<Cama> {
    let resp: ApiResponse<Cama> =
        authed_delete(&format!("{}/admin/camas/{}", API_BASE, id)).send().await
            .map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn create_equipo(req: CreateEquipoRequest) -> ApiResult<Equipo> {
    post("/admin/equipos", req).await
}

pub async fn update_equipo(id: &str, req: UpdateEquipoRequest) -> ApiResult<Equipo> {
    put(&format!("/admin/equipos/{}", id), req).await
}

pub async fn delete_equipo(id: &str) -> ApiResult<Equipo> {
    let resp: ApiResponse<Equipo> =
        authed_delete(&format!("{}/admin/equipos/{}", API_BASE, id)).send().await
            .map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn get_equipos_disponibles() -> ApiResult<Vec<Equipo>> {
    let resp: ApiResponse<Vec<Equipo>> =
        authed_get(&format!("{}/admin/equipos/disponibles", API_BASE)).send().await
            .map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

// ─── Staff CRUD ─────────────────────────────────────────────────────

pub async fn list_staff() -> ApiResult<Vec<User>> {
    let resp: ApiResponse<PaginatedResponse<User>> =
        authed_get(&format!("{}/admin/staff?limit=200", API_BASE)).send().await
            .map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;
    Ok(resp.data.map(|p| p.items).unwrap_or_default())
}

pub async fn get_staff(id: &str) -> ApiResult<User> {
    let resp: ApiResponse<User> =
        authed_get(&format!("{}/admin/staff/{}", API_BASE, id)).send().await
            .map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn create_staff(user: &User) -> ApiResult<User> {
    let resp: ApiResponse<User> =
        authed_post(&format!("{}/admin/staff", API_BASE))
            .json(user).map_err(|e| e.to_string())?
            .send().await.map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn update_staff(id: &str, user: &User) -> ApiResult<User> {
    let resp: ApiResponse<User> =
        authed_put(&format!("{}/admin/staff/{}", API_BASE, id))
            .json(user).map_err(|e| e.to_string())?
            .send().await.map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn delete_staff(id: &str) -> ApiResult<()> {
    authed_delete(&format!("{}/admin/staff/{}", API_BASE, id))
        .send().await.map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn toggle_staff(id: &str) -> ApiResult<User> {
    let resp: ApiResponse<User> =
        authed_post(&format!("{}/admin/staff/{}/toggle", API_BASE, id))
            .send().await.map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

// ─── Cama CRUD with tipo ────────────────────────────────────────────

pub async fn create_cama(numero: u8, tipo: &str, estado: &str) -> ApiResult<Cama> {
    #[derive(serde::Serialize)]
    struct Req { numero: u8, tipo: String, estado: String }
    let resp: ApiResponse<Cama> =
        authed_post(&format!("{}/admin/camas", API_BASE))
            .json(&Req { numero, tipo: tipo.to_string(), estado: estado.to_string() })
            .map_err(|e| e.to_string())?
            .send().await.map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn update_cama(id: &str, numero: u8, tipo: &str, estado: &str) -> ApiResult<Cama> {
    #[derive(serde::Serialize)]
    struct Req { numero: u8, tipo: String, estado: String }
    let resp: ApiResponse<Cama> =
        authed_put(&format!("{}/admin/camas/{}", API_BASE, id))
            .json(&Req { numero, tipo: tipo.to_string(), estado: estado.to_string() })
            .map_err(|e| e.to_string())?
            .send().await.map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn create_patient_with_equipos(patient: &Patient, equipos_ids: Vec<String>) -> ApiResult<Patient> {
    #[derive(serde::Serialize)]
    struct Req { #[serde(flatten)] patient: Patient, equipos_ids: Vec<String> }
    let resp: ApiResponse<Patient> =
        authed_post(&format!("{}/patients", API_BASE))
            .json(&Req { patient: patient.clone(), equipos_ids })
            .map_err(|e| e.to_string())?
            .send().await.map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn egreso_paciente(id: &str) -> ApiResult<String> {
    let resp: ApiResponse<String> =
        authed_post(&format!("{}/patients/{}/egreso", API_BASE, id))
            .send().await.map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

// ─── Institucion Config ─────────────────────────────────────────────

pub async fn get_institucion_config() -> ApiResult<InstitucionConfig> {
    let resp: ApiResponse<InstitucionConfig> =
        authed_get(&format!("{}/admin/institucion", API_BASE))
            .send().await.map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn update_institucion_config(config: &InstitucionConfig) -> ApiResult<InstitucionConfig> {
    let resp: ApiResponse<InstitucionConfig> =
        authed_put(&format!("{}/admin/institucion", API_BASE))
            .json(config).map_err(|e| e.to_string())?
            .send().await.map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

// ─── Diagnosticos CIE-10 ───────────────────────────────────────────

pub async fn search_diagnosticos(query: &str) -> ApiResult<Vec<Diagnostico>> {
    let resp: ApiResponse<Vec<Diagnostico>> =
        authed_get(&format!("{}/diagnosticos/search?q={}", API_BASE, query))
            .send().await.map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

// ─── Sandbox ────────────────────────────────────────────────────────

pub async fn generate_sandbox_data(cantidad: u32, mediciones: u32) -> ApiResult<String> {
    #[derive(serde::Serialize)]
    struct Req { cantidad_pacientes: u32, mediciones_por_paciente: u32 }
    let resp: ApiResponse<String> =
        authed_post(&format!("{}/sandbox/generate", API_BASE))
            .json(&Req { cantidad_pacientes: cantidad, mediciones_por_paciente: mediciones })
            .map_err(|e| e.to_string())?
            .send().await.map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn clear_sandbox_data() -> ApiResult<String> {
    let resp: ApiResponse<String> =
        authed_post(&format!("{}/sandbox/clear", API_BASE))
            .send().await.map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}
