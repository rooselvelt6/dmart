/// HTTP client — communicates with the Axum backend API
use dmart_shared::models::*;
use gloo_net::http::Request;
use gloo_storage::{LocalStorage, Storage};
use serde::Deserialize;
use serde_json::Value;
use web_sys::RequestCredentials;

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
    let resp: ApiResponse<LoginResponse> = authed_post(&format!("{}/auth/login", API_BASE))
        .json(&body)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

/// Identidad del usuario autenticado (rol, nombre, ...).
pub async fn me() -> ApiResult<UserInfo> {
    let resp: ApiResponse<UserInfo> = authed_get(&format!("{}/auth/me", API_BASE))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

/// Renueva el access token usando la cookie httpOnly del refresh token.
pub async fn refresh_session() -> ApiResult<LoginResponse> {
    let resp: ApiResponse<LoginResponse> = Request::post(&format!("{}/auth/refresh", API_BASE))
        .credentials(RequestCredentials::Include)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

/// Cierre de sesión real: revoca tokens en servidor y limpia la cookie.
pub async fn logout() -> ApiResult<()> {
    let _: ApiResponse<()> = authed_post(&format!("{}/auth/logout", API_BASE))
        .credentials(RequestCredentials::Include)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Cambio de contraseña autenticado (verifica la actual y cierra sesiones).
pub async fn change_password(current: &str, new: &str) -> ApiResult<()> {
    let body = serde_json::json!({
        "current_password": current,
        "new_password": new,
    });
    let resp: ApiResponse<()> = authed_post(&format!("{}/auth/change-password", API_BASE))
        .json(&body)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    ensure_success(resp)
}

/// Verifica el flag `success` en respuestas sin payload (`()` serializa como
/// `null` y no puede distinguirse de `None`).
fn ensure_success<T>(resp: ApiResponse<T>) -> ApiResult<()> {
    if resp.success {
        Ok(())
    } else {
        Err(resp.error.unwrap_or_default())
    }
}

// ─── MFA (segundo factor) ───────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct MfaSetupInfo {
    pub secret: String,
    pub otpauth_uri: String,
    pub backup_codes: Vec<String>,
}

/// Indica si el usuario autenticado tiene MFA habilitado.
pub async fn mfa_status() -> ApiResult<bool> {
    let resp: ApiResponse<MfaStatus> = authed_get(&format!("{}/auth/mfa/status", API_BASE))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    resp.data
        .map(|s| s.enabled)
        .ok_or_else(|| resp.error.unwrap_or_default())
}

/// Inicia la activación de MFA: devuelve el secreto, la URI `otpauth` y los
/// códigos de respaldo (aún no queda habilitado hasta confirmar con un código).
pub async fn mfa_setup() -> ApiResult<MfaSetupInfo> {
    let resp: ApiResponse<MfaSetupInfo> = authed_post(&format!("{}/auth/mfa/setup", API_BASE))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

/// Confirma la activación de MFA con el primer código TOTP.
pub async fn mfa_confirm(code: &str) -> ApiResult<()> {
    let body = serde_json::json!({ "code": code });
    let resp: ApiResponse<()> = authed_post(&format!("{}/auth/mfa/confirm", API_BASE))
        .json(&body)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    ensure_success(resp)
}

/// Desactiva MFA verificando el código actual.
pub async fn mfa_disable(code: &str) -> ApiResult<()> {
    let body = serde_json::json!({ "code": code });
    let resp: ApiResponse<()> = authed_post(&format!("{}/auth/mfa/disable", API_BASE))
        .json(&body)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    ensure_success(resp)
}

// ─── Web Push (SPEC-052) ────────────────────────────────────────────────────

/// Clave pública VAPID (base64url) para suscribirse desde el navegador.
pub async fn push_public_key() -> ApiResult<String> {
    let resp: ApiResponse<Value> = authed_get(&format!("{}/push/vapid", API_BASE))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    resp.data
        .and_then(|d| d.get("public_key").and_then(|k| k.as_str()).map(String::from))
        .ok_or_else(|| resp.error.unwrap_or_default())
}

/// Registra la suscripción del navegador en el backend.
pub async fn push_subscribe(endpoint: &str, p256dh: &str, auth: &str) -> ApiResult<()> {
    let body = serde_json::json!({
        "endpoint": endpoint,
        "p256dh": p256dh,
        "auth": auth,
    });
    let resp: ApiResponse<()> = authed_post(&format!("{}/push/subscribe", API_BASE))
        .json(&body)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    ensure_success(resp)
}

/// Da de baja la suscripción del navegador en el backend.
pub async fn push_unsubscribe(endpoint: &str) -> ApiResult<()> {
    let body = serde_json::json!({ "endpoint": endpoint });
    let resp: ApiResponse<()> = authed_delete(&format!("{}/push/unsubscribe", API_BASE))
        .json(&body)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    ensure_success(resp)
}

/// Completa el login MFA enviando el token de reto obtenido en `login` junto
/// con el código TOTP o un código de respaldo. Devuelve la sesión completa.
pub async fn mfa_verify(
    challenge_token: &str,
    code: &str,
    backup_code: Option<&str>,
) -> ApiResult<LoginResponse> {
    let body = serde_json::json!({ "code": code, "backup_code": backup_code });
    let resp: ApiResponse<LoginResponse> = Request::post(&format!("{}/auth/mfa/verify", API_BASE))
        .header("Authorization", &format!("Bearer {}", challenge_token))
        .credentials(RequestCredentials::Include)
        .json(&body)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

// ─── Patients ──────────────────────────────────────────────────────────────

pub async fn list_patients(
    query: Option<&str>,
    estado: Option<&str>,
) -> ApiResult<Vec<PatientListItem>> {
    let mut url = format!("{}/patients", API_BASE);
    let mut sep = '?';
    if let Some(q) = query.filter(|q| !q.is_empty()) {
        url.push_str(&format!("{sep}q={q}"));
        sep = '&';
    }
    if let Some(e) = estado.filter(|e| !e.is_empty()) {
        url.push_str(&format!("{sep}estado={e}"));
    }
    let resp: ApiResponse<PaginatedResponse<PatientListItem>> = authed_get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    Ok(resp.data.map(|p| p.items).unwrap_or_default())
}

#[derive(Debug, Clone, Deserialize)]
pub struct EjecutivoKpi {
    pub egresados: u64,
    pub fallecidos: u64,
    pub mortalidad_real_pct: f64,
    pub mortalidad_predicha_pct: f64,
    pub los_dias_promedio: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UciStatsResponse {
    pub ejecutivo: EjecutivoKpi,
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
    let resp = authed_get(&url)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    let status = resp.status();
    if !(200..300).contains(&status) {
        return Err(format!("API error: status {}", status));
    }

    let resp: ApiResponse<UciStatsResponse> = resp
        .json()
        .await
        .map_err(|e| format!("JSON parse error: {}", e))?;

    resp.data
        .ok_or_else(|| resp.error.unwrap_or_else(|| "No data".to_string()))
}

pub async fn get_patient(id: &str) -> ApiResult<Patient> {
    let resp: ApiResponse<Patient> = authed_get(&format!("{}/patients/{}", API_BASE, id))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn create_patient(patient: &Patient) -> ApiResult<Patient> {
    let resp: ApiResponse<Patient> = authed_post(&format!("{}/patients", API_BASE))
        .json(patient)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn update_patient(id: &str, patient: &Patient) -> ApiResult<Patient> {
    let resp: ApiResponse<Patient> = authed_put(&format!("{}/patients/{}", API_BASE, id))
        .json(patient)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn delete_patient(id: &str) -> ApiResult<()> {
    authed_delete(&format!("{}/patients/{}", API_BASE, id))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ─── Measurements ──────────────────────────────────────────────────────────

pub async fn get_measurements(patient_id: &str) -> ApiResult<Vec<Measurement>> {
    let resp: ApiResponse<Vec<Measurement>> = authed_get(&format!(
        "{}/patients/{}/measurements",
        API_BASE, patient_id
    ))
    .send()
    .await
    .map_err(|e| e.to_string())?
    .json()
    .await
    .map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn get_last_measurement(patient_id: &str) -> ApiResult<Option<Measurement>> {
    let resp: ApiResponse<Option<Measurement>> = authed_get(&format!(
        "{}/patients/{}/measurements/last",
        API_BASE, patient_id
    ))
    .send()
    .await
    .map_err(|e| e.to_string())?
    .json()
    .await
    .map_err(|e| e.to_string())?;
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
    let resp: ApiResponse<Measurement> = authed_post(&format!(
        "{}/patients/{}/measurements",
        API_BASE, patient_id
    ))
    .json(&body)
    .map_err(|e| e.to_string())?
    .send()
    .await
    .map_err(|e| e.to_string())?
    .json()
    .await
    .map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

// ─── Scales ─────────────────────────────────────────────────────────────────

pub async fn calc_apache(
    patient_id: &str,
    data: ApacheIIData,
    notas: Option<String>,
) -> ApiResult<Value> {
    let body = serde_json::json!({ "data": data, "notas": notas });
    let resp: ApiResponse<Value> = authed_post(&format!(
        "{}/patients/{}/scales/apache",
        API_BASE, patient_id
    ))
    .json(&body)
    .map_err(|e| e.to_string())?
    .send()
    .await
    .map_err(|e| e.to_string())?
    .json()
    .await
    .map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn calc_gcs(
    patient_id: &str,
    apertura: u8,
    verbal: u8,
    motora: u8,
    notas: Option<String>,
) -> ApiResult<Value> {
    let body = serde_json::json!({ "apertura_ocular": apertura, "respuesta_verbal": verbal, "respuesta_motora": motora, "notas": notas });
    let resp: ApiResponse<Value> =
        authed_post(&format!("{}/patients/{}/scales/gcs", API_BASE, patient_id))
            .json(&body)
            .map_err(|e| e.to_string())?
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

#[allow(clippy::too_many_arguments)]
pub async fn calc_news2(
    patient_id: &str,
    fr: f32,
    spo2: f32,
    o2: bool,
    pas: f32,
    fc: f32,
    temp: f32,
    alert: bool,
    notas: Option<String>,
) -> ApiResult<Value> {
    let body = serde_json::json!({ "frecuencia_respiratoria": fr, "spo2": spo2, "o2_suplementario": o2, "presion_sistolica": pas, "frecuencia_cardiaca": fc, "temperatura": temp, "alerta": alert, "notas": notas });
    let resp: ApiResponse<Value> = authed_post(&format!(
        "{}/patients/{}/scales/news2",
        API_BASE, patient_id
    ))
    .json(&body)
    .map_err(|e| e.to_string())?
    .send()
    .await
    .map_err(|e| e.to_string())?
    .json()
    .await
    .map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

#[allow(clippy::too_many_arguments)]
pub async fn calc_sofa(
    patient_id: &str,
    pao2: f32,
    fio2: f32,
    plq: f32,
    bili: f32,
    pam: f32,
    vasopresores: bool,
    dosis: f32,
    gcs: u8,
    creat: f32,
    diuresis: u32,
    notas: Option<String>,
) -> ApiResult<Value> {
    let body = serde_json::json!({ "pao2": pao2, "fio2": fio2, "plaquetas": plq, "bilirrubina": bili, "presion_arterial_media": pam, "vasopresores": vasopresores, "dosis_vasopresor": dosis, "gcs_total": gcs, "creatinina": creat, "diuresis_diaria": diuresis, "notas": notas });
    let resp: ApiResponse<Value> =
        authed_post(&format!("{}/patients/{}/scales/sofa", API_BASE, patient_id))
            .json(&body)
            .map_err(|e| e.to_string())?
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn calc_saps3(
    patient_id: &str,
    edad: u8,
    dias: u8,
    tipo: Option<String>,
    fuente: Option<String>,
    notas: Option<String>,
) -> ApiResult<Value> {
    let body = serde_json::json!({ "edad": edad, "dias_pre_uci": dias, "tipo_admision": tipo, "fuente_admision": fuente, "presion_sistolica": 120.0, "frecuencia_cardiaca": 80.0, "gcs_total": 15, "bilirrubina": 0.8, "creatinina": 1.0, "plaquetas": 250.0, "ph_arterial": 7.4, "ventilacion_mecanica": false, "vasopresores": false, "notas": notas });
    let resp: ApiResponse<Value> = authed_post(&format!(
        "{}/patients/{}/scales/saps3",
        API_BASE, patient_id
    ))
    .json(&body)
    .map_err(|e| e.to_string())?
    .send()
    .await
    .map_err(|e| e.to_string())?
    .json()
    .await
    .map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn get_scales_history(patient_id: &str) -> ApiResult<Vec<Value>> {
    let resp: ApiResponse<Vec<Value>> = authed_get(&format!(
        "{}/patients/{}/scales/history",
        API_BASE, patient_id
    ))
    .send()
    .await
    .map_err(|e| e.to_string())?
    .json()
    .await
    .map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

// ─── Export helpers ─────────────────────────────────────────────────────────

pub fn export_csv_url(patient_id: &str) -> String {
    let mut url = format!("{}/patients/{}/export/csv", API_BASE, patient_id);
    if let Ok(token) = LocalStorage::get::<String>("dmart_auth") {
        url = format!("{}?token={}", url, token);
    }
    url
}

pub fn export_pdf_url(patient_id: &str) -> String {
    let mut url = format!("{}/patients/{}/export/pdf", API_BASE, patient_id);
    if let Ok(token) = LocalStorage::get::<String>("dmart_auth") {
        url = format!("{}?token={}", url, token);
    }
    url
}

// ─── Admin ──────────────────────────────────────────────────────────

pub async fn get_admin_stats() -> ApiResult<AdminStats> {
    let resp: ApiResponse<AdminStats> = authed_get(&format!("{}/admin/stats", API_BASE))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

// ─── Auditoría HIPAA ────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct AuditLog {
    pub uid: String,
    pub timestamp: String,
    pub user_id: Option<String>,
    pub username: Option<String>,
    pub action: String,
    pub resource: String,
    pub resource_id: Option<String>,
    pub details: Option<String>,
    pub ip_address: Option<String>,
    pub success: bool,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IntegrityReport {
    pub logs_total: usize,
    pub logs_hashed: usize,
    pub logs_valid: usize,
    pub logs_unhashed: usize,
    pub batches_total: usize,
    pub signatures_valid: usize,
    pub chain_valid: bool,
    pub head_batch_hash: String,
    pub sealable_logs: usize,
    pub ok: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuditBatch {
    pub batch_id: String,
    pub sequence: u64,
    pub first_uid: String,
    pub last_uid: String,
    pub count: u64,
    pub first_ts: String,
    pub last_ts: String,
    pub batch_hash: String,
    pub signature: String,
    pub created_at: String,
}

pub async fn get_audit_logs(limit: usize, action: Option<&str>) -> ApiResult<Vec<AuditLog>> {
    let mut url = format!("{}/admin/audit?limit={}", API_BASE, limit);
    if let Some(a) = action.filter(|a| !a.is_empty()) {
        url.push_str(&format!("&action={a}"));
    }
    let resp: ApiResponse<Vec<AuditLog>> = authed_get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn verify_audit_chain() -> ApiResult<IntegrityReport> {
    let resp: ApiResponse<IntegrityReport> = authed_post(&format!("{}/admin/audit/verify", API_BASE))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn seal_audit_batch() -> ApiResult<Option<AuditBatch>> {
    let resp: ApiResponse<Option<AuditBatch>> =
        authed_post(&format!("{}/admin/audit/seal", API_BASE))
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn init_camas(cantidad: u8) -> ApiResult<Vec<Cama>> {
    #[derive(serde::Serialize)]
    struct Req {
        cantidad: u8,
    }
    let resp: ApiResponse<Vec<Cama>> = authed_post(&format!("{}/admin/camas/init", API_BASE))
        .json(&Req { cantidad })
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn list_camas() -> ApiResult<Vec<Cama>> {
    let resp: ApiResponse<PaginatedResponse<Cama>> =
        authed_get(&format!("{}/admin/camas?limit=200", API_BASE))
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;
    Ok(resp.data.map(|p| p.items).unwrap_or_default())
}

pub async fn get<T: for<'de> serde::Deserialize<'de>>(path: &str) -> ApiResult<T> {
    let resp: ApiResponse<T> = authed_get(&format!("{}{}", API_BASE, path))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
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

pub async fn post<T: serde::Serialize, R: for<'de> serde::Deserialize<'de>>(
    path: &str,
    body: T,
) -> ApiResult<R> {
    let resp: ApiResponse<R> = authed_post(&format!("{}{}", API_BASE, path))
        .json(&body)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn put<T: serde::Serialize, R: for<'de> serde::Deserialize<'de>>(
    path: &str,
    body: T,
) -> ApiResult<R> {
    let resp: ApiResponse<R> = authed_put(&format!("{}{}", API_BASE, path))
        .json(&body)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn delete_cama(id: &str) -> ApiResult<Cama> {
    let resp: ApiResponse<Cama> = authed_delete(&format!("{}/admin/camas/{}", API_BASE, id))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn create_equipo(req: CreateEquipoRequest) -> ApiResult<Equipo> {
    post("/admin/equipos", req).await
}

pub async fn update_equipo(id: &str, req: UpdateEquipoRequest) -> ApiResult<Equipo> {
    put(&format!("/admin/equipos/{}", id), req).await
}

pub async fn delete_equipo(id: &str) -> ApiResult<Equipo> {
    let resp: ApiResponse<Equipo> = authed_delete(&format!("{}/admin/equipos/{}", API_BASE, id))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn get_equipos_disponibles() -> ApiResult<Vec<Equipo>> {
    let resp: ApiResponse<Vec<Equipo>> =
        authed_get(&format!("{}/admin/equipos/disponibles", API_BASE))
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

// ─── Staff CRUD ─────────────────────────────────────────────────────

pub async fn list_staff() -> ApiResult<Vec<StaffInfo>> {
    let resp: ApiResponse<PaginatedResponse<StaffInfo>> =
        authed_get(&format!("{}/admin/staff?limit=200", API_BASE))
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;
    Ok(resp.data.map(|p| p.items).unwrap_or_default())
}

pub async fn get_staff(id: &str) -> ApiResult<StaffInfo> {
    let resp: ApiResponse<StaffInfo> = authed_get(&format!("{}/admin/staff/{}", API_BASE, id))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn create_staff(
    username: &str,
    nombre: &str,
    rol: &str,
    password: &str,
) -> ApiResult<StaffInfo> {
    let resp: ApiResponse<StaffInfo> = authed_post(&format!("{}/admin/staff", API_BASE))
        .json(&serde_json::json!({
            "username": username,
            "nombre": nombre,
            "rol": rol,
            "password": password,
        }))
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn update_staff(
    id: &str,
    nombre: &str,
    rol: &str,
    password: &str,
) -> ApiResult<StaffInfo> {
    let mut body = serde_json::json!({
        "nombre": nombre,
        "rol": rol,
    });
    if !password.is_empty() {
        body["password"] = serde_json::json!(password);
    }
    let resp: ApiResponse<StaffInfo> = authed_put(&format!("{}/admin/staff/{}", API_BASE, id))
        .json(&body)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn delete_staff(id: &str) -> ApiResult<()> {
    authed_delete(&format!("{}/admin/staff/{}", API_BASE, id))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn toggle_staff(id: &str) -> ApiResult<StaffInfo> {
    let resp: ApiResponse<StaffInfo> =
        authed_post(&format!("{}/admin/staff/{}/toggle", API_BASE, id))
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

// ─── Cama CRUD with tipo ────────────────────────────────────────────

pub async fn create_cama(numero: u8, tipo: &str, estado: &str) -> ApiResult<Cama> {
    #[derive(serde::Serialize)]
    struct Req {
        numero: u8,
        tipo: String,
        estado: String,
    }
    let resp: ApiResponse<Cama> = authed_post(&format!("{}/admin/camas", API_BASE))
        .json(&Req {
            numero,
            tipo: tipo.to_string(),
            estado: estado.to_string(),
        })
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn update_cama(id: &str, numero: u8, tipo: &str, estado: &str) -> ApiResult<Cama> {
    #[derive(serde::Serialize)]
    struct Req {
        numero: u8,
        tipo: String,
        estado: String,
    }
    let resp: ApiResponse<Cama> = authed_put(&format!("{}/admin/camas/{}", API_BASE, id))
        .json(&Req {
            numero,
            tipo: tipo.to_string(),
            estado: estado.to_string(),
        })
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn create_patient_with_equipos(
    patient: &Patient,
    equipos_ids: Vec<String>,
) -> ApiResult<Patient> {
    #[derive(serde::Serialize)]
    struct Req {
        #[serde(flatten)]
        patient: Patient,
        equipos_ids: Vec<String>,
    }
    let resp: ApiResponse<Patient> = authed_post(&format!("{}/patients", API_BASE))
        .json(&Req {
            patient: patient.clone(),
            equipos_ids,
        })
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn egreso_paciente(id: &str, desenlace: &str) -> ApiResult<String> {
    let resp: ApiResponse<String> = authed_post(&format!(
        "{}/patients/{}/egreso?desenlace={}",
        API_BASE, id, desenlace
    ))
    .send()
    .await
    .map_err(|e| e.to_string())?
    .json()
    .await
    .map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

// ─── Institucion Config ─────────────────────────────────────────────

pub async fn get_institucion_config() -> ApiResult<InstitucionConfig> {
    let resp: ApiResponse<InstitucionConfig> =
        authed_get(&format!("{}/admin/institucion", API_BASE))
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn update_institucion_config(config: &InstitucionConfig) -> ApiResult<InstitucionConfig> {
    let resp: ApiResponse<InstitucionConfig> =
        authed_put(&format!("{}/admin/institucion", API_BASE))
            .json(config)
            .map_err(|e| e.to_string())?
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

// ─── Diagnosticos CIE-10 ───────────────────────────────────────────

pub async fn search_diagnosticos(query: &str) -> ApiResult<Vec<Diagnostico>> {
    let resp: ApiResponse<Vec<Diagnostico>> =
        authed_get(&format!("{}/diagnosticos/search?q={}", API_BASE, query))
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

// ─── Sandbox ────────────────────────────────────────────────────────

pub async fn generate_sandbox_data(cantidad: u32, mediciones: u32) -> ApiResult<String> {
    #[derive(serde::Serialize)]
    struct Req {
        cantidad_pacientes: u32,
        mediciones_por_paciente: u32,
    }
    let resp: ApiResponse<String> = authed_post(&format!("{}/sandbox/generate", API_BASE))
        .json(&Req {
            cantidad_pacientes: cantidad,
            mediciones_por_paciente: mediciones,
        })
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

pub async fn clear_sandbox_data() -> ApiResult<String> {
    let resp: ApiResponse<String> = authed_post(&format!("{}/sandbox/clear", API_BASE))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    resp.data.ok_or_else(|| resp.error.unwrap_or_default())
}

// ─── Consola Técnica de Soporte (SPEC-044) ─────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct SupportSystem {
    pub key: String,
    pub name: String,
    pub status: String,
    pub latency_ms: Option<f64>,
    pub error_count: u64,
    pub ok_count: u64,
    pub freshness_secs: Option<f64>,
    pub details: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SliIndicator {
    pub key: String,
    pub label: String,
    pub value: String,
    pub target: String,
    pub ok: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Diagnostic {
    pub system: String,
    pub indicators: Vec<SliIndicator>,
    pub suggested_actions: Vec<String>,
    pub notes: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SupportEvent {
    pub uid: String,
    pub timestamp: String,
    pub origin: String,
    pub subsystem: String,
    pub action: String,
    pub username: Option<String>,
    pub ip_address: Option<String>,
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ActionResponse {
    pub action: String,
    pub success: bool,
    pub message: String,
    pub event: Option<SupportEvent>,
}

pub async fn get_support_systems() -> ApiResult<Vec<SupportSystem>> {
    get("/admin/support/systems").await
}

pub async fn get_support_diagnostics() -> ApiResult<Vec<Diagnostic>> {
    get("/admin/support/diagnostics").await
}

pub async fn get_support_history(limit: usize) -> ApiResult<Vec<SupportEvent>> {
    get(&format!("/admin/support/history?limit={}", limit)).await
}

/// Ejecuta una acción del runbook. `model`/`version` solo aplican a `model_swap`
/// (vacíos usan el modelo activo/por defecto).
pub async fn run_support_action(
    action: &str,
    model: &str,
    version: &str,
) -> ApiResult<ActionResponse> {
    #[derive(serde::Serialize)]
    struct Req<'a> {
        model: &'a str,
        version: &'a str,
    }
    post(
        &format!("/admin/support/actions/{}", action),
        Req { model, version },
    )
    .await
}

// ─── Fase 3: Dispositivos (SPEC-017) ───────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct DeviceStateCount {
    pub estado: String,
    pub total: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeviceTypeCount {
    pub device_type: String,
    pub total: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeviceStatusSummary {
    pub total: u64,
    pub por_estado: Vec<DeviceStateCount>,
    pub por_tipo: Vec<DeviceTypeCount>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClinicalDevice {
    pub id: String,
    pub device_type: String,
    pub fabricante: String,
    pub modelo: String,
    pub firmware: String,
    pub serial: String,
    pub cama_id: Option<String>,
    pub ubicacion: Option<String>,
    pub estado: String,
    pub registered_at: i64,
    pub last_seen_at: Option<i64>,
    pub heartbeat_interval_secs: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RegisterDeviceRequest {
    pub device_type: String,
    pub fabricante: String,
    pub modelo: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub firmware: String,
    pub serial: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cama_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ubicacion: Option<String>,
    pub estado: String,
}

pub async fn list_devices(estado: Option<String>) -> ApiResult<Vec<ClinicalDevice>> {
    match estado.filter(|e| !e.is_empty()) {
        Some(e) => get(&format!("/devices?estado={}", e)).await,
        None => get("/devices").await,
    }
}

pub async fn get_device_status() -> ApiResult<DeviceStatusSummary> {
    get("/devices/status").await
}

pub async fn register_device(req: RegisterDeviceRequest) -> ApiResult<ClinicalDevice> {
    post("/devices", req).await
}

pub async fn heartbeat_device(id: &str) -> ApiResult<ClinicalDevice> {
    post(&format!("/devices/{}/heartbeat", id), serde_json::json!({})).await
}

// ─── Fase 3: Calidad de datos (SPEC-018) ───────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct QualityIssue {
    pub id: String,
    pub message_id: String,
    pub patient_ref: String,
    pub severity: String,
    pub code: String,
    pub detail: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QualitySeverityCount {
    pub severity: String,
    pub count: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QualityCodeCount {
    pub code: String,
    pub count: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QualitySummary {
    pub total: u64,
    pub by_severity: Vec<QualitySeverityCount>,
    pub by_code: Vec<QualityCodeCount>,
}

pub async fn get_quality_report() -> ApiResult<Vec<QualityIssue>> {
    get("/data-quality/report").await
}

pub async fn get_quality_summary() -> ApiResult<QualitySummary> {
    get("/data-quality/summary").await
}

// ─── Fase 3: Escalamiento de alertas (SPEC-019) ────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct EscalationPolicy {
    pub severity: String,
    pub max_response_minutes: u32,
    pub timeout_minutes: u32,
    pub target_role: String,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Escalation {
    pub id: String,
    pub patient_id: String,
    pub alert_type: String,
    pub severity: String,
    pub level: u32,
    pub status: String,
    pub policy_severity: String,
    pub created_at: String,
    pub acknowledged_at: Option<String>,
    pub escalated_at: Option<String>,
    pub resolved_at: Option<String>,
    pub acknowledged_by: Option<String>,
    pub escalated_to: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SetPolicyRequest {
    pub severity: String,
    pub max_response_minutes: Option<u32>,
    pub timeout_minutes: Option<u32>,
    pub target_role: Option<String>,
    pub enabled: Option<bool>,
}

pub async fn list_active_escalations() -> ApiResult<Vec<Escalation>> {
    get("/escalation/active").await
}

pub async fn list_escalation_policies() -> ApiResult<Vec<EscalationPolicy>> {
    get("/escalation/policies").await
}

pub async fn set_escalation_policy(req: SetPolicyRequest) -> ApiResult<EscalationPolicy> {
    post("/escalation/policies", req).await
}

pub async fn ack_escalation(id: &str) -> ApiResult<Escalation> {
    post(&format!("/escalation/{}/ack", id), serde_json::json!({})).await
}

pub async fn escalate_escalation(id: &str) -> ApiResult<Escalation> {
    post(&format!("/escalation/{}/escalate", id), serde_json::json!({})).await
}
