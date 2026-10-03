//! Capa de persistencia cifrada de PHI (SPEC-052 / auditoría H4).
//!
//! Es la única puerta de entrada a la PHI en reposo. El patrón es siempre el
//! mismo y se repite por cada agregado:
//!
//! - **Escritura** (`seal_patient`): se serializa el documento completo, se
//!   cifra con AES-256-GCM y se escribe en una columna `phi` junto con las
//!   columnas en claro necesarias para filtrar y los índices ciegos de los
//!   campos buscables. Los campos identificables **no** se escriben en claro.
//! - **Lectura** (`open_patient`): se abre el envelope con el mismo contexto
//!   (tenant, tipo de registro, id). Si la fila no tiene `phi`, es una fila
//!   anterior a la migración y se devuelve tal cual desde las columnas legacy.
//! - **Búsqueda**: coincidencia **exacta** contra los índices ciegos. Se
//!   conserva además la consulta a las columnas legacy para que las filas no
//!   migradas sigan encontrándose; cuando la tabla esté completamente migrada,
//!   ese segundo término puede retirarse junto con el fallback de lectura.
//!
//! El AAD de AES-GCM incluye `tenant_id`, tipo de registro e id, por lo que
//! copiar un envelope entre registros o tenants produce un fallo de
//! autenticación en vez de devolver PHI ajena.

use std::sync::OnceLock;

use anyhow::{Context, Result};
use dmart_shared::models::{ApacheIIData, GcsData, Measurement, Patient};
use serde_json::Value;
use surrealdb::Surreal;
use surrealdb::engine::local::Db;

use crate::crypto::{CryptoError, PhiCipher, PhiContext};

/// Nombre lógico del agregado, usado como parte del AAD.
const PATIENT_RECORD_TYPE: &str = "patient";
const MEASUREMENT_RECORD_TYPE: &str = "measurement";
const CAMA_RECORD_TYPE: &str = "cama";
/// Nombres de campo en el HMAC del índice ciego.
const F_HISTORIA_CLINICA: &str = "historia_clinica";
const F_CEDULA: &str = "cedula";
const F_NOMBRE: &str = "nombre";

static CIPHER: OnceLock<PhiCipher> = OnceLock::new();

/// Cifrador del proceso.
///
/// Se construye una sola vez para no derivar las subclaves en cada llamada. El
/// material de clave queda en memoria dentro de tipos `Zeroizing`, así que se
/// borra al soltar el proceso.
///
/// En producción, si `DMART_MASTER_KEY` falta, **aborta el proceso** en vez de
/// seguir con una clave efímera: los registros ya cifrados quedarían ilegibles
/// tras el siguiente reinicio, y el fallo aparecería en el peor momento. Aquí no
/// se puede devolver `Result` porque todas las funciones de PHI lo llaman por
/// comodidad, así que se hace explícito con `expect` en el único punto de
/// inicialización, que es exactamente donde el proceso debe morir.
pub fn cipher() -> &'static PhiCipher {
    CIPHER.get_or_init(|| {
        PhiCipher::from_env().unwrap_or_else(|e| {
            tracing::error!("cifrado de PHI no disponible: {e}");
            std::process::exit(78); // EX_CONFIG
        })
    })
}

/// Inicializa el cifrador (para binarios standalone como backfill).
pub fn ensure_cipher_initialized() {
    cipher();
}

/// Contexto de cifrado de un paciente. El `record_id` es el `patient_id` (id
/// público), no el id interno del record de SurrealDB: así el mismo paciente
/// conserva su envelope aunque cambie de clave de partición.
fn patient_ctx(tenant_id: &str, patient_id: &str) -> PhiContext {
    PhiContext::new(tenant_id, PATIENT_RECORD_TYPE, patient_id)
}

fn measurement_ctx(tenant_id: &str, measurement_id: &str) -> PhiContext {
    PhiContext::new(tenant_id, MEASUREMENT_RECORD_TYPE, measurement_id)
}

fn cama_ctx(tenant_id: &str, cama_id: &str) -> PhiContext {
    PhiContext::new(tenant_id, CAMA_RECORD_TYPE, cama_id)
}

/// Nota sobre `SELECT`: todas las lecturas de PHI usan `SELECT * OMIT id`.
///
/// El campo `id` de un record de SurrealDB es un `RecordId`, y deserializarlo
/// dentro de un `serde_json::Value` falla (`invalid type: enum`) porque el
/// `RecordId` se representa como enumeración. Como el AAD y las columnas en
/// claro ya identified el registro por `patient_id`, `id` es redundante y se
/// omite en la proyección en vez de pelearse con la deserialización.
///
/// Fila de `patients` tal como se persiste tras la migración.
///
/// El `phi` **no** es `#[serde(default)]` a propósito: si el campo faltara, el
/// `Patient` se deserializaría con todos sus `#[serde(default)]` en cero y se
/// devolvería un paciente vacío en lugar de fallar. Que sea obligatorio
/// convierte cualquier corrupción en un error visible.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct PatientRow {
    // ── Columnas en claro: identidad técnica y filtros no identificables ────
    #[serde(rename = "patient_id")]
    pub patient_id: String,
    #[serde(rename = "tenant_id")]
    pub tenant_id: String,
    #[serde(rename = "estado_gravedad")]
    pub estado_gravedad: String,
    #[serde(rename = "ultimo_apache_score")]
    pub ultimo_apache_score: Option<u32>,
    #[serde(rename = "ultimo_gcs_score")]
    pub ultimo_gcs_score: Option<u8>,
    #[serde(rename = "ultimo_sofa_score")]
    pub ultimo_sofa_score: Option<u32>,
    #[serde(rename = "ultimo_saps3_score")]
    pub ultimo_saps3_score: Option<u32>,
    #[serde(rename = "ultimo_news2_score")]
    pub ultimo_news2_score: Option<u32>,
    #[serde(rename = "mortality_risk")]
    pub mortality_risk: Option<f32>,
    #[serde(rename = "fecha_ingreso_uci")]
    pub fecha_ingreso_uci: String,
    #[serde(rename = "fecha_egreso_uci")]
    pub fecha_egreso_uci: String,
    #[serde(rename = "desenlace_uci")]
    pub desenlace_uci: String,
    #[serde(rename = "cama_id")]
    pub cama_id: Option<String>,
    #[serde(rename = "cama_numero")]
    pub cama_numero: Option<u8>,
    #[serde(rename = "created_at")]
    pub created_at: String,
    #[serde(rename = "updated_at")]
    pub updated_at: String,

    // ── PHI: envelope AES-256-GCM ───────────────────────────────────────────
    pub phi: String,

    // ── Índices ciegos HMAC-SHA256 (búsqueda exacta) ───────────────────────
    #[serde(rename = "bi_hc")]
    pub bi_hc: String,
    #[serde(rename = "bi_ced")]
    pub bi_ced: String,
    #[serde(rename = "bi_nombre")]
    pub bi_nombre: String,
}

/// Traduce un error de criptografía a un error de aplicación.
///
/// Un fallo de descifrado se propaga como error (nunca como `None`), porque
/// `None` significaría "el paciente no existe" y podría ocultar una corrupción
/// o un envelope movido de sitio.
fn crypto_ctx(e: CryptoError) -> anyhow::Error {
    match e {
        CryptoError::DecryptionFailed => anyhow::anyhow!(
            "PHI envelope inválido: fallo de autenticación AES-GCM. \
             La fila fue alterada o el envelope pertenece a otro registro/tenant."
        ),
        other => anyhow::anyhow!("operación criptográfica fallida: {other:?}"),
    }
}

/// PHI de una medición: signos vitales, datos clínicos detallados y las notas
/// libres del operador.
///
/// `notas` es texto clínico libre: puede contener nombre, diagnóstico o
/// incidencias del paciente, así que va **dentro** del envelope (P0.2). Se
/// marca `#[serde(default)]` para que los envelopes ya escritos sin `notas`
/// sigan abriéndose: la re-sellar en la siguiente escritura los completa.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MeasurementPhi {
    pub apache_data: ApacheIIData,
    pub gcs_data: GcsData,
    #[serde(default)]
    pub notas: String,
}

/// Fila de `measurements` tal como se persiste tras la migración PHI.
///
/// Los signos vitales, los datos clínicos y las notas (`apache_data`,
/// `gcs_data`, `notas`) están en el envelope `phi`. Quedan en claro los campos
/// necesarios para filtrar, paginar y agregar: ids, tenant, timestamps, scores
/// calculados y `fingerprint` (SPEC-029, que es un hash: no es PHI).
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct MeasurementRow {
    // ── Columnas en claro: identidad técnica y filtros no identificables ────
    #[serde(rename = "measurement_id")]
    pub measurement_id: String,
    #[serde(rename = "patient_id")]
    pub patient_id: String,
    #[serde(rename = "tenant_id")]
    pub tenant_id: String,
    #[serde(rename = "timestamp")]
    pub timestamp: String,
    #[serde(rename = "apache_score")]
    pub apache_score: u32,
    #[serde(rename = "gcs_score")]
    pub gcs_score: u8,
    #[serde(rename = "severity")]
    pub severity: String,
    #[serde(rename = "mortality_risk")]
    pub mortality_risk: f32,
    #[serde(rename = "saps3_score")]
    pub saps3_score: Option<u32>,
    #[serde(rename = "saps3_mortality")]
    pub saps3_mortality: Option<f32>,
    #[serde(rename = "news2_score")]
    pub news2_score: Option<u32>,
    #[serde(rename = "news2_level")]
    pub news2_level: String,
    #[serde(rename = "sofa_score")]
    pub sofa_score: Option<u32>,
    #[serde(rename = "sofa_mortality")]
    pub sofa_mortality: Option<f32>,
    #[serde(rename = "algorithm_version")]
    pub algorithm_version: String,
    #[serde(rename = "fingerprint")]
    pub fingerprint: String,

    // ── PHI: envelope AES-256-GCM ───────────────────────────────────────────
    pub phi: String,
}

/// Prepara la fila persistible de un paciente: sella la PHI y calcula los
/// índices ciegos bajo el tenant del propio paciente.
///
/// El tenant se toma del paciente (no de un parámetro externo) para que un
/// cliente no pueda cifrar PHI de un tenant ajeno moviendo el registro.
pub fn seal_patient(patient: &Patient) -> Result<PatientRow> {
    let ctx = patient_ctx(&patient.tenant_id, &patient.patient_id);
    let phi = cipher()
        .seal(&ctx, patient)
        .map_err(crypto_ctx)
        .context("cifrando PHI del paciente")?;

    Ok(PatientRow {
        patient_id: patient.patient_id.clone(),
        tenant_id: patient.tenant_id.clone(),
        estado_gravedad: patient.estado_gravedad.to_string(),
        ultimo_apache_score: patient.ultimo_apache_score,
        ultimo_gcs_score: patient.ultimo_gcs_score,
        ultimo_sofa_score: patient.ultimo_sofa_score,
        ultimo_saps3_score: patient.ultimo_saps3_score,
        ultimo_news2_score: patient.ultimo_news2_score,
        mortality_risk: patient.mortality_risk,
        fecha_ingreso_uci: patient.fecha_ingreso_uci.clone(),
        fecha_egreso_uci: patient.fecha_egreso_uci.clone(),
        desenlace_uci: patient.desenlace_uci.clone(),
        cama_id: patient.cama_id.clone(),
        cama_numero: patient.cama_numero,
        created_at: patient.created_at.clone(),
        updated_at: patient.updated_at.clone(),
        phi,
        bi_hc: blind(
            F_HISTORIA_CLINICA,
            &patient.tenant_id,
            &patient.historia_clinica,
        ),
        bi_ced: blind(F_CEDULA, &patient.tenant_id, &patient.cedula),
        bi_nombre: blind(F_NOMBRE, &patient.tenant_id, &patient.nombre),
    })
}

/// Índice ciego con el tenant del paciente.
fn blind(field: &str, tenant_id: &str, value: &str) -> String {
    cipher().blind_index(tenant_id, field, value)
}

/// Índice ciego **formato legacy** (sin longitudes prefijadas) para compatibilidad.
fn blind_legacy(field: &str, tenant_id: &str, value: &str) -> String {
    cipher().blind_index_legacy(tenant_id, field, value)
}

/// Prepara la fila persistible de una medición: sella la PHI
/// (apache_data, gcs_data, notas) en el envelope `phi`.
///
/// El tenant se toma de la medición para que un cliente no pueda cifrar PHI
/// de un tenant ajeno.
pub fn seal_measurement(m: &Measurement) -> Result<MeasurementRow> {
    let phi_payload = MeasurementPhi {
        apache_data: m.apache_data.clone(),
        gcs_data: m.gcs_data.clone(),
        notas: m.notas.clone(),
    };
    let ctx = measurement_ctx(&m.tenant_id, &m.measurement_id);
    let phi = cipher()
        .seal(&ctx, &phi_payload)
        .map_err(crypto_ctx)
        .context("cifrando PHI de la medición")?;

    Ok(MeasurementRow {
        measurement_id: m.measurement_id.clone(),
        patient_id: m.patient_id.clone(),
        tenant_id: m.tenant_id.clone(),
        timestamp: m.timestamp.clone(),
        apache_score: m.apache_score,
        gcs_score: m.gcs_score,
        severity: m.severity.label().to_string(),
        mortality_risk: m.mortality_risk,
        saps3_score: m.saps3_score,
        saps3_mortality: m.saps3_mortality,
        news2_score: m.news2_score,
        news2_level: m.news2_level.label().to_string(),
        sofa_score: m.sofa_score,
        sofa_mortality: m.sofa_mortality,
        algorithm_version: m.algorithm_version.clone(),
        fingerprint: m.fingerprint.clone(),
        phi,
    })
}

/// Abre una fila de `measurements` y devuelve la medición con PHI descifrado.
///
/// Si la fila no tiene `phi` es una fila anterior a la migración: la PHI sigue
/// en las columnas en claro (`apache_data`, `gcs_data`, `notas`), y se leen de
/// ahí. Si la fila tampoco trae esos campos se devuelve el default en vez de
/// fallar: `Measurement` declara `#[serde(default)]` en todo y una fila
/// genuinely corrupta debe verse como tal, no como una medición válida.
pub fn open_measurement(value: Value) -> Result<Measurement> {
    let sealed_phi = value.get("phi").and_then(Value::as_str);
    let tenant_id = value
        .get("tenant_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let measurement_id = value
        .get("measurement_id")
        .and_then(Value::as_str)
        .unwrap_or_default();

    let mut m = Measurement {
        id: None,
        measurement_id: measurement_id.to_string(),
        patient_id: value
            .get("patient_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        timestamp: value
            .get("timestamp")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        apache_data: ApacheIIData::default(),
        gcs_data: GcsData::default(),
        apache_score: value
            .get("apache_score")
            .and_then(Value::as_u64)
            .unwrap_or_default() as u32,
        gcs_score: value
            .get("gcs_score")
            .and_then(Value::as_u64)
            .unwrap_or_default() as u8,
        severity: value
            .get("severity")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default(),
        mortality_risk: value
            .get("mortality_risk")
            .and_then(Value::as_f64)
            .unwrap_or_default() as f32,
        saps3_score: value
            .get("saps3_score")
            .and_then(Value::as_u64)
            .map(|v| v as u32),
        saps3_mortality: value
            .get("saps3_mortality")
            .and_then(Value::as_f64)
            .map(|v| v as f32),
        news2_score: value
            .get("news2_score")
            .and_then(Value::as_u64)
            .map(|v| v as u32),
        news2_level: value
            .get("news2_level")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default(),
        sofa_score: value
            .get("sofa_score")
            .and_then(Value::as_u64)
            .map(|v| v as u32),
        sofa_mortality: value
            .get("sofa_mortality")
            .and_then(Value::as_f64)
            .map(|v| v as f32),
        algorithm_version: value
            .get("algorithm_version")
            .and_then(Value::as_str)
            .unwrap_or("<legacy>")
            .to_string(),
        fingerprint: value
            .get("fingerprint")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        notas: value
            .get("notas")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        tenant_id: tenant_id.to_string(),
    };

    match sealed_phi {
        Some(phi) if !phi.is_empty() => {
            let ctx = measurement_ctx(tenant_id, measurement_id);
            let phi_payload: MeasurementPhi = cipher()
                .open(&ctx, phi)
                .map_err(crypto_ctx)
                .context("descifrando PHI de la medición")?;
            m.apache_data = phi_payload.apache_data;
            m.gcs_data = phi_payload.gcs_data;
            m.notas = phi_payload.notas;
            Ok(m)
        }
        _ => {
            // Fila legacy: la PHI sigue en claro en la propia fila. Sin esto el
            // backfill sellaría un `MeasurementPhi` con los defaults vacíos y
            // destruiría los signos vitales de la fila.
            if let Some(raw) = value.get("apache_data")
                && let Ok(data) = serde_json::from_value::<ApacheIIData>(raw.clone())
            {
                m.apache_data = data;
            }
            if let Some(raw) = value.get("gcs_data")
                && let Ok(data) = serde_json::from_value::<GcsData>(raw.clone())
            {
                m.gcs_data = data;
            }
            Ok(m)
        }
    }
}

/// Abre múltiples filas de mediciones.
pub fn open_measurements(rows: Vec<Value>) -> Result<Vec<Measurement>> {
    rows.into_iter()
        .map(open_measurement)
        .collect::<Result<Vec<_>>>()
}

/// Abre una fila de `measurements` opcional.
pub fn open_measurement_opt(value: Option<Value>) -> Result<Option<Measurement>> {
    match value {
        Some(v) => open_measurement(v).map(Some),
        None => Ok(None),
    }
}

/// PHI de una cama: nombre del paciente (PHI).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CamaPhi {
    pub paciente_nombre: Option<String>,
}

/// Fila de `camas` tal como se persiste tras la migración PHI.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct CamaRow {
    #[serde(rename = "cama_id")]
    pub cama_id: String,
    #[serde(rename = "tenant_id")]
    pub tenant_id: String,
    #[serde(rename = "numero")]
    pub numero: u8,
    #[serde(rename = "tipo")]
    pub tipo: String,
    #[serde(rename = "estado")]
    pub estado: String,
    #[serde(rename = "paciente_id")]
    pub paciente_id: Option<String>,
    #[serde(rename = "created_at")]
    pub created_at: String,
    #[serde(rename = "phi")]
    pub phi: String,
}

/// Prepara la fila persistible de una cama: sella el PHI (paciente_nombre).
pub fn seal_cama(cama: &dmart_shared::models::Cama, tenant_id: &str) -> Result<CamaRow> {
    let phi_payload = CamaPhi {
        paciente_nombre: cama.paciente_nombre.clone(),
    };
    let ctx = cama_ctx(tenant_id, &cama.cama_id);
    let phi = cipher()
        .seal(&ctx, &phi_payload)
        .map_err(crypto_ctx)
        .context("cifrando PHI de la cama")?;

    Ok(CamaRow {
        cama_id: cama.cama_id.clone(),
        tenant_id: tenant_id.to_string(),
        numero: cama.numero,
        tipo: cama.tipo.label().to_string(),
        estado: cama.estado.label().to_string(),
        paciente_id: cama.paciente_id.clone(),
        created_at: cama.created_at.clone(),
        phi,
    })
}

/// Abre una fila de `camas` y devuelve la cama con PHI descifrado.
pub fn open_cama(value: Value) -> Result<dmart_shared::models::Cama> {
    let sealed_phi = value.get("phi").and_then(Value::as_str);
    let tenant_id = value
        .get("tenant_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let cama_id = value
        .get("cama_id")
        .and_then(Value::as_str)
        .unwrap_or_default();

    let mut cama = dmart_shared::models::Cama {
        id: None,
        cama_id: cama_id.to_string(),
        numero: value
            .get("numero")
            .and_then(Value::as_u64)
            .unwrap_or_default() as u8,
        tipo: value
            .get("tipo")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default(),
        estado: value
            .get("estado")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default(),
        paciente_id: value
            .get("paciente_id")
            .and_then(Value::as_str)
            .map(String::from),
        paciente_nombre: None,
        created_at: value
            .get("created_at")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
    };

    match sealed_phi {
        Some(phi) if !phi.is_empty() => {
            let ctx = cama_ctx(tenant_id, cama_id);
            let phi_payload: CamaPhi = cipher()
                .open(&ctx, phi)
                .map_err(crypto_ctx)
                .context("descifrando PHI de la cama")?;
            cama.paciente_nombre = phi_payload.paciente_nombre;
            Ok(cama)
        }
        _ => Ok(cama),
    }
}

/// Abre múltiples filas de camas.
pub fn open_camas(rows: Vec<Value>) -> Result<Vec<dmart_shared::models::Cama>> {
    rows.into_iter().map(open_cama).collect::<Result<Vec<_>>>()
}

/// Abre una fila de `camas` opcional.
pub fn open_cama_opt(value: Option<Value>) -> Result<Option<dmart_shared::models::Cama>> {
    match value {
        Some(v) => open_cama(v).map(Some),
        None => Ok(None),
    }
}

// ─── CarePlan PHI ───────────────────────────────────────────────────────────

const CARE_PLAN_RECORD_TYPE: &str = "care_plan";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CarePlanPhi {
    pub patient_id: String,
    pub activity: serde_json::Value,
    pub notes: Option<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct CarePlanRow {
    #[serde(rename = "care_plan_id")]
    pub care_plan_id: String,
    #[serde(rename = "tenant_id")]
    pub tenant_id: String,
    #[serde(rename = "patient_id")]
    pub patient_id: String,
    #[serde(rename = "status")]
    pub status: String,
    #[serde(rename = "intent")]
    pub intent: String,
    #[serde(rename = "created_at")]
    pub created_at: String,
    #[serde(rename = "phi")]
    pub phi: String,
}

fn care_plan_ctx(tenant_id: &str, care_plan_id: &str) -> PhiContext {
    PhiContext::new(tenant_id, CARE_PLAN_RECORD_TYPE, care_plan_id)
}

pub fn seal_care_plan(
    care_plan: &crate::cds_rules::CarePlan,
    tenant_id: &str,
) -> Result<CarePlanRow> {
    let phi_payload = CarePlanPhi {
        patient_id: care_plan.patient_id.clone(),
        activity: serde_json::to_value(&care_plan.activity).unwrap_or(serde_json::Value::Null),
        notes: None, // CarePlan no tiene campo notes en el struct actual
    };
    let ctx = care_plan_ctx(tenant_id, &care_plan.id);
    let phi = cipher()
        .seal(&ctx, &phi_payload)
        .map_err(crypto_ctx)
        .context("cifrando PHI del care_plan")?;

    Ok(CarePlanRow {
        care_plan_id: care_plan.id.clone(),
        tenant_id: tenant_id.to_string(),
        patient_id: care_plan.patient_id.clone(),
        status: care_plan.status.clone(),
        intent: care_plan.intent.clone(),
        created_at: care_plan.created_at.to_string(),
        phi,
    })
}

pub fn open_care_plan(value: Value) -> Result<crate::cds_rules::CarePlan> {
    let sealed_phi = value.get("phi").and_then(Value::as_str);
    let tenant_id = value
        .get("tenant_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let care_plan_id = value
        .get("care_plan_id")
        .and_then(Value::as_str)
        .unwrap_or_default();

    let mut cp = crate::cds_rules::CarePlan {
        id: care_plan_id.to_string(),
        patient_id: value
            .get("patient_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        status: value
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        intent: value
            .get("intent")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        activity: Vec::new(),
        created_at: value
            .get("created_at")
            .and_then(Value::as_i64)
            .unwrap_or_default(),
    };

    match sealed_phi {
        Some(phi) if !phi.is_empty() => {
            let ctx = care_plan_ctx(tenant_id, care_plan_id);
            let phi_payload: CarePlanPhi = cipher()
                .open(&ctx, phi)
                .map_err(crypto_ctx)
                .context("descifrando PHI del care_plan")?;
            cp.patient_id = phi_payload.patient_id;
            cp.activity = serde_json::from_value(phi_payload.activity).unwrap_or_default();
            Ok(cp)
        }
        _ => Ok(cp),
    }
}

// ─── AuditLog PHI ───────────────────────────────────────────────────────────

const AUDIT_LOG_RECORD_TYPE: &str = "audit_log";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AuditLogPhi {
    pub details: Option<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct AuditLogRow {
    #[serde(rename = "uid")]
    pub uid: String,
    #[serde(rename = "tenant_id")]
    pub tenant_id: String,
    #[serde(rename = "timestamp")]
    pub timestamp: String,
    #[serde(rename = "user_id")]
    pub user_id: Option<String>,
    #[serde(rename = "username")]
    pub username: Option<String>,
    #[serde(rename = "action")]
    pub action: String,
    #[serde(rename = "resource")]
    pub resource: String,
    #[serde(rename = "resource_id")]
    pub resource_id: Option<String>,
    #[serde(rename = "success")]
    pub success: bool,
    #[serde(rename = "ip_address")]
    pub ip_address: Option<String>,
    #[serde(rename = "user_agent")]
    pub user_agent: Option<String>,
    #[serde(rename = "error_message")]
    pub error_message: Option<String>,
    #[serde(rename = "prev_hash")]
    pub prev_hash: Option<String>,
    #[serde(rename = "content_hash")]
    pub content_hash: Option<String>,
    #[serde(rename = "phi")]
    pub phi: String,
}

fn audit_log_ctx(tenant_id: &str, uid: &str) -> PhiContext {
    PhiContext::new(tenant_id, AUDIT_LOG_RECORD_TYPE, uid)
}

pub fn seal_audit_log(log: &crate::audit::AuditLog, tenant_id: &str) -> Result<AuditLogRow> {
    let phi_payload = AuditLogPhi {
        details: log.details.clone(),
    };
    let ctx = audit_log_ctx(tenant_id, &log.uid);
    let phi = cipher()
        .seal(&ctx, &phi_payload)
        .map_err(crypto_ctx)
        .context("cifrando PHI del audit_log")?;

    Ok(AuditLogRow {
        uid: log.uid.clone(),
        tenant_id: tenant_id.to_string(),
        timestamp: log.timestamp.clone(),
        user_id: log.user_id.clone(),
        username: log.username.clone(),
        action: log.action.as_str().to_string(),
        resource: log.resource.clone(),
        resource_id: log.resource_id.clone(),
        success: log.success,
        ip_address: log.ip_address.clone(),
        user_agent: log.user_agent.clone(),
        error_message: log.error_message.clone(),
        prev_hash: log.prev_hash.clone(),
        content_hash: log.content_hash.clone(),
        phi,
    })
}

pub fn open_audit_log(value: Value) -> Result<crate::audit::AuditLog> {
    let sealed_phi = value.get("phi").and_then(Value::as_str);
    let tenant_id = value
        .get("tenant_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let uid = value.get("uid").and_then(Value::as_str).unwrap_or_default();

    let mut log = crate::audit::AuditLog {
        uid: uid.to_string(),
        timestamp: value
            .get("timestamp")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        user_id: value
            .get("user_id")
            .and_then(Value::as_str)
            .map(String::from),
        username: value
            .get("username")
            .and_then(Value::as_str)
            .map(String::from),
        action: value
            .get("action")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or(crate::audit::AuditAction::Read),
        resource: value
            .get("resource")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        resource_id: value
            .get("resource_id")
            .and_then(Value::as_str)
            .map(String::from),
        details: None,
        ip_address: value
            .get("ip_address")
            .and_then(Value::as_str)
            .map(String::from),
        user_agent: value
            .get("user_agent")
            .and_then(Value::as_str)
            .map(String::from),
        success: value
            .get("success")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        error_message: value
            .get("error_message")
            .and_then(Value::as_str)
            .map(String::from),
        prev_hash: value
            .get("prev_hash")
            .and_then(Value::as_str)
            .map(String::from),
        content_hash: value
            .get("content_hash")
            .and_then(Value::as_str)
            .map(String::from),
    };

    match sealed_phi {
        Some(phi) if !phi.is_empty() => {
            let ctx = audit_log_ctx(tenant_id, uid);
            let phi_payload: AuditLogPhi = cipher()
                .open(&ctx, phi)
                .map_err(crypto_ctx)
                .context("descifrando PHI del audit_log")?;
            log.details = phi_payload.details;
            Ok(log)
        }
        _ => Ok(log),
    }
}

// ─── PushSubscription PHI ───────────────────────────────────────────────────

const PUSH_SUB_RECORD_TYPE: &str = "push_sub";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PushSubPhi {
    pub endpoint: String,
    pub user_agent: Option<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct PushSubRow {
    #[serde(rename = "user_id")]
    pub user_id: String,
    #[serde(rename = "endpoint")]
    pub endpoint: String,
    #[serde(rename = "p256dh")]
    pub p256dh: String,
    #[serde(rename = "auth")]
    pub auth: String,
    #[serde(rename = "created_at")]
    pub created_at: String,
    #[serde(rename = "phi")]
    pub phi: String,
}

fn push_sub_ctx(user_id: &str, endpoint: &str) -> PhiContext {
    PhiContext::new(user_id, PUSH_SUB_RECORD_TYPE, endpoint)
}

pub fn seal_push_sub(sub: &crate::push::PushSubscriptionRow, user_id: &str) -> Result<PushSubRow> {
    let phi_payload = PushSubPhi {
        endpoint: sub.endpoint.clone(),
        user_agent: sub.user_agent.clone(),
    };
    let ctx = push_sub_ctx(user_id, &sub.endpoint);
    let phi = cipher()
        .seal(&ctx, &phi_payload)
        .map_err(crypto_ctx)
        .context("cifrando PHI de push_subscription")?;

    Ok(PushSubRow {
        user_id: sub.user_id.clone(),
        endpoint: sub.endpoint.clone(),
        p256dh: sub.p256dh.clone(),
        auth: sub.auth.clone(),
        created_at: sub.created_at.clone(),
        phi,
    })
}

/// Abre una fila de `push_subscription`.
///
/// Si la fila no tiene `phi` es una fila anterior a la migración: `user_agent`
/// sigue en la columna en claro y se lee de ahí. Sin esa rama, el backfill
/// sellaría un envelope con `user_agent: None` y el dato se perdería para
/// siempre.
///
/// El `endpoint` **no** se cifra: sin él en claro no hay entrega posible (es la
/// URL a la que se hace el POST). Aun así viaja también dentro del envelope, que
/// es lo que ata la fila a su AAD.
pub fn open_push_sub(value: Value) -> Result<crate::push::PushSubscriptionRow> {
    let sealed_phi = value.get("phi").and_then(Value::as_str);
    let user_id = value
        .get("user_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let endpoint = value
        .get("endpoint")
        .and_then(Value::as_str)
        .unwrap_or_default();

    let mut sub = crate::push::PushSubscriptionRow {
        user_id: user_id.to_string(),
        endpoint: endpoint.to_string(),
        p256dh: value
            .get("p256dh")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        auth: value
            .get("auth")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        user_agent: value
            .get("user_agent")
            .and_then(Value::as_str)
            .map(String::from),
        created_at: value
            .get("created_at")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
    };

    match sealed_phi {
        Some(phi) if !phi.is_empty() => {
            let ctx = push_sub_ctx(user_id, endpoint);
            let phi_payload: PushSubPhi = cipher()
                .open(&ctx, phi)
                .map_err(crypto_ctx)
                .context("descifrando PHI de push_subscription")?;
            sub.endpoint = phi_payload.endpoint;
            sub.user_agent = phi_payload.user_agent;
            Ok(sub)
        }
        _ => Ok(sub),
    }
}

// ─── DeviceRegistry PHI ─────────────────────────────────────────────────────

const DEVICE_REG_RECORD_TYPE: &str = "device_reg";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeviceRegPhi {
    pub fabricante: String,
    pub modelo: String,
    pub serial: String,
    pub firmware: String,
    pub ubicacion: Option<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct DeviceRegRow {
    #[serde(rename = "device_id")]
    pub device_id: String,
    #[serde(rename = "tenant_id")]
    pub tenant_id: String,
    #[serde(rename = "device_type")]
    pub device_type: String,
    #[serde(rename = "cama_id")]
    pub cama_id: Option<String>,
    #[serde(rename = "estado")]
    pub estado: String,
    #[serde(rename = "registered_at")]
    pub registered_at: i64,
    #[serde(rename = "last_seen_at")]
    pub last_seen_at: Option<i64>,
    #[serde(rename = "heartbeat_interval_secs")]
    pub heartbeat_interval_secs: i64,
    #[serde(rename = "phi")]
    pub phi: String,
}

fn device_reg_ctx(tenant_id: &str, device_id: &str) -> PhiContext {
    PhiContext::new(tenant_id, DEVICE_REG_RECORD_TYPE, device_id)
}

pub fn seal_device_registry(
    device: &crate::device_registry::ClinicalDevice,
    tenant_id: &str,
) -> Result<DeviceRegRow> {
    let phi_payload = DeviceRegPhi {
        fabricante: device.fabricante.clone(),
        modelo: device.modelo.clone(),
        serial: device.serial.clone(),
        firmware: device.firmware.clone(),
        ubicacion: device.ubicacion.clone(),
    };
    let ctx = device_reg_ctx(tenant_id, &device.id);
    let phi = cipher()
        .seal(&ctx, &phi_payload)
        .map_err(crypto_ctx)
        .context("cifrando PHI del device_registry")?;

    Ok(DeviceRegRow {
        device_id: device.id.clone(),
        tenant_id: tenant_id.to_string(),
        device_type: device.device_type.clone(),
        cama_id: device.cama_id.clone(),
        estado: device.estado.clone(),
        registered_at: device.registered_at,
        last_seen_at: device.last_seen_at,
        heartbeat_interval_secs: device.heartbeat_interval_secs,
        phi,
    })
}

pub fn open_device_registry(value: Value) -> Result<crate::device_registry::ClinicalDevice> {
    let sealed_phi = value.get("phi").and_then(Value::as_str);
    let tenant_id = value
        .get("tenant_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let device_id = value
        .get("device_id")
        .and_then(Value::as_str)
        .unwrap_or_default();

    let mut dev = crate::device_registry::ClinicalDevice {
        id: device_id.to_string(),
        device_type: value
            .get("device_type")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        fabricante: String::new(),
        modelo: String::new(),
        firmware: String::new(),
        serial: String::new(),
        cama_id: value
            .get("cama_id")
            .and_then(Value::as_str)
            .map(String::from),
        ubicacion: None,
        estado: value
            .get("estado")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        registered_at: value
            .get("registered_at")
            .and_then(Value::as_i64)
            .unwrap_or_default(),
        last_seen_at: value.get("last_seen_at").and_then(Value::as_i64),
        heartbeat_interval_secs: value
            .get("heartbeat_interval_secs")
            .and_then(Value::as_i64)
            .unwrap_or(60),
    };

    match sealed_phi {
        Some(phi) if !phi.is_empty() => {
            let ctx = device_reg_ctx(tenant_id, device_id);
            let phi_payload: DeviceRegPhi = cipher()
                .open(&ctx, phi)
                .map_err(crypto_ctx)
                .context("descifrando PHI del device_registry")?;
            dev.fabricante = phi_payload.fabricante;
            dev.modelo = phi_payload.modelo;
            dev.serial = phi_payload.serial;
            dev.firmware = phi_payload.firmware;
            dev.ubicacion = phi_payload.ubicacion;
            Ok(dev)
        }
        _ => Ok(dev),
    }
}

// ─── Reports PHI ────────────────────────────────────────────────────────────

const REPORT_RECORD_TYPE: &str = "report";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReportPhi {
    pub content: serde_json::Value,
    pub generated_by: Option<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ReportRow {
    #[serde(rename = "report_id")]
    pub report_id: String,
    #[serde(rename = "tenant_id")]
    pub tenant_id: String,
    #[serde(rename = "patient_id")]
    pub patient_id: String,
    #[serde(rename = "report_type")]
    pub report_type: String,
    #[serde(rename = "status")]
    pub status: String,
    #[serde(rename = "created_at")]
    pub created_at: String,
    #[serde(rename = "generated_by")]
    pub generated_by: Option<String>,
    #[serde(rename = "phi")]
    pub phi: String,
}

fn report_ctx(tenant_id: &str, report_id: &str) -> PhiContext {
    PhiContext::new(tenant_id, REPORT_RECORD_TYPE, report_id)
}

#[allow(clippy::too_many_arguments)]
pub fn seal_report(
    report_id: &str,
    tenant_id: &str,
    patient_id: &str,
    report_type: &str,
    status: &str,
    content: serde_json::Value,
    generated_by: Option<String>,
    created_at: &str,
) -> Result<ReportRow> {
    let phi_payload = ReportPhi {
        content,
        generated_by: generated_by.clone(),
    };
    let ctx = report_ctx(tenant_id, report_id);
    let phi = cipher()
        .seal(&ctx, &phi_payload)
        .map_err(crypto_ctx)
        .context("cifrando PHI del report")?;

    Ok(ReportRow {
        report_id: report_id.to_string(),
        tenant_id: tenant_id.to_string(),
        patient_id: patient_id.to_string(),
        report_type: report_type.to_string(),
        status: status.to_string(),
        created_at: created_at.to_string(),
        generated_by,
        phi,
    })
}

pub fn open_report(value: Value) -> Result<serde_json::Value> {
    let sealed_phi = value.get("phi").and_then(Value::as_str);
    let tenant_id = value
        .get("tenant_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let report_id = value
        .get("report_id")
        .and_then(Value::as_str)
        .unwrap_or_default();

    let mut report = serde_json::json!({
        "report_id": report_id,
        "tenant_id": tenant_id,
        "patient_id": value.get("patient_id").and_then(Value::as_str).unwrap_or_default(),
        "report_type": value.get("report_type").and_then(Value::as_str).unwrap_or_default(),
        "status": value.get("status").and_then(Value::as_str).unwrap_or_default(),
        "created_at": value.get("created_at").and_then(Value::as_str).unwrap_or_default(),
        "generated_by": value.get("generated_by").and_then(Value::as_str),
    });

    match sealed_phi {
        Some(phi) if !phi.is_empty() => {
            let ctx = report_ctx(tenant_id, report_id);
            let phi_payload: ReportPhi = cipher()
                .open(&ctx, phi)
                .map_err(crypto_ctx)
                .context("descifrando PHI del report")?;
            report["content"] = phi_payload.content;
            report["generated_by"] =
                serde_json::to_value(phi_payload.generated_by).unwrap_or(serde_json::Value::Null);
            Ok(report)
        }
        _ => Ok(report),
    }
}

/// Abre una fila de `patients` y devuelve el paciente.
///
/// Si la fila no tiene `phi` es una fila anterior a la migración: se
/// deserializa desde las columnas legacy. Ese camino es de solo lectura; la
/// siguiente escritura la re-cifra automáticamente.
pub fn open_patient(value: Value) -> Result<Patient> {
    let sealed_phi = value.get("phi").and_then(Value::as_str);
    match sealed_phi {
        Some(phi) if !phi.is_empty() => {
            let tenant_id = value
                .get("tenant_id")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let patient_id = value
                .get("patient_id")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let ctx = patient_ctx(tenant_id, patient_id);
            cipher()
                .open::<Patient>(&ctx, phi)
                .map_err(crypto_ctx)
                .context("abriendo envelope de PHI del paciente")
        }
        _ => open_legacy_patient(value),
    }
}

/// Abre una fila anterior a la migración, que aún tiene la PHI en claro.
///
/// `Patient` declara `#[serde(default)]` en todos sus campos, así que
/// deserializar sin más convertiría una fila corrupta o de otro esquema en un
/// paciente con todos los campos vacíos, y ese silencio sería indistinguible de
/// "así se guardó". Por eso antes de deserializar se exige que la fila traiga
/// las claves de identidad que toda fila legacy válida tiene.
fn open_legacy_patient(value: Value) -> Result<Patient> {
    const REQUIRED: &[&str] = &["patient_id", "historia_clinica", "cedula", "nombre"];
    let obj = value
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("fila de paciente legacy no es un objeto"))?;
    let missing: Vec<&str> = REQUIRED
        .iter()
        .copied()
        .filter(|k| !obj.contains_key(*k))
        .collect();
    if !missing.is_empty() {
        return Err(anyhow::anyhow!(
            "fila de paciente legacy incompleta (faltan {missing:?}); \
             no se deserializa para no devolver un paciente vacío por defecto"
        ));
    }
    serde_json::from_value(value)
        .map_err(|e| anyhow::anyhow!("fila de paciente legacy ilegible: {e}"))
}

/// Igual que [`open_patient`] pero para un registro `Option` de SurrealDB.
pub fn open_patient_opt(value: Option<Value>) -> Result<Option<Patient>> {
    value.map(open_patient).transpose()
}

/// Abre una lista de filas de `patients`.
pub fn open_patients(rows: Vec<Value>) -> Result<Vec<Patient>> {
    rows.into_iter().map(open_patient).collect()
}

/// Guarda un paciente cifrando su PHI y devuelve el paciente tal como quedó.
///
/// El `patient_id` se completa con UUID si viene vacío, como antes.
pub async fn save_patient(db: &Surreal<Db>, mut patient: Patient) -> Result<Patient> {
    if patient.patient_id.is_empty() {
        patient.patient_id = uuid::Uuid::new_v4().to_string();
    }
    let row = seal_patient(&patient)?;
    let saved: Option<PatientRow> = db
        .create(("patients", row.patient_id.clone()))
        .content(row)
        .await
        .context("escribiendo paciente cifrado")?;
    // Se devuelve el documento en claro que el propio caller acaba de sellar, en
    // lugar de reabrir el envelope: el resultado es idéntico y evita un
    // descifrado por escritura.
    let _ = saved;
    Ok(patient)
}

/// Actualiza un paciente, re-sellando su PHI bajo el contexto vigente.
pub async fn update_patient(
    db: &Surreal<Db>,
    id: &str,
    patient: Patient,
) -> Result<Option<Patient>> {
    let row = seal_patient(&patient)?;
    let _: Option<PatientRow> = db
        .update(("patients", id.to_string()))
        .content(row)
        .await
        .context("actualizando paciente cifrado")?;
    Ok(Some(patient))
}

/// Resuelve pacientes por MRN/cédula con coincidencia exacta, acotados a un
/// tenant.
///
/// Reemplaza la búsqueda con comodines (`historia_clinica ~ $q`) que era
/// necesaria cuando el valor estaba en claro. Con la PHI cifrada la
/// normalización se resuelve antes del HMAC, así que `MRN-001` encuentra
/// `mrn 001` sin escanear la colección ni exponer el identificador.
pub async fn find_patients_by_identifier(
    db: &Surreal<Db>,
    tenant_id: &str,
    identifier: &str,
    limit: u32,
) -> Result<Vec<Patient>> {
    let q = identifier.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let bi_hc = blind(F_HISTORIA_CLINICA, tenant_id, q);
    let bi_ced = blind(F_CEDULA, tenant_id, q);
    let bi_hc_legacy = blind_legacy(F_HISTORIA_CLINICA, tenant_id, q);
    let bi_ced_legacy = blind_legacy(F_CEDULA, tenant_id, q);

    let rows: Vec<Value> = db
        .query(
            "SELECT * OMIT id FROM patients WHERE tenant_id = $tenant \
             AND (bi_hc = $bi_hc OR bi_ced = $bi_ced \
                  OR bi_hc = $bi_hc_legacy OR bi_ced = $bi_ced_legacy \
                  OR historia_clinica = $q OR cedula = $q) \
             ORDER BY created_at DESC LIMIT $limit",
        )
        .bind(("tenant", tenant_id.to_string()))
        .bind(("q", q.to_string()))
        .bind(("bi_hc", bi_hc))
        .bind(("bi_ced", bi_ced))
        .bind(("bi_hc_legacy", bi_hc_legacy))
        .bind(("bi_ced_legacy", bi_ced_legacy))
        .bind(("limit", limit.min(100) as i64))
        .await
        .context("buscando paciente por identificador")?
        .take(0)?;
    open_patients(rows)
}

/// Búsqueda de pacientes por historia clínica, cédula o nombre: coincidencia
/// **exacta** sobre los tres índices ciegos (formato actual + legacy).
pub async fn search_patients_exact(
    db: &Surreal<Db>,
    tenant_id: &str,
    query: &str,
    extra_where: &str,
    limit: u32,
    offset: u32,
) -> Result<Vec<Patient>> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let bi_hc = blind(F_HISTORIA_CLINICA, tenant_id, q);
    let bi_ced = blind(F_CEDULA, tenant_id, q);
    let bi_nombre = blind(F_NOMBRE, tenant_id, q);
    // Legacy (sin longitudes prefijadas) para filas cifradas antes de la migración.
    let bi_hc_legacy = blind_legacy(F_HISTORIA_CLINICA, tenant_id, q);
    let bi_ced_legacy = blind_legacy(F_CEDULA, tenant_id, q);
    let bi_nombre_legacy = blind_legacy(F_NOMBRE, tenant_id, q);

    let rows: Vec<Value> = db
        .query(format!(
            "SELECT * OMIT id FROM patients WHERE tenant_id = $tenant \
             AND (bi_hc = $bi_hc OR bi_ced = $bi_ced OR bi_nombre = $bi_nombre \
                  OR bi_hc = $bi_hc_legacy OR bi_ced = $bi_ced_legacy OR bi_nombre = $bi_nombre_legacy) \
             {extra_where} ORDER BY created_at DESC LIMIT $limit START $offset"
        ))
        .bind(("tenant", tenant_id.to_string()))
        .bind(("bi_hc", bi_hc))
        .bind(("bi_ced", bi_ced))
        .bind(("bi_nombre", bi_nombre))
        .bind(("bi_hc_legacy", bi_hc_legacy))
        .bind(("bi_ced_legacy", bi_ced_legacy))
        .bind(("bi_nombre_legacy", bi_nombre_legacy))
        .bind((
            "limit",
            limit.min(dmart_shared::models::MAX_PAGE_LIMIT) as i64,
        ))
        .bind(("offset", offset as i64))
        .await
        .context("buscando pacientes por índice ciego")?
        .take(0)?;

    // Fallback para filas heredadas, que no tienen `bi_*`: se filtran en Rust
    // contra las columnas legacy con el mismo criterio exacto.
    let mut out = open_patients(rows)?;
    if let Ok(legacy) = legacy_matches(db, tenant_id, q, extra_where).await {
        out.extend(legacy);
    }
    Ok(out)
}

/// Búsqueda exacta sobre filas heredadas que aún no tienen índice ciego.
async fn legacy_matches(
    db: &Surreal<Db>,
    tenant_id: &str,
    q: &str,
    extra_where: &str,
) -> Result<Vec<Patient>> {
    let rows: Vec<Value> = db
        .query(format!(
            "SELECT * OMIT id FROM patients WHERE tenant_id = $tenant \
             AND phi IS NONE AND (historia_clinica = $q OR cedula = $q OR nombre = $q) \
             {extra_where}"
        ))
        .bind(("tenant", tenant_id.to_string()))
        .bind(("q", q.to_string()))
        .await?
        .take(0)?;
    open_patients(rows)
}

/// Cuenta pacientes que casan con una búsqueda exacta.
pub async fn count_patients_exact(
    db: &Surreal<Db>,
    tenant_id: &str,
    query: &str,
    extra_where: &str,
) -> Result<u64> {
    let found = search_patients_exact(db, tenant_id, query, extra_where, 1000, 0).await?;
    Ok(found.len() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dmart_shared::models::{SeverityLevel, Sexo};

    fn paciente() -> Patient {
        Patient {
            patient_id: "P-1".into(),
            tenant_id: "hosp-a".into(),
            nombre: "María Fernández".into(),
            apellido: "Ruiz".into(),
            cedula: "1012345678".into(),
            historia_clinica: "MRN-001".into(),
            fecha_nacimiento: "1960-05-04".into(),
            direccion: "Calle 45 #12-30".into(),
            edad: 63,
            sexo: Sexo::Femenino,
            estado_gravedad: SeverityLevel::Moderado,
            created_at: "2024-01-01T00:00:00Z".into(),
            updated_at: "2024-01-01T00:00:00Z".into(),
            ..Patient::default()
        }
    }

    #[test]
    fn seal_keeps_phi_out_of_plaintext_columns() {
        let row = seal_patient(&paciente()).expect("seal");
        let json = serde_json::to_string(&row).expect("json");
        for secreto in [
            "María",
            "Fernández",
            "1012345678",
            "MRN-001",
            "1960-05-04",
            "Calle 45",
        ] {
            assert!(!json.contains(secreto), "columna en claro filtró {secreto}");
        }
        // Las columnas de filtro siguen disponibles en claro.
        assert_eq!(row.tenant_id, "hosp-a");
        assert_eq!(row.patient_id, "P-1");
        assert_eq!(row.estado_gravedad, "Moderado");
    }

    #[test]
    fn roundtrip_preserves_all_fields() {
        let original = paciente();
        let row = seal_patient(&original).expect("seal");
        let value = serde_json::to_value(&row).expect("value");
        let opened = open_patient(value).expect("open");
        assert_eq!(opened.nombre, original.nombre);
        assert_eq!(opened.apellido, original.apellido);
        assert_eq!(opened.cedula, original.cedula);
        assert_eq!(opened.historia_clinica, original.historia_clinica);
        assert_eq!(opened.direccion, original.direccion);
        assert_eq!(opened.fecha_nacimiento, original.fecha_nacimiento);
        assert_eq!(opened.edad, original.edad);
        assert_eq!(opened.sexo, original.sexo);
        assert_eq!(opened.estado_gravedad, original.estado_gravedad);
    }

    /// Mover el envelope a otro registro o tenant debe fallar, no devolver la
    /// PHI del paciente original.
    #[test]
    fn envelope_is_bound_to_tenant_and_record() {
        let mut row = seal_patient(&paciente()).expect("seal");
        let mut value = serde_json::to_value(&row).expect("value");

        value["tenant_id"] = Value::String("hosp-b".into());
        assert!(open_patient(value).is_err(), "otro tenant no debe abrir");

        let mut value = serde_json::to_value(&row).expect("value");
        value["patient_id"] = Value::String("P-2".into());
        assert!(open_patient(value).is_err(), "otro registro no debe abrir");

        // Alterar la PHI cifrada rompe la autenticación.
        row.phi.push('A');
        assert!(open_patient(serde_json::to_value(&row).expect("value")).is_err());
    }

    #[test]
    fn blind_indexes_are_tenant_scoped() {
        let a = seal_patient(&paciente()).expect("a");
        let mut otro = paciente();
        otro.tenant_id = "hosp-b".into();
        let b = seal_patient(&otro).expect("b");
        assert_ne!(
            a.bi_hc, b.bi_hc,
            "el MRN no debe correlacionarse entre tenants"
        );
        assert_ne!(a.bi_ced, b.bi_ced);
    }

    /// Las filas anteriores a la migración (sin `phi`) siguen siendo legibles.
    #[test]
    fn legacy_row_without_envelope_is_readable() {
        let mut legacy = serde_json::to_value(paciente()).expect("value");
        legacy.as_object_mut().expect("obj").remove("phi");
        let opened = open_patient(legacy).expect("legacy");
        assert_eq!(opened.cedula, "1012345678");
        assert_eq!(opened.nombre, "María Fernández");
    }

    /// Si `phi` falta pero la fila está corrupta, debe fallar y no devolver un
    /// paciente vacío (los `#[serde(default)]` de `Patient` lo ocultarían).
    #[test]
    fn corrupt_legacy_row_fails_loudly() {
        let value = serde_json::json!({ "patient_id": "P-1" });
        assert!(open_patient(value).is_err());
    }
}

// ── Cobertura contra la base real (SurrealKV) ─────────────────────────────

#[cfg(test)]
mod db_tests {
    use super::*;
    use crate::crypto::AES256_MAGIC;
    use dmart_shared::models::Patient;
    use surrealdb::engine::local::SurrealKv;

    async fn test_db() -> (Surreal<Db>, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("tmpdir");
        let db = Surreal::new::<SurrealKv>(dir.path()).await.expect("db");
        db.use_ns("dmart").use_db("dmart").await.expect("ns");
        crate::migrations::run_migrations(&db)
            .await
            .expect("migrations");
        (db, dir)
    }

    fn paciente(tenant: &str, hc: &str, cedula: &str, nombre: &str) -> Patient {
        let mut p = Patient::new();
        p.tenant_id = tenant.into();
        p.historia_clinica = hc.into();
        p.cedula = cedula.into();
        p.nombre = nombre.into();
        p.apellido = "Prueba".into();
        p
    }

    /// Ninguna columna en claro debe contener PHI identificable.
    #[tokio::test]
    async fn nothing_identifiable_is_stored_in_cleartext() {
        let (db, _dir) = test_db().await;
        let mut p = paciente("hosp-a", "MRN-001", "1098765432", "María Fernández Ruiz");
        p.direccion = "Calle 45 #12-30".into();
        p.fecha_nacimiento = "1960-05-04".into();
        p.familiar_encargado = "3001234567".into();
        save_patient(&db, p).await.expect("save");

        // Se lee el registro crudo, sin pasar por la capa de descifrado.
        let rows: Vec<Value> = db
            .query("SELECT * OMIT id FROM patients")
            .await
            .expect("query")
            .take(0)
            .expect("take");
        let crudo = serde_json::to_string(&rows[0]).expect("json");

        for secreto in [
            "María",
            "Fernández",
            "Ruiz",
            "1098765432",
            "MRN-001",
            "1960-05-04",
            "Calle 45",
            "3001234567",
        ] {
            assert!(!crudo.contains(secreto), "PHI en claro filtró: {secreto}");
        }
        // Y sí quedan las columnas de filtro, y la columna `phi` está poblada.
        assert!(crudo.contains("hosp-a"));
        let phi = rows[0]
            .get("phi")
            .and_then(Value::as_str)
            .expect("columna phi presente");
        assert!(!phi.is_empty());
        // El magic viaja dentro del base64, así que se comprueba sobre el blob.
        let blob = crate::crypto::base64_decode(phi).expect("base64 válido");
        assert_eq!(&blob[..AES256_MAGIC.len()], AES256_MAGIC);
    }

    /// La búsqueda por MRN debe funcionar con el valor cifrado, ignorando
    /// diferencias de formato.
    #[tokio::test]
    async fn mrn_search_is_exact_but_format_insensitive() {
        let (db, _dir) = test_db().await;
        save_patient(&db, paciente("hosp-a", "MRN-001", "1098765432", "Ana"))
            .await
            .expect("save");

        for variante in ["MRN-001", "mrn-001", " MRN 001 ", "mrn.001"] {
            let found = find_patients_by_identifier(&db, "hosp-a", variante, 10)
                .await
                .expect("find");
            assert_eq!(found.len(), 1, "variante {variante} no encontró");
            assert_eq!(found[0].historia_clinica, "MRN-001");
        }
        // Un MRN distinto no debe aparecer.
        assert!(
            find_patients_by_identifier(&db, "hosp-a", "MRN-999", 10)
                .await
                .expect("find")
                .is_empty()
        );
    }

    /// La búsqueda por MRN debe respetar el tenant: el mismo MRN en dos
    /// hospitales devuelve el paciente de cada uno, no se mezclan.
    #[tokio::test]
    async fn mrn_search_never_crosses_tenants() {
        let (db, _dir) = test_db().await;
        save_patient(&db, paciente("hosp-a", "MRN-777", "111", "Ana A"))
            .await
            .expect("save a");
        save_patient(&db, paciente("hosp-b", "MRN-777", "222", "Ana B"))
            .await
            .expect("save b");

        let a = find_patients_by_identifier(&db, "hosp-a", "MRN-777", 10)
            .await
            .expect("a");
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].nombre, "Ana A");
        assert_eq!(a[0].tenant_id, "hosp-a");

        let b = find_patients_by_identifier(&db, "hosp-b", "MRN-777", 10)
            .await
            .expect("b");
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].nombre, "Ana B");
    }

    #[tokio::test]
    async fn roundtrip_through_the_database() {
        let (db, _dir) = test_db().await;
        let original = paciente("hosp-a", "MRN-001", "1098765432", "María Fernández");
        let saved = save_patient(&db, original.clone()).await.expect("save");

        let loaded = crate::db::get_patient(&db, &saved.patient_id)
            .await
            .expect("get")
            .expect("found");
        assert_eq!(loaded.cedula, original.cedula);
        assert_eq!(loaded.historia_clinica, original.historia_clinica);
        assert_eq!(loaded.nombre, original.nombre);
        assert_eq!(loaded.apellido, original.apellido);
        assert_eq!(loaded.tenant_id, original.tenant_id);
    }

    /// Actualizar re-sella: el valor nuevo se recupera y el anterior ya no está
    /// disponible ni en claro ni por índice ciego.
    #[tokio::test]
    async fn update_reseals_phi() {
        let (db, _dir) = test_db().await;
        let original = paciente(
            "hosp-a",
            "MRN-OLD",
            "CEDULA-LEGACY-X9Q",
            "NOMBRE-VIEJO-SENTINELA-Z7R",
        );
        let saved = save_patient(&db, original).await.expect("save");

        let mut updated = saved.clone();
        updated.cedula = "CEDULA-NUEVA-A3K".into();
        updated.historia_clinica = "MRN-NEW".into();
        updated.nombre = "NOMBRE-NUEVO-B8Y".into();
        update_patient(&db, &saved.patient_id, updated)
            .await
            .expect("update");

        let rows: Vec<Value> = db
            .query("SELECT * OMIT id FROM patients")
            .await
            .expect("q")
            .take(0)
            .expect("t");
        assert_eq!(rows.len(), 1, "no debe duplicar la fila");

        let row = &rows[0];
        let crudo = serde_json::to_string(row).expect("json");

        let legacy_keys = ["cedula", "historia_clinica", "nombre", "apellido"];
        for k in legacy_keys {
            assert!(
                !row.get(k).is_some(),
                "columna legacy '{k}' no debe existir en la fila sellada"
            );
        }

        assert!(!crudo.contains("NOMBRE-VIEJO-SENTINELA-Z7R"));
        assert!(!crudo.contains("CEDULA-LEGACY-X9Q"));

        assert!(
            find_patients_by_identifier(&db, "hosp-a", "MRN-OLD", 10)
                .await
                .expect("find")
                .is_empty()
        );
        let found = find_patients_by_identifier(&db, "hosp-a", "MRN-NEW", 10)
            .await
            .expect("find");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].nombre, "NOMBRE-NUEVO-B8Y");
    }

    /// Una fila legacy (sin `phi`) sigue encontrándose por sus columnas en
    /// claro, y la siguiente escritura la migra.
    #[tokio::test]
    async fn legacy_row_is_readable_and_migrates_on_write() {
        let (db, _dir) = test_db().await;
        let legacy = paciente("hosp-a", "MRN-LEG", "555", "Paciente Viejo");
        let pid = legacy.patient_id.clone();
        // Se inserta deliberadamente en claro, como la migración anterior.
        let row = serde_json::to_value(&legacy).expect("value");
        db.query("CREATE $rec CONTENT $row")
            .bind(("rec", crate::db::sdb_id_pub("patients", &pid)))
            .bind(("row", row))
            .await
            .expect("legacy insert");

        let found = find_patients_by_identifier(&db, "hosp-a", "MRN-LEG", 10)
            .await
            .expect("find");
        assert_eq!(found.len(), 1, "la fila legacy debe seguir encontrándose");
        assert_eq!(found[0].nombre, "Paciente Viejo");

        // Al reescribir, queda cifrada.
        update_patient(&db, &pid, legacy).await.expect("update");
        let rows: Vec<Value> = db
            .query("SELECT * OMIT id FROM patients")
            .await
            .expect("q")
            .take(0)
            .expect("t");
        assert!(
            !serde_json::to_string(&rows[0])
                .expect("json")
                .contains("Paciente Viejo")
        );
    }

    /// Mover un envelope cifrado a otro registro debe fallar al abrir: el AAD
    /// incluye el `record_id`, así que el tag deja de validar.
    #[tokio::test]
    async fn moving_envelope_between_records_is_rejected() {
        let (db, _dir) = test_db().await;
        let victima = save_patient(&db, paciente("hosp-a", "MRN-B", "222", "Ana B"))
            .await
            .expect("save");

        // Se sella la PHI de B bajo el contexto de un registro inexistente y se
        // escribe en la fila de B: el descifrado debe rechazarlo.
        let envelope = cipher()
            .seal(
                &patient_ctx("hosp-a", "registro-ajeno"),
                &paciente("hosp-a", "MRN-B", "222", "Ana B"),
            )
            .expect("seal");
        db.query("UPDATE $target SET phi = $phi")
            .bind((
                "target",
                crate::db::sdb_id_pub("patients", &victima.patient_id),
            ))
            .bind(("phi", envelope))
            .await
            .expect("update");

        assert!(
            crate::db::get_patient(&db, &victima.patient_id)
                .await
                .is_err(),
            "un envelope con AAD de otro registro debe fallar"
        );
    }

    /// Y lo mismo entre tenants: mismo `patient_id`, distinto `tenant_id`.
    #[tokio::test]
    async fn moving_envelope_across_tenants_is_rejected() {
        let (db, _dir) = test_db().await;
        let p = paciente("hosp-a", "MRN-SHARED", "333", "Ana A");
        let pid = p.patient_id.clone();
        save_patient(&db, p).await.expect("save");

        // Se re-sella bajo el tenant contrario y se sobrescribe la fila.
        let envelope = cipher()
            .seal(
                &patient_ctx("hosp-b", &pid),
                &paciente("hosp-b", "X", "Y", "Ana B"),
            )
            .expect("seal");
        db.query("UPDATE $target SET phi = $phi")
            .bind(("target", crate::db::sdb_id_pub("patients", &pid)))
            .bind(("phi", envelope))
            .await
            .expect("update");

        assert!(
            crate::db::get_patient(&db, &pid).await.is_err(),
            "un envelope de otro tenant debe fallar"
        );
    }

    /// La búsqueda exacta no debe filtrar PHI: los resultados sólo aparecen
    /// al comparar índices ciegos, nunca por subcadena.
    #[tokio::test]
    async fn partial_search_no_longer_matches_substrings() {
        let (db, _dir) = test_db().await;
        save_patient(&db, paciente("hosp-a", "MRN-001", "111", "Ana"))
            .await
            .expect("save");

        // Coincidencia parcial: antes devolvía el paciente, ahora no.
        assert!(
            find_patients_by_identifier(&db, "hosp-a", "RN-00", 10)
                .await
                .expect("find")
                .is_empty()
        );
        // Coincidencia exacta: sí.
        assert_eq!(
            find_patients_by_identifier(&db, "hosp-a", "MRN-001", 10)
                .await
                .expect("find")
                .len(),
            1
        );
    }
}
