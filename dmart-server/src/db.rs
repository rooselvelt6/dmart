#![allow(dead_code)]

use anyhow::Result;
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
    let mut agg = PatientAggregates::default();

    let groups: Vec<serde_json::Value> = db
        .query("SELECT estado_gravedad, count() AS n FROM patients GROUP BY estado_gravedad")
        .await?
        .take(0)?;
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

    let scores: Vec<serde_json::Value> = db
        .query(
            "SELECT ultimo_apache_score, ultimo_gcs_score, ultimo_sofa_score, ultimo_saps3_score, ultimo_news2_score, fecha_egreso_uci, fecha_ingreso_uci, desenlace_uci, mortality_risk FROM patients",
        )
        .await?
        .take(0)?;

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

pub async fn create_patient(db: &Surreal<Db>, mut patient: Patient) -> Result<Patient> {
    let patient_id = patient.patient_id.clone();
    if patient_id.is_empty() {
        patient.patient_id = Uuid::new_v4().to_string();
    }
    let created: Option<Patient> = db
        .create(("patients", patient.patient_id.clone()))
        .content(patient)
        .await?;
    created.ok_or_else(|| anyhow::anyhow!("Failed to create patient"))
}

pub async fn get_patient(db: &Surreal<Db>, id: &str) -> Result<Option<Patient>> {
    let patient: Option<Patient> = db.select(("patients", id)).await?;
    Ok(patient)
}

/// Busca paciente por número de registro médico (historia clínica) o cédula.
/// Usado por la integración HL7 (PID-3) para vincular monitores a pacientes.
pub async fn get_patient_by_mrn(db: &Surreal<Db>, mrn: &str) -> Result<Option<Patient>> {
    let q = format!("%{}%", mrn.trim());
    let patients: Vec<Patient> = db
        .query(
            "SELECT * FROM patients WHERE historia_clinica = $mrn OR cedula = $mrn \
             ORDER BY created_at DESC LIMIT 1",
        )
        .bind(("mrn", mrn.trim().to_string()))
        .await?
        .take(0)?;
    if patients.is_empty() {
        // búsqueda con comodines por si trae espacios/guiones distintos
        let patients: Vec<Patient> = db
            .query(
                "SELECT * FROM patients WHERE historia_clinica ~= $q OR cedula ~= $q \
                 ORDER BY created_at DESC LIMIT 1",
            )
            .bind(("q", q))
            .await?
            .take(0)?;
        return Ok(patients.into_iter().next());
    }
    Ok(patients.into_iter().next())
}

/// Crea el paciente asignando cama y equipos dentro de una única transacción
/// SurrealQL. Si cualquier paso falla (cama ocupada, error de escritura), toda
/// la transacción se revierte y no quedan camas ni equipos huérfanos.
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

    if !cama_id.is_empty() {
        let cama = get_cama(db, &cama_id).await?;
        let cama = cama.ok_or_else(|| anyhow::anyhow!("Cama no encontrada"))?;
        if cama.estado != EstadoCama::Libre {
            return Err(anyhow::anyhow!("Cama no disponible o no encontrada"));
        }
    }

    let sql = r#"
        BEGIN TRANSACTION;
        IF string::len($cama_id) > 0 {
            LET $cama = (SELECT * FROM camas WHERE id = type::thing('camas', $cama_id) AND estado = 'Libre' LIMIT 1);
            IF array::len($cama) = 0 { THROW 'Cama no disponible o no encontrada'; };
            UPDATE type::thing('camas', $cama_id) SET estado = 'Ocupada', paciente_id = $pid, paciente_nombre = $pnombre;
            FOR $e in $equipos {
                UPDATE type::thing('equipos', $e) SET cama_id = $cama_id;
            };
        };
LET $created = CREATE type::thing('patients', $pid) CONTENT $patient RETURN AFTER;
        COMMIT TRANSACTION;
        RETURN $created;
    "#;

    let mut res = db
        .query(sql)
        .bind(("cama_id", cama_id))
        .bind(("pid", patient_id.clone()))
        .bind(("pnombre", paciente_nombre))
        .bind(("equipos", equipos_ids.to_vec()))
        .bind(("patient", patient))
        .await?;

    if let Some((_, err)) = res.take_errors().into_iter().next() {
        return Err(anyhow::anyhow!("{}", err));
    }

    get_patient(db, &patient_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Failed to create patient"))
}

/// Libera la cama y sus equipos al egresar un paciente, en una sola
/// transacción SurrealQL. `desenlace` es el resultado clínico (`Mejorado`,
/// `Trasladado`, `Fallecido`), que se guarda en el paciente para poder
/// comparar la mortalidad **real** contra la **predicha**.
pub async fn egresar_paciente(db: &Surreal<Db>, patient: &Patient, desenlace: &str) -> Result<()> {
    let Some(cama_id) = &patient.cama_id else {
        return Ok(());
    };

    let sql = r#"
        BEGIN TRANSACTION;
        UPDATE type::table('equipos') SET cama_id = NONE WHERE cama_id = $cama_id;
        UPDATE type::thing('camas', $cama_id) SET estado = 'Libre', paciente_id = NONE, paciente_nombre = NONE;
        UPDATE type::thing('patients', $paciente_id)
            SET fecha_egreso_uci = time::now(),
                desenlace_uci = $desenlace;
        COMMIT TRANSACTION;
    "#;

    let mut res = db
        .query(sql)
        .bind(("cama_id", cama_id.clone()))
        .bind(("paciente_id", patient.patient_id.clone()))
        .bind(("desenlace", desenlace.to_owned()))
        .await?;
    if let Some((_, err)) = res.take_errors().into_iter().next() {
        return Err(anyhow::anyhow!("{}", err));
    }
    Ok(())
}

pub async fn update_patient(
    db: &Surreal<Db>,
    id: &str,
    patient: Patient,
) -> Result<Option<Patient>> {
    let updated: Option<Patient> = db.update(("patients", id)).content(patient).await?;
    Ok(updated)
}

pub async fn list_patients(db: &Surreal<Db>, limit: u32, offset: u32) -> Result<Vec<Patient>> {
    let patients: Vec<Patient> = db
        .query("SELECT * FROM patients ORDER BY created_at DESC LIMIT $limit START $offset")
        .bind(("limit", limit as i64))
        .bind(("offset", offset as i64))
        .await?
        .take(0)?;
    Ok(patients)
}

pub async fn count_patients(db: &Surreal<Db>) -> Result<u64> {
    let count: Vec<serde_json::Value> = db
        .query("SELECT count() as count FROM patients GROUP BY count")
        .await?
        .take(0)?;
    Ok(count.first().and_then(|v| v["count"].as_u64()).unwrap_or(0))
}

pub async fn search_patients(
    db: &Surreal<Db>,
    query: &str,
    limit: u32,
    offset: u32,
) -> Result<Vec<Patient>> {
    let q = format!("%{}%", query);
    let patients: Vec<Patient> = db
        .query("SELECT * FROM patients WHERE nombre ~= $q OR apellido ~= $q OR cedula ~= $q OR historia_clinica ~= $q ORDER BY created_at DESC LIMIT $limit START $offset")
        .bind(("q", q))
        .bind(("limit", limit as i64))
        .bind(("offset", offset as i64))
        .await?
        .take(0)?;
    Ok(patients)
}

pub async fn search_patients_count(db: &Surreal<Db>, query: &str) -> Result<u64> {
    let q = format!("%{}%", query);
    let count: Vec<serde_json::Value> = db
        .query("SELECT count() as count FROM patients WHERE nombre ~= $q OR apellido ~= $q OR cedula ~= $q OR historia_clinica ~= $q GROUP BY count")
        .bind(("q", q))
        .await?
        .take(0)?;
    Ok(count.first().and_then(|v| v["count"].as_u64()).unwrap_or(0))
}

pub async fn delete_patient(db: &Surreal<Db>, id: &str) -> Result<()> {
    let _: Option<Patient> = db.delete(("patients", id)).await?;
    Ok(())
}

// ─── Measurements ──────────────────────────────────────────────────────────

pub async fn create_measurement(db: &Surreal<Db>, mut m: Measurement) -> Result<Measurement> {
    let patient_id = m.patient_id.clone();
    let measurement_id = m.measurement_id.clone();
    if measurement_id.is_empty() {
        m.measurement_id = Uuid::new_v4().to_string();
    }
    let created: Option<Measurement> = db
        .create(("measurements", m.measurement_id.clone()))
        .content(m)
        .await?;

    if crate::cache::cache_available() {
        crate::cache::cache_del(&format!("measurements:{}", patient_id)).await;
        crate::cache::cache_del(&format!("last_measurement:{}", patient_id)).await;
    }

    created.ok_or_else(|| anyhow::anyhow!("Failed to create measurement"))
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
    let measurements: Vec<Measurement> = db
        .query("SELECT * FROM measurements WHERE patient_id = $pid ORDER BY timestamp DESC")
        .bind(("pid", pid))
        .await?
        .take(0)?;

    if crate::cache::cache_available()
        && let Ok(json) = serde_json::to_string(&measurements)
    {
        crate::cache::cache_set(&cache_key, &json, 60).await;
    }

    Ok(measurements)
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
    let measurements: Vec<Measurement> = db
        .query("SELECT * FROM measurements WHERE patient_id = $pid ORDER BY timestamp DESC LIMIT 1")
        .bind(("pid", pid))
        .await?
        .take(0)?;
    let result = measurements.into_iter().next();

    if crate::cache::cache_available()
        && let Ok(json) = serde_json::to_string(&result)
    {
        crate::cache::cache_set(&cache_key, &json, 60).await;
    }

    Ok(result)
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

pub async fn list_staff(db: &Surreal<Db>) -> Result<Vec<User>> {
    let staff: Vec<User> = db
        .query(
            "SELECT * FROM users WHERE rol = $medico OR rol = $enfermero ORDER BY created_at DESC",
        )
        .bind(("medico", "Medico"))
        .bind(("enfermero", "Enfermero"))
        .await?
        .take(0)?;
    Ok(staff)
}

pub async fn list_staff_paginated(db: &Surreal<Db>, limit: u32, offset: u32) -> Result<Vec<User>> {
    let staff: Vec<User> = db
        .query("SELECT * FROM users WHERE rol = $medico OR rol = $enfermero ORDER BY created_at DESC LIMIT $limit START $offset")
        .bind(("medico", "Medico"))
        .bind(("enfermero", "Enfermero"))
        .bind(("limit", limit as i64))
        .bind(("offset", offset as i64))
        .await?
        .take(0)?;
    Ok(staff)
}

pub async fn count_staff(db: &Surreal<Db>) -> Result<u64> {
    let count: Vec<serde_json::Value> = db
        .query("SELECT count() as count FROM users WHERE rol = $medico OR rol = $enfermero GROUP BY count")
        .bind(("medico", "Medico"))
        .bind(("enfermero", "Enfermero"))
        .await?
        .take(0)?;
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
