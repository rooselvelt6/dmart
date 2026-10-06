#![allow(dead_code)]

use crate::phi_store;
use anyhow::{Context, Result};
use serde_json::Value;

/// Variante pública de [`sdb_id`] para uso desde los tests del crate.
pub fn sdb_id_pub(table: &str, id: &str) -> surrealdb::sql::Thing {
    sdb_id(table, id)
}

/// Construye un `Thing` de SurrealDB para vincularlo como parámetro de query.
fn sdb_id(table: &str, id: &str) -> surrealdb::sql::Thing {
    surrealdb::sql::Thing::from((table, id))
}
use dmart_shared::models::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use surrealdb::Surreal;
use surrealdb::engine::local::{Db, SurrealKv};
use uuid::Uuid;

pub type Database = Arc<Surreal<Db>>;

pub async fn connect(path: &str) -> Result<Database> {
    // Ensure parent directory exists for persistence
    if let Some(parent) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(parent)?;
    }

    let db = Surreal::new::<SurrealKv>(path).await?;
    db.use_ns("dmart").use_db("icu").await?;

    // Esquema versionado: aplica índices y cambios pendientes (idempotente).
    let applied = crate::migrations::run_migrations(&db).await?;
    if !applied.is_empty() {
        tracing::info!("🧬 Migrations applied: {}", applied.join(", "));
    }

    Ok(Arc::new(db))
}

// ─── Diagnósticos CIE-10 (persistidos en SurrealDB) ─────────────────

/// Seeds el catálogo CIE-10 en la tabla `diagnosticos` si aún no existe.
/// El catálogo vive en `dmart_shared::models::diagnosticos_uci()` como única
/// fuente, pero se persiste para sobrevivir reinicios.
pub async fn seed_diagnosticos(db: &Surreal<Db>) -> Result<()> {
    let existing: Vec<Diagnostico> = db.select("diagnosticos").await.unwrap_or_default();
    if !existing.is_empty() {
        return Ok(());
    }
    for d in diagnosticos_uci() {
        let _: Option<Diagnostico> = db
            .create(("diagnosticos", d.codigo.clone()))
            .content(d)
            .await?;
    }
    tracing::info!("🧬 Seeded {} CIE-10 diagnostics", diagnosticos_uci().len());
    Ok(())
}

pub async fn list_diagnosticos(db: &Surreal<Db>) -> Result<Vec<Diagnostico>> {
    let mut diags: Vec<Diagnostico> = db.select("diagnosticos").await?;
    diags.sort_by(|a, b| a.codigo.cmp(&b.codigo));
    Ok(diags)
}

pub async fn search_diagnosticos(db: &Surreal<Db>, query: &str) -> Result<Vec<Diagnostico>> {
    let q = query.trim();
    if q.is_empty() {
        return list_diagnosticos(db).await;
    }
    let like = q.to_lowercase();
    let diags: Vec<Diagnostico> = db
        .query(
            "SELECT * FROM diagnosticos WHERE \
                string::contains(string::lowercase(codigo), $q) OR \
                string::contains(string::lowercase(descripcion), $q) OR \
                string::contains(string::lowercase(categoria), $q)",
        )
        .bind(("q", like))
        .await?
        .take(0)?;
    Ok(diags)
}

// ─── MFA TOTP settings ───────────────────────────────────────────────

pub async fn get_mfa_settings(db: &Surreal<Db>, user_id: &str) -> Result<Option<MfaSettings>> {
    let settings: Option<MfaSettings> = db.select(("mfa_settings", user_id)).await?;
    Ok(settings)
}

pub async fn upsert_mfa_settings(db: &Surreal<Db>, settings: MfaSettings) -> Result<()> {
    let _: Vec<MfaSettings> = db
        .query("UPSERT type::thing('mfa_settings', $id) CONTENT $data RETURN AFTER")
        .bind(("id", settings.user_id.clone()))
        .bind(("data", settings))
        .await?
        .take(0)?;
    Ok(())
}

pub async fn delete_mfa_settings(db: &Surreal<Db>, user_id: &str) -> Result<()> {
    let _: Option<MfaSettings> = db.delete(("mfa_settings", user_id)).await?;
    Ok(())
}

// ─── Stats (agregaciones server-side, sin cargar filas) ──────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PatientAggregates {
    pub total: u64,
    pub criticos: u64,
    pub severos: u64,
    pub moderados: u64,
    pub bajos: u64,
    pub apache_sum: f64,
    pub apache_n: u64,
    pub gcs_sum: f64,
    pub gcs_n: u64,
    pub sofa_sum: f64,
    pub sofa_n: u64,
    pub saps3_sum: f64,
    pub saps3_n: u64,
    pub news2_sum: f64,
    pub news2_n: u64,
    pub egresados: u64,
    pub fallecidos: u64,
    pub mortalidad_predicha_sum: f64,
    pub mortalidad_predicha_n: u64,
    pub los_dias_sum: f64,
    pub los_dias_n: u64,
}

/// Calcula totales y promedios de scores con agregaciones SurrealQL en lugar
/// de cargar los pacientes en memoria.
pub async fn aggregate_patient_stats(db: &Surreal<Db>) -> Result<PatientAggregates> {
    aggregate_patient_stats_scoped(db, None).await
}

/// SPEC-025: igual que [`aggregate_patient_stats`] pero solo para el tenant
/// indicado (los KPIs ejecutivos no pueden sumar pacientes de otros tenants).
pub async fn aggregate_patient_stats_for_tenant(
    db: &Surreal<Db>,
    tenant_id: &str,
) -> Result<PatientAggregates> {
    aggregate_patient_stats_scoped(db, Some(tenant_id)).await
}

async fn aggregate_patient_stats_scoped(
    db: &Surreal<Db>,
    tenant: Option<&str>,
) -> Result<PatientAggregates> {
    let mut agg = PatientAggregates::default();
    let (scope_where, bind) = match tenant {
        Some(t) => (
            "WHERE tenant_id = $tenant ".to_string(),
            Some(t.to_string()),
        ),
        None => (String::new(), None),
    };

    let mut groups_q = db.query(format!(
        "SELECT estado_gravedad, count() AS n FROM patients {scope_where}GROUP BY estado_gravedad"
    ));
    if let Some(t) = &bind {
        groups_q = groups_q.bind(("tenant", t.clone()));
    }
    let groups: Vec<serde_json::Value> = groups_q.await?.take(0)?;
    for row in groups {
        let (Some(sev), Some(n)) = (
            row.get("estado_gravedad").and_then(|v| v.as_str()),
            row.get("n").and_then(|v| v.as_u64()),
        ) else {
            continue;
        };
        match sev {
            "Critico" => agg.criticos = n,
            "Severo" => agg.severos = n,
            "Moderado" => agg.moderados = n,
            _ => agg.bajos += n,
        }
    }

    let mut scores_q = db.query(format!(
        "SELECT ultimo_apache_score, ultimo_gcs_score, ultimo_sofa_score, ultimo_saps3_score, ultimo_news2_score, fecha_egreso_uci, fecha_ingreso_uci, desenlace_uci, mortality_risk FROM patients {scope_where}"
    ));
    if let Some(t) = &bind {
        scores_q = scores_q.bind(("tenant", t.clone()));
    }
    let scores: Vec<serde_json::Value> = scores_q.await?.take(0)?;

    agg.total = scores.len() as u64;
    for row in scores {
        if let Some(v) = row.get("ultimo_apache_score").and_then(|v| v.as_f64()) {
            agg.apache_sum += v;
            agg.apache_n += 1;
        }
        if let Some(v) = row.get("ultimo_gcs_score").and_then(|v| v.as_f64()) {
            agg.gcs_sum += v;
            agg.gcs_n += 1;
        }
        if let Some(v) = row.get("ultimo_sofa_score").and_then(|v| v.as_f64()) {
            agg.sofa_sum += v;
            agg.sofa_n += 1;
        }
        if let Some(v) = row.get("ultimo_saps3_score").and_then(|v| v.as_f64()) {
            agg.saps3_sum += v;
            agg.saps3_n += 1;
        }
        if let Some(v) = row.get("ultimo_news2_score").and_then(|v| v.as_f64()) {
            agg.news2_sum += v;
            agg.news2_n += 1;
        }
        // ── KPIs ejecutivos (5.3): egreso real, mortalidad predicha y LOS ──
        if let Some(egreso) = row.get("fecha_egreso_uci").and_then(|v| v.as_str())
            && !egreso.is_empty()
        {
            agg.egresados += 1;
            if let Some(dl) = row.get("desenlace_uci").and_then(|v| v.as_str())
                && dl.eq_ignore_ascii_case("Fallecido")
            {
                agg.fallecidos += 1;
            }
        }
        if let Some(egreso) = row.get("fecha_egreso_uci").and_then(|v| v.as_str()) {
            if !egreso.is_empty()
                && let Some(ingreso) = row.get("fecha_ingreso_uci").and_then(|v| v.as_str())
            {
                use chrono::DateTime;
                if let (Ok(e), Ok(i)) = (
                    DateTime::parse_from_rfc3339(egreso),
                    DateTime::parse_from_rfc3339(ingreso),
                ) {
                    let dias = (e - i).num_hours() as f64 / 24.0;
                    agg.los_dias_sum += dias.max(0.0);
                    agg.los_dias_n += 1;
                }
            }
            if let Some(m) = row.get("mortality_risk").and_then(|v| v.as_f64()) {
                agg.mortalidad_predicha_sum += m;
                agg.mortalidad_predicha_n += 1;
            }
        }
    }

    Ok(agg)
}

// ─── Patients ──────────────────────────────────────────────────────────────

/// SPEC-052: crea el paciente con la PHI cifrada en reposo.
///
/// Ya no se persiste el `Patient` completo: se sella en un envelope
/// AES-256-GCM (`phi`) y sólo quedan en claro las columnas de filtro. Ver
/// [`crate::phi_store`].
pub async fn create_patient(db: &Surreal<Db>, patient: Patient) -> Result<Patient> {
    let created = phi_store::save_patient(db, patient).await?;
    crate::metrics::patient_created();
    Ok(created)
}

pub async fn get_patient(db: &Surreal<Db>, id: &str) -> Result<Option<Patient>> {
    // `OMIT id`: ver nota en phi_store. `id` es un RecordId y no se puede
    // deserializar dentro de un serde_json::Value.
    let mut res = db
        .query("SELECT * OMIT id FROM $id")
        .bind(("id", sdb_id("patients", id)))
        .await?;
    let rows: Vec<serde_json::Value> = res.take(0)?;
    let row = rows.into_iter().next();
    phi_store::open_patient_opt(row)
}

/// Busca paciente por número de registro médico (historia clínica) o cédula.
///
/// SPEC-052: la PHI está cifrada, así que la coincidencia es **exacta** sobre
/// índices ciegos HMAC en vez de un `~` sobre el valor en claro. La
/// normalización (`MRN-001` = `mrn 001`) ocurre antes del HMAC, así que el
/// comportamiento de búsqueda se conserva sin exponer el identificador.
pub async fn get_patient_by_mrn(db: &Surreal<Db>, mrn: &str) -> Result<Option<Patient>> {
    let candidates = patients_by_identifier_any_tenant(db, mrn).await?;
    Ok(candidates.into_iter().next())
}

/// SPEC-025: variante **acotada al tenant** de [`get_patient_by_mrn`].
///
/// La referencia MRN/cédula (`PID-3`) de un mensaje HL7 la elige el emisor, así
/// que la ingesta autenticada debe resolverse siempre dentro del tenant del
/// emisor: sin este filtro, un `MRN` duplicado en otro hospital escribiría
/// signos vitales en la historia equivocada (o devolvería PHI ajena).
pub async fn get_patient_by_mrn_for_tenant(
    db: &Surreal<Db>,
    mrn: &str,
    tenant_id: &str,
) -> Result<Vec<Patient>> {
    phi_store::find_patients_by_identifier(db, tenant_id, mrn, 10).await
}

/// Búsqueda de MRN sin tenant, para los endpoints administrativos legacy.
///
/// Cada tenant se consulta por separado y con su propia clave de índice: el
/// índice ciego se calcula con el tenant como parte del HMAC, así que un mismo
/// MRN en dos hospitales produce valores distintos y se resuelven sin cruzarlos.
async fn patients_by_identifier_any_tenant(db: &Surreal<Db>, mrn: &str) -> Result<Vec<Patient>> {
    let mut out = Vec::new();
    for t in tenants_with_patients(db).await? {
        if let Ok(found) = phi_store::find_patients_by_identifier(db, &t, mrn, 1).await {
            out.extend(found);
        }
    }
    out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    out.truncate(1);
    Ok(out)
}

/// SPEC-052: re-sella la PHI del paciente con su contexto vigente.
pub async fn update_patient(
    db: &Surreal<Db>,
    id: &str,
    patient: Patient,
) -> Result<Option<Patient>> {
    phi_store::update_patient(db, id, patient).await
}

/// SPEC-052: crea el paciente y le asigna cama/equipos en una sola transacción
/// SurrealQL.
///
/// El `CONTENT` que se escribe es la fila ya sellada por
/// [`crate::phi_store::seal_patient`], no el `Patient` en claro: una escritura
/// que pase por alto la capa de cifrado reintroduciría PHI en el disco.
pub async fn create_patient_with_assignments(
    db: &Surreal<Db>,
    patient: Patient,
    equipos_ids: &[String],
) -> Result<Patient> {
    let patient_id = if patient.patient_id.is_empty() {
        Uuid::new_v4().to_string()
    } else {
        patient.patient_id.clone()
    };
    let cama_id = patient.cama_id.clone().unwrap_or_default();
    let paciente_nombre = patient.nombre_completo();

    let mut cama_existente: Option<dmart_shared::models::Cama> = None;
    if !cama_id.is_empty() {
        let cama = get_cama(db, &cama_id).await?;
        let cama = cama.ok_or_else(|| anyhow::anyhow!("Cama no encontrada"))?;
        if cama.estado != EstadoCama::Libre {
            return Err(anyhow::anyhow!("Cama no disponible o no encontrada"));
        }
        cama_existente = Some(cama);
    }

    // Se sella **antes** de abrir la transacción para que un fallo del cifrado
    // no deje una cama reservada sin paciente.
    let mut sealed_patient = patient.clone();
    sealed_patient.patient_id = patient_id.clone();
    let row = serde_json::to_value(phi_store::seal_patient(&sealed_patient)?)
        .context("serializando fila cifrada del paciente")?;

    // Pre-sella la cama con paciente_nombre en PHI.
    //
    // La fila se escribe con `CONTENT`, que **reemplaza el documento entero**,
    // así que hay que partir de la cama real: `..Default::default()` usaría
    // `Cama::new(1, General)` y pisaría `numero`, `tipo` y `created_at` de la
    // cama con valores neutros en cada ingreso.
    let tenant_id = std::env::var("DMART_TENANT_ID").unwrap_or_else(|_| "default".to_string());
    let mut cama_for_seal = cama_existente.unwrap_or_else(|| dmart_shared::models::Cama {
        cama_id: cama_id.clone(),
        ..Default::default()
    });
    cama_for_seal.cama_id = cama_id.clone();
    cama_for_seal.estado = EstadoCama::Ocupada;
    cama_for_seal.paciente_id = Some(patient_id.clone());
    cama_for_seal.paciente_nombre = Some(paciente_nombre.clone());
    let cama_row = phi_store::seal_cama(&cama_for_seal, &tenant_id)?;
    let cama_row_value = serde_json::to_value(&cama_row)?;

    let sql = r#"
        BEGIN TRANSACTION;
        IF string::len($cama_id) > 0 {
            LET $cama = (SELECT * FROM camas WHERE id = type::thing('camas', $cama_id) AND estado = 'Libre' LIMIT 1);
            IF array::len($cama) = 0 { THROW 'Cama no disponible o no encontrada'; };
            UPDATE type::thing('camas', $cama_id) CONTENT $cama_row;
            FOR $e in $equipos {
                UPDATE type::thing('equipos', $e) SET cama_id = $cama_id;
            };
        };
        CREATE type::thing('patients', $pid) CONTENT $row;
        COMMIT TRANSACTION;
    "#;

    let mut res = db
        .query(sql)
        .bind(("cama_id", cama_id))
        .bind(("pid", patient_id.clone()))
        .bind(("cama_row", cama_row_value))
        .bind(("equipos", equipos_ids.to_vec()))
        .bind(("row", row))
        .await?;

    if let Some((_, err)) = res.take_errors().into_iter().next() {
        return Err(anyhow::anyhow!("{}", err));
    }

    crate::metrics::patient_created();
    get_patient(db, &patient_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Failed to create patient"))
}

/// Libera la cama y sus equipos al egresar un paciente, en una sola
/// transacción SurrealQL. `desenlace` es el resultado clínico (`Mejorado`,
/// `Trasladado`, `Fallecido`), que se guarda en el paciente para poder
/// comparar la mortalidad **real** contra la **predicha**.
///
/// SPEC-052: los KPIs de egreso (`fecha_egreso_uci`, `desenlace_uci`) son
/// columnas en claro porque se agregan en SQL, pero también viven dentro del
/// envelope. Actualizar sólo la columna dejaría el envelope desactualizado y
/// el paciente devuelto por la API mostraría el estado anterior, así que tras
/// la transacción se re-sella el documento completo.
pub async fn egresar_paciente(db: &Surreal<Db>, patient: &Patient, desenlace: &str) -> Result<()> {
    let Some(cama_id) = &patient.cama_id else {
        return Ok(());
    };

    let tenant_id = std::env::var("DMART_TENANT_ID").unwrap_or_else(|_| "default".to_string());
    // Igual que en el ingreso, la fila se escribe con `CONTENT`: sin leer la
    // cama real, `..Default::default()` reiniciaría su `numero`, `tipo` y
    // `created_at` al liberar la cama.
    let cama_actual = get_cama(db, cama_id).await.ok().flatten();
    let mut cama_for_seal = cama_actual.unwrap_or_else(|| dmart_shared::models::Cama {
        cama_id: cama_id.clone(),
        ..Default::default()
    });
    cama_for_seal.cama_id = cama_id.clone();
    cama_for_seal.estado = EstadoCama::Libre;
    cama_for_seal.paciente_id = None;
    cama_for_seal.paciente_nombre = None;
    let cama_row = phi_store::seal_cama(&cama_for_seal, &tenant_id)?;
    let cama_row_value = serde_json::to_value(&cama_row)?;

    let sql = r#"
        BEGIN TRANSACTION;
        UPDATE type::table('equipos') SET cama_id = NONE WHERE cama_id = $cama_id;
        UPDATE type::thing('camas', $cama_id) CONTENT $cama_row;
        UPDATE type::thing('patients', $paciente_id)
            SET fecha_egreso_uci = time::now(),
                desenlace_uci = $desenlace;
        COMMIT TRANSACTION;
    "#;

    let mut res = db
        .query(sql)
        .bind(("cama_id", cama_id.clone()))
        .bind(("cama_row", cama_row_value))
        .bind(("paciente_id", patient.patient_id.clone()))
        .bind(("desenlace", desenlace.to_owned()))
        .await?;
    if let Some((_, err)) = res.take_errors().into_iter().next() {
        return Err(anyhow::anyhow!("{}", err));
    }

    // Re-sella el envelope con el estado de egreso que acaba de aplicar SurrealDB.
    if let Some(mut fresh) = get_patient(db, &patient.patient_id).await? {
        fresh.desenlace_uci = desenlace.to_owned();
        fresh.fecha_egreso_uci = chrono::Utc::now().to_rfc3339();
        fresh.updated_at = chrono::Utc::now().to_rfc3339();
        phi_store::update_patient(db, &patient.patient_id, fresh).await?;
    }
    Ok(())
}

/// Filtro por estado del paciente para listados y búsquedas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EstadoFilter {
    #[default]
    Activos,
    Egresados,
    Todos,
}

impl EstadoFilter {
    /// Interpreta el valor recibido por query string (`activos` por defecto).
    pub fn from_query(value: Option<&str>) -> Self {
        match value.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
            Some("egresados") | Some("egresado") => Self::Egresados,
            Some("todos") | Some("all") => Self::Todos,
            _ => Self::Activos,
        }
    }

    /// Fragmento SurrealQL que se anexa al final de una cláusula WHERE.
    /// Un paciente está egresado cuando `fecha_egreso_uci` tiene valor.
    fn sql(&self) -> &'static str {
        match self {
            Self::Activos => " AND (fecha_egreso_uci IS NONE OR fecha_egreso_uci = '')",
            Self::Egresados => " AND (fecha_egreso_uci IS NOT NONE AND fecha_egreso_uci != '')",
            Self::Todos => "",
        }
    }
}

pub async fn list_patients(db: &Surreal<Db>, limit: u32, offset: u32) -> Result<Vec<Patient>> {
    let limit = limit.min(dmart_shared::models::MAX_PAGE_LIMIT);
    let rows: Vec<serde_json::Value> = db
        .query("SELECT * OMIT id FROM patients ORDER BY created_at DESC LIMIT $limit START $offset")
        .bind(("limit", limit as i64))
        .bind(("offset", offset as i64))
        .await?
        .take(0)?;
    phi_store::open_patients(rows)
}

/// SPEC-025: listado de pacientes filtrado por tenant (RLS).
pub async fn list_patients_for_tenant(
    db: &Surreal<Db>,
    tenant_id: &str,
    estado: EstadoFilter,
    limit: u32,
    offset: u32,
) -> Result<Vec<Patient>> {
    let limit = limit.min(dmart_shared::models::MAX_PAGE_LIMIT);
    let sql = format!(
        "SELECT * OMIT id FROM patients WHERE tenant_id = $tenant{} ORDER BY created_at DESC LIMIT $limit START $offset",
        estado.sql()
    );
    let rows: Vec<serde_json::Value> = db
        .query(sql)
        .bind(("tenant", tenant_id.to_string()))
        .bind(("limit", limit as i64))
        .bind(("offset", offset as i64))
        .await?
        .take(0)?;
    phi_store::open_patients(rows)
}

pub async fn count_patients(db: &Surreal<Db>) -> Result<u64> {
    let count: Vec<serde_json::Value> = db
        .query("SELECT count() as count FROM patients GROUP BY count")
        .await?
        .take(0)?;
    Ok(count.first().and_then(|v| v["count"].as_u64()).unwrap_or(0))
}

/// SPEC-025: recuento de pacientes filtrado por tenant (RLS).
pub async fn count_patients_for_tenant(
    db: &Surreal<Db>,
    tenant_id: &str,
    estado: EstadoFilter,
) -> Result<u64> {
    let sql = format!(
        "SELECT count() as count FROM patients WHERE tenant_id = $tenant{} GROUP BY count",
        estado.sql()
    );
    let count: Vec<serde_json::Value> = db
        .query(sql)
        .bind(("tenant", tenant_id.to_string()))
        .await?
        .take(0)?;
    Ok(count.first().and_then(|v| v["count"].as_u64()).unwrap_or(0))
}

/// Búsqueda **parcial y tolerante a erratas** de pacientes, acotada al tenant.
///
/// Ya no es posible `nombre ~ $q`: la PHI está cifrada. Cada campo buscable
/// tiene su índice ciego HMAC y, además, los trigramas ciegos del nombre
/// completo, así que el filtro previo corre en la base sin exponer texto ni
/// escanear la colección. El ranking final es en Rust sobre los candidatos
/// descifrados (ver [`crate::search`]).
///
/// Menos de 3 caracteres no genera trigrama: en ese caso cae al camino exacto,
/// que aún encuentra historias clínicas y cédulas completas.
pub async fn search_patients_for_tenant(
    db: &Surreal<Db>,
    query: &str,
    tenant_id: &str,
    estado: EstadoFilter,
    limit: u32,
    offset: u32,
) -> Result<Vec<Patient>> {
    phi_store::search_patients_fuzzy(db, tenant_id, query, estado.sql(), limit, offset).await
}

/// SPEC-025: recuento de la búsqueda difusa acotada al tenant (RLS).
///
/// Cuenta sobre el mismo conjunto ordenado que devuelve el listado, para que
/// `total` y los elementos de la página no puedan discrepar.
pub async fn search_patients_count_for_tenant(
    db: &Surreal<Db>,
    query: &str,
    tenant_id: &str,
    estado: EstadoFilter,
) -> Result<u64> {
    phi_store::count_patients_fuzzy(db, tenant_id, query, estado.sql()).await
}

/// Búsqueda exacta de pacientes, conservada para callers que necesitan
/// coincidencia estricta (p. ej. resolver un código sin falsos positivos de
/// similitud).
pub async fn search_patients(
    db: &Surreal<Db>,
    query: &str,
    limit: u32,
    offset: u32,
) -> Result<Vec<Patient>> {
    search_patients_any_tenant(db, query, "", limit, offset).await
}

pub async fn search_patients_count(db: &Surreal<Db>, query: &str) -> Result<u64> {
    search_patients_any_tenant(db, query, "", 1000, 0)
        .await
        .map(|v| v.len() as u64)
}

/// Búsqueda exacta de pacientes acotada al tenant del solicitante.
///
/// Ya no se usa en el listado (ver [`search_patients_for_tenant`]), pero se
/// mantiene para los callers que necesitan coincidencia estricta.
pub async fn search_patients_exact_for_tenant(
    db: &Surreal<Db>,
    query: &str,
    tenant_id: &str,
    estado: EstadoFilter,
    limit: u32,
    offset: u32,
) -> Result<Vec<Patient>> {
    phi_store::search_patients_exact(db, tenant_id, query, estado.sql(), limit, offset).await
}

/// Recuento de la búsqueda exacta acotada al tenant.
pub async fn search_patients_exact_count_for_tenant(
    db: &Surreal<Db>,
    query: &str,
    tenant_id: &str,
    estado: EstadoFilter,
) -> Result<u64> {
    phi_store::count_patients_exact(db, tenant_id, query, estado.sql()).await
}

/// Búsqueda sin tenant para endpoints administrativos: cada tenant se consulta
/// con su propia clave de índice ciego.
async fn search_patients_any_tenant(
    db: &Surreal<Db>,
    query: &str,
    extra_where: &str,
    limit: u32,
    offset: u32,
) -> Result<Vec<Patient>> {
    let mut out = Vec::new();
    for t in tenants_with_patients(db).await? {
        if let Ok(found) =
            phi_store::search_patients_fuzzy(db, &t, query, extra_where, limit, offset).await
        {
            out.extend(found);
        }
    }
    // Cada tenant se pagina por separado, así que el recorte global se hace
    // después de unir: sin esto, `limit` se aplicaría por tenant.
    let limit = limit.min(dmart_shared::models::MAX_PAGE_LIMIT) as usize;
    Ok(out.into_iter().take(limit).collect())
}

/// Tenants que tienen al menos un paciente.
///
/// `SELECT DISTINCT` no es válido en SurrealQL 2.x; se usa `SELECT VALUE` y la
/// deduplicación se hace en Rust, donde ya se está recorriendo la lista.
async fn tenants_with_patients(db: &Surreal<Db>) -> Result<Vec<String>> {
    let values: Vec<String> = db
        .query("SELECT VALUE tenant_id FROM patients WHERE tenant_id != ''")
        .await?
        .take(0)?;
    let mut seen = Vec::new();
    for t in values {
        if !seen.contains(&t) {
            seen.push(t);
        }
    }
    Ok(seen)
}

pub async fn delete_patient(db: &Surreal<Db>, id: &str) -> Result<()> {
    // Se borra por query en vez de con `db.delete(..)`: el builder devuelve el
    // record eliminado y su `id` es un `Thing`, que no se puede deserializar
    // (ni interesa: la PHI se va con el registro).
    db.query("DELETE $id")
        .bind(("id", sdb_id("patients", id)))
        .await?;
    crate::metrics::patient_deleted();
    Ok(())
}

// ─── Measurements ──────────────────────────────────────────────────────────

pub async fn create_measurement(db: &Surreal<Db>, mut m: Measurement) -> Result<Measurement> {
    let patient_id = m.patient_id.clone();
    let measurement_id = m.measurement_id.clone();
    if measurement_id.is_empty() {
        m.measurement_id = Uuid::new_v4().to_string();
    }
    let row = phi_store::seal_measurement(&m)?;
    let created: Option<phi_store::MeasurementRow> = db
        .create(("measurements", row.measurement_id.clone()))
        .content(row)
        .await?;

    crate::metrics::measurement_created();

    if crate::cache::cache_available() {
        crate::cache::cache_del(&format!("measurements:{}", patient_id)).await;
        crate::cache::cache_del(&format!("last_measurement:{}", patient_id)).await;
    }

    created
        .map(|r| {
            phi_store::open_measurement(serde_json::to_value(r).expect("serialize"))
                .expect("open measurement")
        })
        .ok_or_else(|| anyhow::anyhow!("Failed to create measurement"))
}

pub async fn get_measurements_for_patient(
    db: &Surreal<Db>,
    patient_id: &str,
) -> Result<Vec<Measurement>> {
    let cache_key = format!("measurements:{}", patient_id);
    if crate::cache::cache_available()
        && let Some(cached) = crate::cache::cache_get(&cache_key).await
        && let Ok(measurements) = serde_json::from_str::<Vec<Measurement>>(&cached)
    {
        return Ok(measurements);
    }

    let pid = patient_id.to_string();
    let rows: Vec<Value> = db
        .query("SELECT * OMIT id FROM measurements WHERE patient_id = $pid ORDER BY timestamp DESC")
        .bind(("pid", pid))
        .await?
        .take(0)?;

    let measurements = phi_store::open_measurements(rows)?;

    if crate::cache::cache_available()
        && let Ok(json) = serde_json::to_string(&measurements)
    {
        crate::cache::cache_set(&cache_key, &json, 60).await;
    }

    Ok(measurements)
}

pub async fn get_all_measurements(db: &Surreal<Db>, patient_id: &str) -> Result<Vec<Measurement>> {
    let rows: Vec<Value> = db
        .query("SELECT * OMIT id FROM measurements WHERE patient_id = $pid ORDER BY timestamp DESC")
        .bind(("pid", patient_id.to_string()))
        .await?
        .take(0)?;
    phi_store::open_measurements(rows)
}

pub async fn get_last_measurement(
    db: &Surreal<Db>,
    patient_id: &str,
) -> Result<Option<Measurement>> {
    let cache_key = format!("last_measurement:{}", patient_id);
    if crate::cache::cache_available()
        && let Some(cached) = crate::cache::cache_get(&cache_key).await
        && let Ok(m) = serde_json::from_str::<Option<Measurement>>(&cached)
    {
        return Ok(m);
    }

    let pid = patient_id.to_string();
    let rows: Vec<Value> = db
        .query("SELECT * OMIT id FROM measurements WHERE patient_id = $pid ORDER BY timestamp DESC LIMIT 1")
        .bind(("pid", pid))
        .await?
        .take(0)?;
    let result = phi_store::open_measurements(rows)?.into_iter().next();

    if crate::cache::cache_available()
        && let Ok(json) = serde_json::to_string(&result)
    {
        crate::cache::cache_set(&cache_key, &json, 60).await;
    }

    Ok(result)
}

pub async fn get_measurement(
    db: &Surreal<Db>,
    measurement_id: &str,
) -> Result<Option<Measurement>> {
    let rows: Vec<Value> = db
        .query("SELECT * OMIT id FROM measurements WHERE measurement_id = $mid LIMIT 1")
        .bind(("mid", measurement_id.to_string()))
        .await?
        .take(0)?;
    let measurements = phi_store::open_measurements(rows)?;
    Ok(measurements.into_iter().next())
}

// ─── Camas ───────────────────────────────────────────────────────────

pub async fn init_camas(db: &Surreal<Db>, cantidad: u8, tipo: TipoCama) -> Result<Vec<Cama>> {
    let mut camas = Vec::new();
    let existentes = list_camas(db).await?;
    let start_num = existentes.iter().map(|c| c.numero).max().unwrap_or(0) + 1;
    for i in 0..cantidad {
        let cama = Cama::new(start_num + i, tipo.clone());
        let created: Option<Cama> = db
            .create(("camas", cama.cama_id.clone()))
            .content(cama.clone())
            .await?;
        if let Some(c) = created {
            camas.push(c);
        }
    }
    Ok(camas)
}

pub async fn create_cama(db: &Surreal<Db>, mut cama: Cama) -> Result<Cama> {
    if cama.cama_id.is_empty() {
        cama.cama_id = Uuid::new_v4().to_string();
    }
    let created: Option<Cama> = db
        .create(("camas", cama.cama_id.clone()))
        .content(cama)
        .await?;
    created.ok_or_else(|| anyhow::anyhow!("Failed to create cama"))
}

pub async fn get_cama(db: &Surreal<Db>, id: &str) -> Result<Option<Cama>> {
    let cama: Option<Cama> = db.select(("camas", id)).await?;
    Ok(cama)
}

pub async fn get_cama_by_numero(db: &Surreal<Db>, numero: u8) -> Result<Option<Cama>> {
    let camas: Vec<Cama> = db
        .query("SELECT * FROM camas WHERE numero = $numero LIMIT 1")
        .bind(("numero", numero))
        .await?
        .take(0)?;
    Ok(camas.into_iter().next())
}

pub async fn update_cama(db: &Surreal<Db>, id: &str, cama: Cama) -> Result<Option<Cama>> {
    let updated: Option<Cama> = db.update(("camas", id)).content(cama).await?;
    Ok(updated)
}

pub async fn list_camas(db: &Surreal<Db>) -> Result<Vec<Cama>> {
    let camas: Vec<Cama> = db.select("camas").await?;
    Ok(camas)
}

pub async fn list_camas_paginated(db: &Surreal<Db>, limit: u32, offset: u32) -> Result<Vec<Cama>> {
    let limit = limit.min(dmart_shared::models::MAX_PAGE_LIMIT);
    let camas: Vec<Cama> = db
        .query("SELECT * FROM camas ORDER BY numero ASC LIMIT $limit START $offset")
        .bind(("limit", limit as i64))
        .bind(("offset", offset as i64))
        .await?
        .take(0)?;
    Ok(camas)
}

pub async fn count_camas(db: &Surreal<Db>) -> Result<u64> {
    let count: Vec<serde_json::Value> = db
        .query("SELECT count() as count FROM camas GROUP BY count")
        .await?
        .take(0)?;
    Ok(count.first().and_then(|v| v["count"].as_u64()).unwrap_or(0))
}

pub async fn get_cama_libre(db: &Surreal<Db>) -> Result<Option<Cama>> {
    let estado = format!("{:?}", EstadoCama::Libre);
    let camas: Vec<Cama> = db
        .query("SELECT * FROM camas WHERE estado = $estado LIMIT 1")
        .bind(("estado", estado))
        .await?
        .take(0)?;
    Ok(camas.into_iter().next())
}

pub async fn count_camas_por_tipo(
    db: &Surreal<Db>,
) -> Result<Vec<dmart_shared::models::TipoCamaCount>> {
    let estado = format!("{:?}", EstadoCama::Libre);

    let totales: Vec<serde_json::Value> = db
        .query("SELECT tipo, count() AS total FROM camas GROUP BY tipo")
        .await?
        .take(0)?;
    let libres: Vec<serde_json::Value> = db
        .query("SELECT tipo, count() AS libres FROM camas WHERE estado = $estado GROUP BY tipo")
        .bind(("estado", estado))
        .await?
        .take(0)?;

    let mut map: std::collections::HashMap<String, (u8, u8)> = std::collections::HashMap::new();
    for row in totales {
        if let (Some(tipo), Some(total)) = (
            row.get("tipo").and_then(|v| v.as_str()),
            row.get("total").and_then(|v| v.as_u64()),
        ) {
            map.insert(tipo.to_string(), (total as u8, 0));
        }
    }
    for row in libres {
        if let (Some(tipo), Some(libres)) = (
            row.get("tipo").and_then(|v| v.as_str()),
            row.get("libres").and_then(|v| v.as_u64()),
        ) {
            map.entry(tipo.to_string()).or_insert((0, 0)).1 = libres as u8;
        }
    }

    Ok(map
        .into_iter()
        .map(
            |(tipo, (total, libres))| dmart_shared::models::TipoCamaCount {
                tipo: tipo_cama_label(&tipo).to_string(),
                total,
                libres,
            },
        )
        .collect())
}

fn tipo_cama_label(stored: &str) -> &'static str {
    match stored {
        "General" => TipoCama::General.label(),
        "Aislamiento" => TipoCama::Aislamiento.label(),
        "Pediatrica" => TipoCama::Pediatrica.label(),
        "Coronaria" => TipoCama::Coronaria.label(),
        "Quemados" => TipoCama::Quemados.label(),
        _ => TipoCama::Otro.label(),
    }
}

pub async fn asignar_cama_paciente(
    db: &Surreal<Db>,
    cama_id: &str,
    paciente_id: &str,
    paciente_nombre: &str,
) -> Result<Option<Cama>> {
    let cama: Option<Cama> = db.select(("camas", cama_id)).await?;
    if let Some(mut c) = cama {
        if !c.estado.puede_asignar() {
            return Err(anyhow::anyhow!("Cama no disponible"));
        }
        c.estado = EstadoCama::Ocupada;
        c.paciente_id = Some(paciente_id.to_string());
        c.paciente_nombre = Some(paciente_nombre.to_string());
        let updated: Option<Cama> = db.update(("camas", cama_id)).content(c).await?;
        Ok(updated)
    } else {
        Err(anyhow::anyhow!("Cama not found"))
    }
}

pub async fn liberar_cama(db: &Surreal<Db>, cama_id: &str) -> Result<Option<Cama>> {
    let cama: Option<Cama> = db.select(("camas", cama_id)).await?;
    if let Some(mut c) = cama {
        c.estado = EstadoCama::Libre;
        c.paciente_id = None;
        c.paciente_nombre = None;
        let updated: Option<Cama> = db.update(("camas", cama_id)).content(c).await?;
        Ok(updated)
    } else {
        Err(anyhow::anyhow!("Cama not found"))
    }
}

pub async fn delete_cama(db: &Surreal<Db>, id: &str) -> Result<()> {
    let _: Option<Cama> = db.delete(("camas", id)).await?;
    Ok(())
}

// ─── Equipos ──────────────────────────────────────────────────────────

pub async fn create_equipo(db: &Surreal<Db>, mut equipo: Equipo) -> Result<Equipo> {
    if equipo.equipo_id.is_empty() {
        equipo.equipo_id = Uuid::new_v4().to_string();
    }
    let created: Option<Equipo> = db
        .create(("equipos", equipo.equipo_id.clone()))
        .content(equipo)
        .await?;
    created.ok_or_else(|| anyhow::anyhow!("Failed to create equipo"))
}

pub async fn get_equipo(db: &Surreal<Db>, id: &str) -> Result<Option<Equipo>> {
    let equipo: Option<Equipo> = db.select(("equipos", id)).await?;
    Ok(equipo)
}

pub async fn update_equipo(db: &Surreal<Db>, id: &str, equipo: Equipo) -> Result<Option<Equipo>> {
    let updated: Option<Equipo> = db.update(("equipos", id)).content(equipo).await?;
    Ok(updated)
}

pub async fn list_equipos(db: &Surreal<Db>) -> Result<Vec<Equipo>> {
    let equipos: Vec<Equipo> = db.select("equipos").await?;
    Ok(equipos)
}

pub async fn list_equipos_paginated(
    db: &Surreal<Db>,
    limit: u32,
    offset: u32,
) -> Result<Vec<Equipo>> {
    let limit = limit.min(dmart_shared::models::MAX_PAGE_LIMIT);
    let equipos: Vec<Equipo> = db
        .query("SELECT * FROM equipos ORDER BY created_at DESC LIMIT $limit START $offset")
        .bind(("limit", limit as i64))
        .bind(("offset", offset as i64))
        .await?
        .take(0)?;
    Ok(equipos)
}

pub async fn count_equipos(db: &Surreal<Db>) -> Result<u64> {
    let count: Vec<serde_json::Value> = db
        .query("SELECT count() as count FROM equipos GROUP BY count")
        .await?
        .take(0)?;
    Ok(count.first().and_then(|v| v["count"].as_u64()).unwrap_or(0))
}

pub async fn list_equipos_por_cama(db: &Surreal<Db>, cama_id: &str) -> Result<Vec<Equipo>> {
    let cama = cama_id.to_string();
    let equipos: Vec<Equipo> = db
        .query("SELECT * FROM equipos WHERE cama_id = $cama")
        .bind(("cama", cama))
        .await?
        .take(0)?;
    Ok(equipos)
}

pub async fn asignar_equipo_cama(
    db: &Surreal<Db>,
    equipo_id: &str,
    cama_id: &str,
) -> Result<Option<Equipo>> {
    let equipo: Option<Equipo> = db.select(("equipos", equipo_id)).await?;
    if let Some(mut e) = equipo {
        e.cama_id = Some(cama_id.to_string());
        let updated: Option<Equipo> = db.update(("equipos", equipo_id)).content(e).await?;
        Ok(updated)
    } else {
        Err(anyhow::anyhow!("Equipo not found"))
    }
}

pub async fn desvincular_equipo_cama(db: &Surreal<Db>, equipo_id: &str) -> Result<Option<Equipo>> {
    let equipo: Option<Equipo> = db.select(("equipos", equipo_id)).await?;
    if let Some(mut e) = equipo {
        e.cama_id = None;
        let updated: Option<Equipo> = db.update(("equipos", equipo_id)).content(e).await?;
        Ok(updated)
    } else {
        Err(anyhow::anyhow!("Equipo not found"))
    }
}

pub async fn list_equipos_disponibles(db: &Surreal<Db>) -> Result<Vec<Equipo>> {
    let estado = format!("{:?}", EstadoEquipo::Activo);
    let equipos: Vec<Equipo> = db
        .query("SELECT * FROM equipos WHERE estado = $estado AND cama_id = NONE")
        .bind(("estado", estado))
        .await?
        .take(0)?;
    Ok(equipos)
}

pub async fn count_equipos_por_tipo(
    db: &Surreal<Db>,
) -> Result<Vec<dmart_shared::models::EquipoTipoCount>> {
    let estado = format!("{:?}", EstadoEquipo::Activo);

    let totales: Vec<serde_json::Value> = db
        .query("SELECT tipo, count() AS total FROM equipos GROUP BY tipo")
        .await?
        .take(0)?;
    let disponibles: Vec<serde_json::Value> = db
        .query("SELECT tipo, count() AS disponibles FROM equipos WHERE estado = $estado AND cama_id = NONE GROUP BY tipo")
        .bind(("estado", estado))
        .await?
        .take(0)?;

    let mut map: std::collections::HashMap<String, (u32, u32)> = std::collections::HashMap::new();
    for row in totales {
        if let (Some(tipo), Some(total)) = (
            row.get("tipo").and_then(|v| v.as_str()),
            row.get("total").and_then(|v| v.as_u64()),
        ) {
            map.insert(tipo.to_string(), (total as u32, 0));
        }
    }
    for row in disponibles {
        if let (Some(tipo), Some(d)) = (
            row.get("tipo").and_then(|v| v.as_str()),
            row.get("disponibles").and_then(|v| v.as_u64()),
        ) {
            map.entry(tipo.to_string()).or_insert((0, 0)).1 = d as u32;
        }
    }

    Ok(map
        .into_iter()
        .map(
            |(tipo, (total, disponibles))| dmart_shared::models::EquipoTipoCount {
                tipo: tipo_equipo_label(&tipo).to_string(),
                total,
                disponibles,
            },
        )
        .collect())
}

fn tipo_equipo_label(stored: &str) -> &'static str {
    match stored {
        "VentiladorMecanico" => TipoEquipo::VentiladorMecanico.label(),
        "Monitor" => TipoEquipo::Monitor.label(),
        "Computador" => TipoEquipo::Computador.label(),
        "BombaInfusion" => TipoEquipo::BombaInfusion.label(),
        _ => TipoEquipo::Otro.label(),
    }
}

pub async fn delete_equipo(db: &Surreal<Db>, id: &str) -> Result<()> {
    let _: Option<Equipo> = db.delete(("equipos", id)).await?;
    Ok(())
}

pub async fn liberar_equipos_de_cama(db: &Surreal<Db>, cama_id: &str) -> Result<()> {
    let equipos = list_equipos_por_cama(db, cama_id).await?;
    for e in equipos {
        desvincular_equipo_cama(db, &e.equipo_id).await?;
    }
    Ok(())
}

// ─── Users (Staff) ─────────────────────────────────────────────────

pub async fn create_user(db: &Surreal<Db>, mut user: User) -> Result<User> {
    if user.user_id.is_empty() {
        user.user_id = Uuid::new_v4().to_string();
    }
    let created: Option<User> = db
        .create(("users", user.user_id.clone()))
        .content(user)
        .await?;
    created.ok_or_else(|| anyhow::anyhow!("Failed to create user"))
}

pub async fn get_user(db: &Surreal<Db>, id: &str) -> Result<Option<User>> {
    let user: Option<User> = db.select(("users", id)).await?;
    Ok(user)
}

pub async fn get_user_by_username(db: &Surreal<Db>, username: &str) -> Result<Option<User>> {
    let users: Vec<User> = db
        .query("SELECT * FROM users WHERE username = $username LIMIT 1")
        .bind(("username", username.to_string()))
        .await?
        .take(0)?;
    Ok(users.into_iter().next())
}

pub async fn update_user(db: &Surreal<Db>, id: &str, user: User) -> Result<Option<User>> {
    let updated: Option<User> = db.update(("users", id)).content(user).await?;
    Ok(updated)
}

pub async fn list_users(db: &Surreal<Db>) -> Result<Vec<User>> {
    let users: Vec<User> = db.select("users").await?;
    Ok(users)
}

/// Lista el personal registrado. `rol` es el nombre canónico (`Admin`, `Medico`,
/// `Enfermero`, `Viewer`) o `None` para incluir todos los roles.
pub async fn list_staff(db: &Surreal<Db>, rol: Option<&str>) -> Result<Vec<User>> {
    let staff: Vec<User> = match rol {
        Some(rol) => db
            .query("SELECT * FROM users WHERE rol = $rol ORDER BY created_at DESC")
            .bind(("rol", rol.to_string()))
            .await?
            .take(0)?,
        None => db
            .query("SELECT * FROM users ORDER BY created_at DESC")
            .await?
            .take(0)?,
    };
    Ok(staff)
}

pub async fn list_staff_paginated(
    db: &Surreal<Db>,
    limit: u32,
    offset: u32,
    rol: Option<&str>,
) -> Result<Vec<User>> {
    let limit = limit.min(dmart_shared::models::MAX_PAGE_LIMIT);
    let staff: Vec<User> = match rol {
        Some(rol) => {
            db.query("SELECT * FROM users WHERE rol = $rol ORDER BY created_at DESC LIMIT $limit START $offset")
                .bind(("rol", rol.to_string()))
                .bind(("limit", limit as i64))
                .bind(("offset", offset as i64))
                .await?
                .take(0)?
        }
        None => {
            db.query("SELECT * FROM users ORDER BY created_at DESC LIMIT $limit START $offset")
                .bind(("limit", limit as i64))
                .bind(("offset", offset as i64))
                .await?
                .take(0)?
        }
    };
    Ok(staff)
}

/// Cuenta el personal. `rol` es el nombre canónico o `None` para todos los roles.
pub async fn count_staff(db: &Surreal<Db>, rol: Option<&str>) -> Result<u64> {
    let count: Vec<serde_json::Value> = match rol {
        Some(rol) => db
            .query("SELECT count() as count FROM users WHERE rol = $rol GROUP BY count")
            .bind(("rol", rol.to_string()))
            .await?
            .take(0)?,
        None => db
            .query("SELECT count() as count FROM users GROUP BY count")
            .await?
            .take(0)?,
    };
    Ok(count.first().and_then(|v| v["count"].as_u64()).unwrap_or(0))
}

pub async fn delete_user(db: &Surreal<Db>, id: &str) -> Result<()> {
    let _: Option<User> = db.delete(("users", id)).await?;
    Ok(())
}

// ─── Institucion Config ────────────────────────────────────────────────

pub async fn get_institucion_config(db: &Surreal<Db>) -> Result<Option<InstitucionConfig>> {
    let configs: Vec<InstitucionConfig> = db.select("institucion_config").await?;
    Ok(configs.into_iter().next())
}

pub async fn upsert_institucion_config(
    db: &Surreal<Db>,
    config: InstitucionConfig,
) -> Result<InstitucionConfig> {
    let existing = get_institucion_config(db).await?;
    let mut c = config;
    if let Some(existing) = existing {
        c.config_id = existing.config_id.clone();
        let updated: Option<InstitucionConfig> = db
            .update(("institucion_config", c.config_id.clone()))
            .content(c)
            .await?;
        updated.ok_or_else(|| anyhow::anyhow!("Failed to update institucion config"))
    } else {
        if c.config_id.is_empty() {
            c.config_id = Uuid::new_v4().to_string();
        }
        let created: Option<InstitucionConfig> = db
            .create(("institucion_config", c.config_id.clone()))
            .content(c)
            .await?;
        created.ok_or_else(|| anyhow::anyhow!("Failed to create institucion config"))
    }
}

pub async fn seed_institucion_config(db: &Surreal<Db>) -> Result<()> {
    let existing = get_institucion_config(db).await?;
    if existing.is_some() {
        return Ok(());
    }
    let default_config = InstitucionConfig::default_config();
    upsert_institucion_config(db, default_config).await?;
    tracing::info!("🏥 Seeded default institution config");
    Ok(())
}

// ─── Auditoría de aislamiento multi-tenant (SPEC-025) ─────────────────────

/// Resultado de la auditoría para una tabla de datos con `tenant_id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableTenancyAudit {
    pub table: String,
    pub total: u64,
    /// Registros con `tenant_id` ausente (NONE) o vacío — fugas de aislamiento.
    pub missing_tenant_id: u64,
    /// Registros cuyo `tenant_id` no existe en el catálogo `tenant` ni es el default.
    pub orphan_tenant_ids: Vec<String>,
    pub orphan_count: u64,
}

/// Reporte global de la auditoría de tenants.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenancyAuditReport {
    pub healthy: bool,
    pub tables: Vec<TableTenancyAudit>,
    pub total_records: u64,
    /// Suma de registros con tenant ausente/o huérfano (riesgo de fuga cruzada).
    pub records_at_risk: u64,
}

/// Auditoría SEC-025: escanea pacientes, mediciones, usuarios y embeddings
/// buscando registros sin `tenant_id` o con `tenant_id` no registrado.
/// Devuelve filas problemáticas agrupadas por tabla (útil para un comando
/// de soporte `GET /admin/tenants/audit`).
pub async fn audit_tenancy(db: &Surreal<Db>) -> Result<TenancyAuditReport> {
    // Conjunto de tenants válidos: slugs registrados + el tenant por defecto
    // (siempre válido; prevalece en single-tenant legacy).
    let mut valid: std::collections::HashSet<String> = std::collections::HashSet::new();
    valid.insert(crate::tenant::default_tenant());
    let known: Vec<String> = db.query("SELECT VALUE slug FROM tenant").await?.take(0)?;
    for slug in &known {
        valid.insert(slug.clone());
    }

    const TABLES: [&str; 4] = ["patients", "measurements", "users", "patient_embedding"];

    // Lista de tenants válidos (usada en la query de huérfanos).
    let valid_list: Vec<String> = valid.iter().cloned().collect();

    let mut tables = Vec::with_capacity(TABLES.len());
    let mut total_records = 0u64;
    let mut records_at_risk = 0u64;

    for table in TABLES {
        // Total de registros.
        let total = count_all(db, table).await?;
        total_records += total;

        // Ausentes/vacíos.
        let missing = count_tenant_missing(db, table).await?;

        // Distinct tenant_id presentes en la tabla (excluye NONE/vacío).
        let present: Vec<serde_json::Value> = db
            .query(format!(
                "SELECT tenant_id FROM {table} WHERE tenant_id IS NOT NONE AND tenant_id != '' GROUP BY tenant_id"
            ))
            .await?
            .take(0)?;
        let mut orphans: Vec<String> = Vec::new();
        for v in &present {
            let Some(tid) = v.get("tenant_id").and_then(|t| t.as_str()) else {
                continue;
            };
            if !valid.contains(tid) {
                orphans.push(tid.to_string());
            }
        }

        // Registros con tenant_id huérfano (no en el catálogo ni default).
        let orphan_count = if orphans.is_empty() {
            0
        } else {
            let count: Vec<serde_json::Value> = db
                .query(format!(
                    "SELECT count() as count FROM {table} \
                     WHERE tenant_id IS NOT NONE AND tenant_id != '' AND tenant_id NOT IN $valid \
                     GROUP BY count"
                ))
                .bind(("valid", valid_list.clone()))
                .await?
                .take(0)?;
            count.first().and_then(|v| v["count"].as_u64()).unwrap_or(0)
        };

        records_at_risk += missing + orphan_count;
        tables.push(TableTenancyAudit {
            table: table.to_string(),
            total,
            missing_tenant_id: missing,
            orphan_tenant_ids: orphans,
            orphan_count,
        });
    }

    Ok(TenancyAuditReport {
        healthy: records_at_risk == 0,
        tables,
        total_records,
        records_at_risk,
    })
}

async fn count_all(db: &Surreal<Db>, table: &str) -> Result<u64> {
    let count: Vec<serde_json::Value> = db
        .query(format!(
            "SELECT count() as count FROM {table} GROUP BY count"
        ))
        .await?
        .take(0)?;
    Ok(count.first().and_then(|v| v["count"].as_u64()).unwrap_or(0))
}

async fn count_tenant_missing(db: &Surreal<Db>, table: &str) -> Result<u64> {
    let count: Vec<serde_json::Value> = db
        .query(format!(
            "SELECT count() as count FROM {table} WHERE tenant_id IS NONE OR tenant_id = '' GROUP BY count"
        ))
        .await?
        .take(0)?;
    Ok(count.first().and_then(|v| v["count"].as_u64()).unwrap_or(0))
}

// ─── Feature Flags ──────────────────────────────────────────────────────

use dmart_shared::models::{CreateFeatureFlagRequest, FeatureFlag, UpdateFeatureFlagRequest};

/// Crea una nueva feature flag.
pub async fn create_feature_flag(
    db: &Surreal<Db>,
    req: CreateFeatureFlagRequest,
) -> Result<FeatureFlag> {
    let now = chrono::Utc::now().to_rfc3339();
    let flag = FeatureFlag {
        key: req.key.clone(),
        description: req.description,
        enabled: req.enabled,
        tenant_id: req.tenant_id,
        created_at: now.clone(),
        updated_at: now,
    };

    let created: Option<FeatureFlag> = db
        .query("CREATE type::thing('feature_flag', $key) CONTENT $data RETURN AFTER")
        .bind(("key", req.key))
        .bind(("data", flag))
        .await?
        .take(0)?;

    created.ok_or_else(|| anyhow::anyhow!("failed to create feature flag"))
}

/// Lista todas las feature flags (opcionalmente filtradas por tenant).
pub async fn list_feature_flags(
    db: &Surreal<Db>,
    tenant_id: Option<String>,
) -> Result<Vec<FeatureFlag>> {
    let flags: Vec<FeatureFlag> = if let Some(tid) = tenant_id {
        db.query(
            "SELECT * FROM feature_flag WHERE tenant_id = $tid OR tenant_id IS NONE ORDER BY key",
        )
        .bind(("tid", tid))
        .await?
        .take(0)?
    } else {
        db.query("SELECT * FROM feature_flag ORDER BY key")
            .await?
            .take(0)?
    };
    Ok(flags)
}

/// Obtiene una feature flag por key y tenant.
pub async fn get_feature_flag(
    db: &Surreal<Db>,
    key: &str,
    tenant_id: Option<String>,
) -> Result<Option<FeatureFlag>> {
    let flag: Option<FeatureFlag> = if let Some(tid) = tenant_id {
        db.query(
            "SELECT * FROM feature_flag WHERE key = $key AND (tenant_id = $tid OR tenant_id IS NONE) ORDER BY tenant_id DESC LIMIT 1"
        )
        .bind(("key", key.to_string()))
        .bind(("tid", tid))
        .await?
        .take(0)?
    } else {
        db.query("SELECT * FROM feature_flag WHERE key = $key AND tenant_id IS NONE LIMIT 1")
            .bind(("key", key.to_string()))
            .await?
            .take(0)?
    };
    Ok(flag)
}

/// Actualiza una feature flag.
pub async fn update_feature_flag(
    db: &Surreal<Db>,
    key: &str,
    req: UpdateFeatureFlagRequest,
) -> Result<FeatureFlag> {
    let now = chrono::Utc::now().to_rfc3339();

    // Build update query dynamically based on provided fields
    let mut query = "UPDATE type::thing('feature_flag', $key) SET updated_at = $now".to_string();
    if req.description.is_some() {
        query.push_str(", description = $description");
    }
    if req.enabled.is_some() {
        query.push_str(", enabled = $enabled");
    }
    if req.tenant_id.is_some() {
        query.push_str(", tenant_id = $tenant_id");
    }
    query.push_str(" RETURN AFTER");

    let mut q = db
        .query(query)
        .bind(("key", key.to_string()))
        .bind(("now", now));

    if let Some(desc) = req.description {
        q = q.bind(("description", desc));
    }
    if let Some(enabled) = req.enabled {
        q = q.bind(("enabled", enabled));
    }
    if let Some(tid) = req.tenant_id {
        q = q.bind(("tenant_id", tid));
    }

    let updated: Option<FeatureFlag> = q.await?.take(0)?;
    updated.ok_or_else(|| anyhow::anyhow!("feature flag not found"))
}

/// Elimina una feature flag.
pub async fn delete_feature_flag(db: &Surreal<Db>, key: &str) -> Result<()> {
    let deleted: Option<FeatureFlag> = db.delete(("feature_flag", key)).await?;
    if deleted.is_none() {
        return Err(anyhow::anyhow!("feature flag not found"));
    }
    Ok(())
}

/// Evalúa una feature flag para un tenant (con cache TTL).
/// Devuelve true si la flag está enabled para el tenant, false en caso contrario (fail-closed).
pub async fn evaluate_feature_flag(
    db: &Surreal<Db>,
    key: &str,
    tenant_id: Option<String>,
) -> Result<bool> {
    let flag = get_feature_flag(db, key, tenant_id).await?;
    Ok(flag.map(|f| f.enabled).unwrap_or(false))
}
