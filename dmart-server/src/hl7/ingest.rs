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
use surrealdb::sql::Thing;

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

    // ─── P1.4: Idempotencia por MSH.10 (message_id) ───
    // La clave es (tenant_id, message_id). Si ya existe, devolvemos la
    // medición existente en lugar de crear una duplicada.
    let tenant = &patient.tenant_id;
    let msh10 = &msg.message_id;

    // Intentar insertar la clave de idempotencia (única por tenant+message_id).
    // Si falla por UNIQUE, la fila ya existe → reintento.
    let key_record_id: Option<String> = {
        let res = db
            .query(
                "CREATE hl7_ingest_key CONTENT { tenant_id: $tenant, message_id: $msh10, sender: $sender, source: $source } RETURN record::id(id) as id_str"
            )
            .bind(("tenant", tenant.to_string()))
            .bind(("msh10", msh10.to_string()))
            .bind(("sender", msg.sender.clone()))
            .bind(("source", msg.source.label().to_string()))
            .await;

        match res {
            Ok(mut res) => {
                // Verificar si hay errores en la respuesta (ej. UNIQUE violation)
                if let Some((_, err)) = res.take_errors().into_iter().next() {
                    let err_str = err.to_string();
                    if err_str.contains("UNIQUE") || err_str.contains("already contains") {
                        None // Clave ya existe → reintento
                    } else {
                        return Err(anyhow!("error creando clave idempotencia: {}", err_str));
                    }
                } else {
                    // Primera vez: la fila se creó. Extraer el ID.
                    let created_key: Option<serde_json::Value> = res.take(0)?;
                    created_key
                        .and_then(|v| v.get("id_str").cloned())
                        .and_then(|v| v.as_str().map(|s| s.to_string()))
                }
            }
            Err(e) => {
                // Error de conexión/consulta - si es UNIQUE, tratar como reintento
                let err_str = e.to_string();
                if err_str.contains("UNIQUE") || err_str.contains("already contains") {
                    None
                } else {
                    return Err(anyhow!("error creando clave idempotencia: {}", err_str));
                }
            }
        }
    };

    if let Some(key_record_id) = key_record_id {
        // Primera pasada: crear medición y actualizar clave
        let measurement = do_ingest_vitals(db, &patient, msg).await?;

        // Actualizar la clave con el measurement_id
        let _ = db
            .query("UPDATE $id SET measurement_id = $mid")
            .bind(("id", Thing::from(("hl7_ingest_key", key_record_id.as_str()))))
            .bind(("mid", measurement.measurement_id.clone()))
            .await;

        Ok(measurement)
    } else {
        // Clave ya existe (UNIQUE violation) → reintento.
        // Buscar la fila existente y devolver la measurement_id asociada.
        let existing: Vec<serde_json::Value> = db
            .query(
                "SELECT measurement_id FROM hl7_ingest_key WHERE tenant_id = $tenant AND message_id = $msh10 LIMIT 1"
            )
            .bind(("tenant", tenant.to_string()))
            .bind(("msh10", msh10.to_string()))
            .await?
            .take(0)?;

        if let Some(row) = existing.first()
            && let Some(mid) = row.get("measurement_id").and_then(|v| v.as_str())
            && !mid.is_empty()
        {
            // La medición ya fue creada en la primera pasada → devolverla.
            return db_ops::get_measurement(db, mid)
                .await?
                .ok_or_else(|| anyhow!("medición referenciada no encontrada: {}", mid));
        }

        // Edge case: la clave existe pero measurement_id aún no se escribió
        // (proceso murió entre insert y update). Devolvemos la última
        // medición del paciente como fallback seguro.
        let last = db_ops::get_last_measurement(db, &patient.patient_id).await?;
        last.ok_or_else(|| anyhow!("reintento sin measurement_id y sin historial previo"))
    }
}

/// Lógica interna de ingesta (extraída para no duplicar código entre primera
/// pasada y reintento que cae al fallback).
async fn do_ingest_vitals(
    db: &Database,
    patient: &Patient,
    msg: &VitalsMessage,
) -> Result<Measurement> {
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
