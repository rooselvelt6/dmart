// SPEC-031: superficie HL7/MLLP legacy ejercitada por los 38 tests de conformance/integración (SPEC-028); el pipeline ACTIVO es server_ingest + ingest/ (SPEC-031).
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
///
/// **SPEC-025 (`tenant_scope`):** cuando se pasa `Some(tenant)`, la resolución
/// queda acotada a ese tenant. Es obligatorio para toda ingesta autenticada
/// (HTTP `/monitores/hl7`), donde la referencia `PID-3` la elige el emisor: sin
/// el filtro, un usuario de un hospital podría escribir signos vitales en la
/// historia de un paciente de otro (`cross-tenant write`). `None` se reserva
/// para el listener MLLP, cuya autenticación vive en la frontera de red.
pub async fn resolve_patient(
    db: &Database,
    msg: &VitalsMessage,
    tenant_scope: Option<&str>,
) -> Result<Option<Patient>> {
    let tenant = tenant_scope.unwrap_or_default();

    if msg.patient_ref_is_uuid {
        let patient = db_ops::get_patient(db, &msg.patient_ref)
            .await
            .context("get patient by id")?;
        return Ok(patient.filter(|p| p.tenant_id == tenant || tenant_scope.is_none()));
    }

    // Búsqueda por MRN/cédula acotada al tenant cuando aplica.
    let patients: Vec<Patient> = if tenant_scope.is_some() {
        db_ops::get_patient_by_mrn_for_tenant(db, &msg.patient_ref, tenant)
            .await
            .context("get patient by MRN (scoped)")?
    } else {
        db_ops::get_patient_by_mrn(db, &msg.patient_ref)
            .await
            .context("get patient by MRN")?
            .into_iter()
            .collect()
    };

    Ok(patients.into_iter().next())
}

/// Ingiere el mensaje: persiste una medición, actualiza el paciente y publica
/// el evento realtime. Devuelve la medición creada.
///
/// La medición **hereda el tenant del paciente resuelto** (`Measurement::
/// new_for_tenant`), nunca el tenant por defecto: en multi-tenancy un paciente
/// de `hosp-b` ingested por MLLP quedaba etiquetado como `default` y se salía
/// del RLS.
pub async fn ingest_vitals(db: &Database, msg: &VitalsMessage) -> Result<Measurement> {
    ingest_vitals_for_tenant(db, msg, None).await
}

/// SPEC-025: ingesta acotada a un tenant. `Some(tenant)` = el emisor está
/// autenticado y solo puede escribir en su propio hospital. `None` = MLLP
/// (frontera de red), donde el tenant se deduce del paciente encontrado.
pub async fn ingest_vitals_for_tenant(
    db: &Database,
    msg: &VitalsMessage,
    tenant_scope: Option<&str>,
) -> Result<Measurement> {
    let patient = resolve_patient(db, msg, tenant_scope)
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

    let mut measurement =
        Measurement::new_for_tenant(&patient.patient_id, &patient.tenant_id, apache, gcs);
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

    crate::realtime::publish_for_tenant(
        &created.tenant_id,
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
