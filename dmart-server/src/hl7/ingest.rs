//! Ingestión de signos vitales desde monitores (HL7) hacia el registro clínico.
//!
//! Convierte un `VitalsMessage` parseado en una `Measurement` completa
//! (los scores se recalculan con las escalas clínicas) y publica el evento
//! en tiempo real.

use crate::db::{self as db_ops, Database};
use crate::hl7::parser::{VitalsMessage, vitals_into_apache};
use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use dmart_shared::models::{ApacheIIData, GcsData, Measurement, Patient};

/// Resuelve un paciente a partir de la referencia del mensaje HL7.
/// Si `msg.patient_ref_is_uuid` busca por `patient_id`; si no, por MRN/cédula.
pub async fn resolve_patient(db: &Database, msg: &VitalsMessage) -> Result<Option<Patient>> {
    let patient = if msg.patient_ref_is_uuid {
        db_ops::get_patient(db, &msg.patient_ref)
            .await
            .context("get patient by id")?
    } else {
        db_ops::get_patient_by_mrn(db, &msg.patient_ref)
            .await
            .context("get patient by MRN")?
    };
    Ok(patient)
}

/// Ingiere el mensaje: persiste una medición, actualiza el paciente y publica
/// el evento realtime. Devuelve el ID de la medición creada.
pub async fn ingest_vitals(db: &Database, msg: &VitalsMessage) -> Result<Measurement> {
    let patient = resolve_patient(db, msg)
        .await?
        .ok_or_else(|| anyhow!("paciente no encontrado para ref {}", msg.patient_ref))?;

    // Base: la última medición del paciente (para conservar labs/GCS), o neutra.
    let last = db_ops::get_last_measurement(db, &patient.patient_id).await?;

    let mut base = last
        .as_ref()
        .map(|m| m.apache_data.clone())
        .unwrap_or_else(ApacheIIData::default);
    base.edad = patient.edad;
    base.gcs_ojos = last
        .as_ref()
        .map(|m| m.gcs_data.apertura_ocular)
        .unwrap_or(4);
    base.gcs_verbal = last
        .as_ref()
        .map(|m| m.gcs_data.respuesta_verbal)
        .unwrap_or(5);
    base.gcs_motor = last
        .as_ref()
        .map(|m| m.gcs_data.respuesta_motora)
        .unwrap_or(6);
    base.gcs_total = base.gcs_ojos + base.gcs_verbal + base.gcs_motor;

    let apache = vitals_into_apache(msg, &base);

    let gcs = last
        .as_ref()
        .map(|m| m.gcs_data.clone())
        .or(Some(GcsData::default()))
        .expect("gte");

    let mut measurement = Measurement::new(&patient.patient_id, apache, gcs);
    measurement.timestamp = msg.timestamp.clone();
    measurement.notas = format!(
        "Auto-ingesta HL7 v2 ({}) — {}",
        msg.source.label(),
        msg.sender
    );

    let created = db_ops::create_measurement(db, measurement)
        .await
        .context("create measurement")?;

    // Actualizar el estado del paciente con los nuevos scores
    if let Ok(Some(mut p)) = db_ops::get_patient(db, &patient.patient_id).await {
        p.estado_gravedad = created.severity.clone();
        p.ultimo_apache_score = Some(created.apache_score);
        p.ultimo_gcs_score = Some(created.gcs_score);
        p.ultimo_sofa_score = created.sofa_score;
        p.ultimo_saps3_score = created.saps3_score;
        p.ultimo_news2_score = created.news2_score;
        p.mortality_risk = Some(created.mortality_risk);
        p.updated_at = Utc::now().to_rfc3339();
        let _ = db_ops::update_patient(db, &patient.patient_id, p).await;
    }

    crate::realtime::publish(
        "measurement",
        serde_json::json!({
            "patient_id": created.patient_id,
            "measurement_id": created.measurement_id,
            "apache_score": created.apache_score,
            "gcs_score": created.gcs_score,
            "severity": created.severity.label(),
            "mortality_risk": created.mortality_risk,
            "source": msg.source.label(),
            "timestamp": created.timestamp,
        }),
    );

    Ok(created)
}
