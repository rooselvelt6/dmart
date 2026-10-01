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
use dmart_shared::models::Patient;
use serde_json::Value;
use surrealdb::Surreal;
use surrealdb::engine::local::Db;

use crate::crypto::{CryptoError, PhiCipher, PhiContext};

/// Nombre lógico del agregado, usado como parte del AAD.
const PATIENT_RECORD_TYPE: &str = "patient";
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
pub fn cipher() -> &'static PhiCipher {
    CIPHER.get_or_init(PhiCipher::from_env)
}

/// Contexto de cifrado de un paciente. El `record_id` es el `patient_id` (id
/// público), no el id interno del record de SurrealDB: así el mismo paciente
/// conserva su envelope aunque cambie de clave de partición.
fn patient_ctx(tenant_id: &str, patient_id: &str) -> PhiContext {
    PhiContext::new(tenant_id, PATIENT_RECORD_TYPE, patient_id)
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

/// Cláusula SurrealQL de coincidencia exacta sobre índices ciegos.
///
/// Cada índice se compara contra **su propio** parámetro: `bi_hc = $bi_hc`. Una
/// columna `bi_hc = $q` con `$q` siendo el identificador en claro no encontraría
/// nunca la fila y además metería el MRN en el plan de la consulta.
///
/// Los términos `historia_clinica`/`cedula` sólo aplican a filas heredadas, que
/// aún no tienen `bi_*`. Se pueden retirar cuando la tabla esté migrada.
fn exact_match_clause(indexes: &[(&str, &str)], legacy_columns: &[&str]) -> String {
    let by_index = indexes
        .iter()
        .map(|(col, param)| format!("{col} = ${param}"))
        .collect::<Vec<_>>();
    let legacy = legacy_columns
        .iter()
        .map(|c| format!("{c} = $q"))
        .collect::<Vec<_>>();
    match (by_index.is_empty(), legacy.is_empty()) {
        (true, true) => "FALSE".to_string(),
        (true, false) => legacy.join(" OR "),
        (false, true) => by_index.join(" OR "),
        (false, false) => {
            let mut parts = by_index;
            parts.extend(legacy);
            format!("({})", parts.join(" OR "))
        }
    }
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
    let rows: Vec<Value> = db
        .query(format!(
            "SELECT * OMIT id FROM patients WHERE tenant_id = $tenant \
             AND ({}) \
             ORDER BY created_at DESC LIMIT $limit",
            exact_match_clause(
                &[("bi_hc", "bi_hc"), ("bi_ced", "bi_ced")],
                &["historia_clinica", "cedula"],
            )
        ))
        .bind(("tenant", tenant_id.to_string()))
        .bind(("q", q.to_string()))
        .bind(("bi_hc", blind(F_HISTORIA_CLINICA, tenant_id, q)))
        .bind(("bi_ced", blind(F_CEDULA, tenant_id, q)))
        .bind(("limit", limit.min(100) as i64))
        .await
        .context("buscando paciente por identificador")?
        .take(0)?;
    open_patients(rows)
}

/// Búsqueda de pacientes por historia clínica, cédula o nombre: coincidencia
/// **exacta** sobre los tres índices ciegos.
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

    let rows: Vec<Value> = db
        .query(format!(
            "SELECT * OMIT id FROM patients WHERE tenant_id = $tenant \
             AND (bi_hc = $bi_hc OR bi_ced = $bi_ced OR bi_nombre = $bi_nombre) \
             {extra_where} ORDER BY created_at DESC LIMIT $limit START $offset"
        ))
        .bind(("tenant", tenant_id.to_string()))
        .bind(("bi_hc", bi_hc))
        .bind(("bi_ced", bi_ced))
        .bind(("bi_nombre", bi_nombre))
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
        let original = paciente("hosp-a", "MRN-OLD", "111", "Nombre Viejo");
        let saved = save_patient(&db, original).await.expect("save");

        let mut updated = saved.clone();
        updated.cedula = "999".into();
        updated.historia_clinica = "MRN-NEW".into();
        updated.nombre = "Nombre Nuevo".into();
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
        let crudo = serde_json::to_string(&rows[0]).expect("json");
        assert!(!crudo.contains("Nombre Viejo"));
        assert!(!crudo.contains("111"));

        // El índice ciego del MRN antiguo ya no casa; el nuevo sí.
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
        assert_eq!(found[0].nombre, "Nombre Nuevo");
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
