#![allow(dead_code)]

use std::sync::Arc;
use anyhow::Result;
use surrealdb::engine::local::{Db, SurrealKv};
use surrealdb::Surreal;
use uuid::Uuid;
use dmart_shared::models::*;

pub type Database = Arc<Surreal<Db>>;

pub async fn connect(path: &str) -> Result<Database> {
    // Ensure parent directory exists for persistence
    if let Some(parent) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(parent)?;
    }
    
    let db = Surreal::new::<SurrealKv>(path).await?;
    db.use_ns("dmart").use_db("icu").await?;
    Ok(Arc::new(db))
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

pub async fn update_patient(db: &Surreal<Db>, id: &str, patient: Patient) -> Result<Option<Patient>> {
    let updated: Option<Patient> = db
        .update(("patients", id))
        .content(patient)
        .await?;
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

pub async fn search_patients(db: &Surreal<Db>, query: &str, limit: u32, offset: u32) -> Result<Vec<Patient>> {
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
    if crate::cache::cache_available() {
        if let Some(cached) = crate::cache::cache_get(&cache_key).await {
            if let Ok(measurements) = serde_json::from_str::<Vec<Measurement>>(&cached) {
                return Ok(measurements);
            }
        }
    }

    let pid = patient_id.to_string();
    let measurements: Vec<Measurement> = db
        .query("SELECT * FROM measurements WHERE patient_id = $pid ORDER BY timestamp DESC")
        .bind(("pid", pid))
        .await?
        .take(0)?;

    if crate::cache::cache_available() {
        if let Ok(json) = serde_json::to_string(&measurements) {
            crate::cache::cache_set(&cache_key, &json, 60).await;
        }
    }

    Ok(measurements)
}

pub async fn get_last_measurement(
    db: &Surreal<Db>,
    patient_id: &str,
) -> Result<Option<Measurement>> {
    let cache_key = format!("last_measurement:{}", patient_id);
    if crate::cache::cache_available() {
        if let Some(cached) = crate::cache::cache_get(&cache_key).await {
            if let Ok(m) = serde_json::from_str::<Option<Measurement>>(&cached) {
                return Ok(m);
            }
        }
    }

    let pid = patient_id.to_string();
    let measurements: Vec<Measurement> = db
        .query("SELECT * FROM measurements WHERE patient_id = $pid ORDER BY timestamp DESC LIMIT 1")
        .bind(("pid", pid))
        .await?
        .take(0)?;
    let result = measurements.into_iter().next();

    if crate::cache::cache_available() {
        if let Ok(json) = serde_json::to_string(&result) {
            crate::cache::cache_set(&cache_key, &json, 60).await;
        }
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

pub async fn get_cama_libre_por_tipo(db: &Surreal<Db>, tipo: &TipoCama) -> Result<Option<Cama>> {
    let todas: Vec<Cama> = db.select("camas").await?;
    Ok(todas.into_iter().find(|c| c.estado == EstadoCama::Libre && c.tipo == *tipo))
}

pub async fn get_cama(db: &Surreal<Db>, id: &str) -> Result<Option<Cama>> {
    let cama: Option<Cama> = db.select(("camas", id)).await?;
    Ok(cama)
}

pub async fn get_cama_by_numero(db: &Surreal<Db>, numero: u8) -> Result<Option<Cama>> {
    let todas: Vec<Cama> = db.select("camas").await?;
    Ok(todas.into_iter().find(|c| c.numero == numero))
}

pub async fn update_cama(db: &Surreal<Db>, id: &str, cama: Cama) -> Result<Option<Cama>> {
    let updated: Option<Cama> = db
        .update(("camas", id))
        .content(cama)
        .await?;
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
    let todas: Vec<Cama> = db.select("camas").await?;
    Ok(todas.into_iter().find(|c| c.estado == EstadoCama::Libre))
}

pub async fn count_camas_por_tipo(db: &Surreal<Db>) -> Result<Vec<dmart_shared::models::TipoCamaCount>> {
    use std::collections::HashMap;
    let todas: Vec<Cama> = db.select("camas").await?;
    let mut map: HashMap<String, (u8, u8)> = HashMap::new();
    for c in &todas {
        let key = c.tipo.label().to_string();
        let entry = map.entry(key).or_insert((0, 0));
        entry.0 += 1;
        if c.estado == EstadoCama::Libre {
            entry.1 += 1;
        }
    }
    Ok(map.into_iter().map(|(tipo, (total, libres))| dmart_shared::models::TipoCamaCount { tipo, total, libres }).collect())
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
        let updated: Option<Cama> = db
            .update(("camas", cama_id))
            .content(c)
            .await?;
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
        let updated: Option<Cama> = db
            .update(("camas", cama_id))
            .content(c)
            .await?;
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
    let updated: Option<Equipo> = db
        .update(("equipos", id))
        .content(equipo)
        .await?;
    Ok(updated)
}

pub async fn list_equipos(db: &Surreal<Db>) -> Result<Vec<Equipo>> {
    let equipos: Vec<Equipo> = db.select("equipos").await?;
    Ok(equipos)
}

pub async fn list_equipos_paginated(db: &Surreal<Db>, limit: u32, offset: u32) -> Result<Vec<Equipo>> {
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
    let todas: Vec<Equipo> = db.select("equipos").await?;
    Ok(todas.into_iter().filter(|e| e.cama_id.as_deref() == Some(cama_id)).collect())
}

pub async fn asignar_equipo_cama(db: &Surreal<Db>, equipo_id: &str, cama_id: &str) -> Result<Option<Equipo>> {
    let equipo: Option<Equipo> = db.select(("equipos", equipo_id)).await?;
    if let Some(mut e) = equipo {
        e.cama_id = Some(cama_id.to_string());
        let updated: Option<Equipo> = db
            .update(("equipos", equipo_id))
            .content(e)
            .await?;
        Ok(updated)
    } else {
        Err(anyhow::anyhow!("Equipo not found"))
    }
}

pub async fn desvincular_equipo_cama(db: &Surreal<Db>, equipo_id: &str) -> Result<Option<Equipo>> {
    let equipo: Option<Equipo> = db.select(("equipos", equipo_id)).await?;
    if let Some(mut e) = equipo {
        e.cama_id = None;
        let updated: Option<Equipo> = db
            .update(("equipos", equipo_id))
            .content(e)
            .await?;
        Ok(updated)
    } else {
        Err(anyhow::anyhow!("Equipo not found"))
    }
}

pub async fn list_equipos_disponibles(db: &Surreal<Db>) -> Result<Vec<Equipo>> {
    let todas: Vec<Equipo> = db.select("equipos").await?;
    Ok(todas.into_iter().filter(|e| e.estado == EstadoEquipo::Activo && e.cama_id.is_none()).collect())
}

pub async fn count_equipos_por_tipo(db: &Surreal<Db>) -> Result<Vec<dmart_shared::models::EquipoTipoCount>> {
    use std::collections::HashMap;
    let todas: Vec<Equipo> = db.select("equipos").await?;
    let mut map: HashMap<String, (u32, u32)> = HashMap::new();
    for e in &todas {
        let key = e.tipo.label().to_string();
        let entry = map.entry(key).or_insert((0, 0));
        entry.0 += 1;
        if e.estado == EstadoEquipo::Activo && e.cama_id.is_none() {
            entry.1 += 1;
        }
    }
    Ok(map.into_iter().map(|(tipo, (total, disponibles))| dmart_shared::models::EquipoTipoCount { tipo, total, disponibles }).collect())
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
    let todos: Vec<User> = db.select("users").await?;
    Ok(todos.into_iter().find(|u| u.username == username))
}

pub async fn update_user(db: &Surreal<Db>, id: &str, user: User) -> Result<Option<User>> {
    let updated: Option<User> = db
        .update(("users", id))
        .content(user)
        .await?;
    Ok(updated)
}

pub async fn list_users(db: &Surreal<Db>) -> Result<Vec<User>> {
    let users: Vec<User> = db.select("users").await?;
    Ok(users)
}

pub async fn list_staff(db: &Surreal<Db>) -> Result<Vec<User>> {
    let todos: Vec<User> = db.select("users").await?;
    Ok(todos.into_iter()
        .filter(|u| matches!(u.rol, UserRole::Medico | UserRole::Enfermero))
        .collect())
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