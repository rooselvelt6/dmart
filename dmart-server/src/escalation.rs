//! SPEC-019: Alert Escalation — escalamiento y acuse de alertas.
//!
//! Modelos y operaciones de las tablas SurrealDB `escalation_policies`
//! (configuración por severidad, upsert por clave estable) y `escalations`
//! (incidentes con máquina de estados created → acknowledged → escalated →
//! resolved y timestamps RFC3339). Productor de eventos en tiempo real en el
//! canal "escalation" al crear / acusar / escalar.

use crate::db::Database;
use crate::realtime;
use anyhow::{Result, anyhow};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use surrealdb::RecordId;
use uuid::Uuid;

/// Severidad clínica de la alerta. El orden deriva de la declaración de
/// variantes: `low < medium < high < critical`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Low => "low",
            Severity::Medium => "medium",
            Severity::High => "high",
            Severity::Critical => "critical",
        }
    }
}

/// Ciclo de vida de una escalación.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EscalationStatus {
    Created,
    Acknowledged,
    Escalated,
    Resolved,
}

impl EscalationStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            EscalationStatus::Created => "created",
            EscalationStatus::Acknowledged => "acknowledged",
            EscalationStatus::Escalated => "escalated",
            EscalationStatus::Resolved => "resolved",
        }
    }
}

/// Política de escalamiento configurable, upsert por `severity`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationPolicy {
    pub severity: Severity,
    pub max_response_minutes: u32,
    pub timeout_minutes: u32,
    pub target_role: String,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// Incidente de escalación (activo o resuelto).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Escalation {
    pub id: RecordId,
    pub patient_id: String,
    pub alert_type: String,
    pub severity: Severity,
    pub level: u32,
    pub status: EscalationStatus,
    pub policy_severity: String,
    pub created_at: String,
    pub acknowledged_at: Option<String>,
    pub escalated_at: Option<String>,
    pub resolved_at: Option<String>,
    pub acknowledged_by: Option<String>,
    pub escalated_to: Option<String>,
}

/// Defaults de política por severidad (espejo de la migración 019).
pub fn default_policy(severity: Severity) -> EscalationPolicy {
    let (max_response_minutes, timeout_minutes) = match severity {
        Severity::Low => (60, 120),
        Severity::Medium => (30, 90),
        Severity::High => (15, 60),
        Severity::Critical => (5, 30),
    };
    let now = Utc::now().to_rfc3339();
    EscalationPolicy {
        severity,
        max_response_minutes,
        timeout_minutes,
        target_role: if severity >= Severity::High {
            "Dr".to_string()
        } else {
            "Enfermera".to_string()
        },
        enabled: true,
        created_at: now.clone(),
        updated_at: now,
    }
}

// ─── Políticas ────────────────────────────────────────────────────────────

pub async fn get_policy(db: &Database, severity: Severity) -> Result<Option<EscalationPolicy>> {
    Ok(db
        .select(("escalation_policies", severity.as_str()))
        .await?)
}

/// Upsert por severidad (clave única estable): actualiza si ya existe, crea si no.
pub async fn upsert_policy(db: &Database, policy: EscalationPolicy) -> Result<EscalationPolicy> {
    let key = policy.severity.as_str();
    let now = Utc::now().to_rfc3339();
    let existing: Option<EscalationPolicy> = db.select(("escalation_policies", key)).await?;
    let saved = match existing {
        Some(prev) => {
            let merged = EscalationPolicy {
                created_at: prev.created_at,
                updated_at: now,
                ..policy
            };
            db.update(("escalation_policies", key))
                .content(merged)
                .await?
        }
        None => {
            let mut created = policy;
            created.created_at = now.clone();
            created.updated_at = now;
            db.create(("escalation_policies", key))
                .content(created)
                .await?
        }
    };
    saved.ok_or_else(|| anyhow!("failed to upsert escalation policy for {}", key))
}

/// Todas las políticas ordenadas por severidad ascendente (low → critical).
pub async fn list_policies(db: &Database) -> Result<Vec<EscalationPolicy>> {
    let mut policies: Vec<EscalationPolicy> = db.select("escalation_policies").await?;
    policies.sort_by_key(|a| a.severity);
    Ok(policies)
}

// ─── Escalaciones ─────────────────────────────────────────────────────────

pub async fn get_escalation(db: &Database, id: &str) -> Result<Option<Escalation>> {
    Ok(db.select(("escalations", id)).await?)
}

/// Crea una escalación en nivel inicial derivando la política vigente de la
/// severidad. Publica el evento en tiempo real y registra el `EventType::Alert`
/// en la timeline del paciente.
pub async fn create_escalation(
    db: &Database,
    patient_id: &str,
    alert_type: &str,
    severity: Severity,
) -> Result<Escalation> {
    let id = Uuid::new_v4().to_string();
    let policy = get_policy(db, severity).await?;
    let policy_severity = policy
        .map(|p| p.severity.as_str().to_string())
        .unwrap_or_else(|| severity.as_str().to_string());
    let created_at = Utc::now().to_rfc3339();

    let sql = r#"
        CREATE escalations CONTENT {
            id: $id,
            patient_id: $patient_id,
            alert_type: $alert_type,
            severity: $severity,
            level: 1,
            status: 'created',
            policy_severity: $policy_severity,
            created_at: $created_at
        }
    "#;
    db.query(sql)
        .bind(("id", RecordId::from(("escalations", id.clone()))))
        .bind(("patient_id", patient_id.to_string()))
        .bind(("alert_type", alert_type.to_string()))
        .bind(("severity", severity.as_str()))
        .bind(("policy_severity", policy_severity))
        .bind(("created_at", created_at))
        .await?;

    let created: Option<Escalation> = db.select(("escalations", &id)).await?;
    let created = created.ok_or_else(|| anyhow!("failed to create escalation"))?;

    realtime::publish(
        "escalation",
        json!({
            "event": "created",
            "escalation_id": id,
            "patient_id": patient_id,
            "alert_type": alert_type,
            "severity": severity.as_str()
        }),
    );
    let _ = crate::patient_timeline::record_alert_event(
        db,
        patient_id,
        alert_type,
        &format!(
            "Escalación creada ({}) — política {}",
            severity.as_str(),
            created.policy_severity
        ),
        to_timeline_severity(severity),
    )
    .await;

    Ok(created)
}

/// Marca la escalación como `acknowledged` (actor opcional). Idempotente: si ya
/// está acusada o resuelta devuelve el registro actual sin re-marcar timestamps.
pub async fn acknowledge_escalation(db: &Database, id: &str) -> Result<Option<Escalation>> {
    let Some(esc) = get_escalation(db, id).await? else {
        return Ok(None);
    };
    if matches!(
        esc.status,
        EscalationStatus::Acknowledged | EscalationStatus::Resolved
    ) {
        return Ok(Some(esc));
    }

    let acknowledged_at = Utc::now().to_rfc3339();
    let updated: Vec<Escalation> = db
        .query(
            "UPDATE escalations SET status = $status, acknowledged_at = $ts, acknowledged_by = NONE WHERE id = $id",
        )
        .bind(("status", EscalationStatus::Acknowledged.as_str()))
        .bind(("ts", acknowledged_at))
        .bind(("id", esc.id.clone()))
        .await?
        .take(0)?;
    let updated = updated.into_iter().next();

    realtime::publish(
        "escalation",
        json!({ "event": "acknowledged", "escalation_id": id }),
    );
    Ok(updated.or(Some(esc)))
}

/// Sube un nivel la escalación (+1) y la marca `escalated` (rol objetivo
/// opcional). No vuelve a escalar una resuelta.
pub async fn escalate_escalation(
    db: &Database,
    id: &str,
    escalated_to: Option<&str>,
) -> Result<Option<Escalation>> {
    let Some(esc) = get_escalation(db, id).await? else {
        return Ok(None);
    };
    if esc.status == EscalationStatus::Resolved {
        return Ok(Some(esc));
    }

    let escalated_at = Utc::now().to_rfc3339();
    let level = esc.level + 1;
    let updated: Vec<Escalation> = db
        .query(
            "UPDATE escalations SET status = $status, level = $level, escalated_at = $ts, escalated_to = $to WHERE id = $id",
        )
        .bind(("status", EscalationStatus::Escalated.as_str()))
        .bind(("level", level))
        .bind(("ts", escalated_at))
        .bind(("to", escalated_to.map(String::from)))
        .bind(("id", esc.id.clone()))
        .await?
        .take(0)?;
    let updated = updated.into_iter().next();

    realtime::publish(
        "escalation",
        json!({
            "event": "escalated",
            "escalation_id": id,
            "level": level
        }),
    );
    Ok(updated.or(Some(esc)))
}

/// Marca la escalación como `resolved`. Idempotente sobre resueltas.
pub async fn resolve_escalation(db: &Database, id: &str) -> Result<Option<Escalation>> {
    let Some(esc) = get_escalation(db, id).await? else {
        return Ok(None);
    };
    if esc.status == EscalationStatus::Resolved {
        return Ok(Some(esc));
    }

    let resolved_at = Utc::now().to_rfc3339();
    let updated: Vec<Escalation> = db
        .query("UPDATE escalations SET status = $status, resolved_at = $ts WHERE id = $id")
        .bind(("status", EscalationStatus::Resolved.as_str()))
        .bind(("ts", resolved_at))
        .bind(("id", esc.id.clone()))
        .await?
        .take(0)?;
    let updated = updated.into_iter().next();

    realtime::publish(
        "escalation",
        json!({ "event": "resolved", "escalation_id": id }),
    );
    Ok(updated.or(Some(esc)))
}

/// Escalaciones no resueltas, ordenadas por severidad desc / created_at asc.
pub async fn active_escalations(db: &Database) -> Result<Vec<Escalation>> {
    let escalations: Vec<Escalation> = db
        .query("SELECT * FROM escalations WHERE status != $resolved")
        .bind(("resolved", EscalationStatus::Resolved.as_str()))
        .await?
        .take(0)?;
    let mut escalations = escalations;
    escalations.sort_by(|a, b| {
        b.severity
            .cmp(&a.severity)
            .then_with(|| a.created_at.cmp(&b.created_at))
    });
    Ok(escalations)
}

/// Mapeo de severidad de escalación a severidad de timeline (SPEC-015).
fn to_timeline_severity(s: Severity) -> crate::patient_timeline::EventSeverity {
    match s {
        Severity::Low => crate::patient_timeline::EventSeverity::Info,
        Severity::Medium | Severity::High => crate::patient_timeline::EventSeverity::Warning,
        Severity::Critical => crate::patient_timeline::EventSeverity::Critical,
    }
}
