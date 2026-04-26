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

pub async fn list_patients(db: &Surreal<Db>) -> Result<Vec<Patient>> {
    let patients: Vec<Patient> = db.select("patients").await?;
    Ok(patients)
}

pub async fn search_patients(db: &Surreal<Db>, query: &str) -> Result<Vec<Patient>> {
    let q = query.to_lowercase();
    let patients: Vec<Patient> = db.select("patients").await?;
    let filtered: Vec<Patient> = patients.into_iter()
        .filter(|p| {
            p.nombre.to_lowercase().contains(&q) ||
            p.apellido.to_lowercase().contains(&q) ||
            p.cedula.to_lowercase().contains(&q) ||
            p.historia_clinica.to_lowercase().contains(&q)
        })
        .collect();
    Ok(filtered)
}

pub async fn delete_patient(db: &Surreal<Db>, id: &str) -> Result<()> {
    let _: Option<Patient> = db.delete(("patients", id)).await?;
    Ok(())
}

// ─── Measurements ──────────────────────────────────────────────────────────

pub async fn create_measurement(db: &Surreal<Db>, mut m: Measurement) -> Result<Measurement> {
    let measurement_id = m.measurement_id.clone();
    if measurement_id.is_empty() {
        m.measurement_id = Uuid::new_v4().to_string();
    }
    let created: Option<Measurement> = db
        .create(("measurements", m.measurement_id.clone()))
        .content(m)
        .await?;
    created.ok_or_else(|| anyhow::anyhow!("Failed to create measurement"))
}

pub async fn get_measurements_for_patient(
    db: &Surreal<Db>,
    patient_id: &str,
) -> Result<Vec<Measurement>> {
    let pid = patient_id.to_string();
    let measurements: Vec<Measurement> = db
        .query("SELECT * FROM measurements WHERE patient_id = $pid ORDER BY timestamp DESC")
        .bind(("pid", pid))
        .await?
        .take(0)?;
    Ok(measurements)
}

pub async fn get_last_measurement(
    db: &Surreal<Db>,
    patient_id: &str,
) -> Result<Option<Measurement>> {
    let pid = patient_id.to_string();
    let measurements: Vec<Measurement> = db
        .query("SELECT * FROM measurements WHERE patient_id = $pid ORDER BY timestamp DESC LIMIT 1")
        .bind(("pid", pid))
        .await?
        .take(0)?;
    Ok(measurements.into_iter().next())
}

// ─── Camas ───────────────────────────────────────────────────────────

pub async fn init_camas(db: &Surreal<Db>, cantidad: u8) -> Result<Vec<Cama>> {
    let mut camas = Vec::new();
    for i in 1..=cantidad {
        let cama = Cama::new(i);
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

pub async fn get_cama_libre(db: &Surreal<Db>) -> Result<Option<Cama>> {
    let todas: Vec<Cama> = db.select("camas").await?;
    Ok(todas.into_iter().find(|c| c.estado == EstadoCama::Libre))
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

pub async fn delete_equipo(db: &Surreal<Db>, id: &str) -> Result<()> {
    let _: Option<Equipo> = db.delete(("equipos", id)).await?;
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

pub async fn delete_user(db: &Surreal<Db>, id: &str) -> Result<()> {
    let _: Option<User> = db.delete(("users", id)).await?;
    Ok(())
}