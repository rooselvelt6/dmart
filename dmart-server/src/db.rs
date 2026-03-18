use std::sync::Arc;
use anyhow::Result;
use surrealdb::engine::local::{Db, RocksDb};
use surrealdb::Surreal;
use uuid::Uuid;
use dmart_shared::models::*;

pub type Database = Arc<Surreal<Db>>;

pub async fn connect(path: &str) -> Result<Database> {
    let db = Surreal::new::<RocksDb>(path).await?;
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
