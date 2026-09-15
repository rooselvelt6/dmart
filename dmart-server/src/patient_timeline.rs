use crate::db::Database;
use crate::metrics::scale_calculated;
use chrono::Utc;
use dmart_shared::models::Patient;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicI64, Ordering};
use uuid::Uuid;

static LAST_OCCURRED_AT: AtomicI64 = AtomicI64::new(0);

/// Timestamp epoch-millis estrictamente creciente por proceso: garantiza
/// `occurred_at` único y cursor de paginación estable (SPEC-015).
fn next_occurred_at() -> i64 {
    let now = Utc::now().timestamp_millis();
    loop {
        let last = LAST_OCCURRED_AT.load(Ordering::Relaxed);
        let next = now.max(last + 1);
        if LAST_OCCURRED_AT
            .compare_exchange_weak(last, next, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
        {
            return next;
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "PascalCase")]
pub enum EventType {
    Admission,
    VitalSigns,
    ScoreCalculated,
    Intervention,
    Medication,
    Procedure,
    Note,
    Discharge,
    Alert,
    CdsAction,
}

impl EventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            EventType::Admission => "Admission",
            EventType::VitalSigns => "VitalSigns",
            EventType::ScoreCalculated => "ScoreCalculated",
            EventType::Intervention => "Intervention",
            EventType::Medication => "Medication",
            EventType::Procedure => "Procedure",
            EventType::Note => "Note",
            EventType::Discharge => "Discharge",
            EventType::Alert => "Alert",
            EventType::CdsAction => "CdsAction",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EventSeverity {
    Info,
    Warning,
    Critical,
}

impl EventSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            EventSeverity::Info => "info",
            EventSeverity::Warning => "warning",
            EventSeverity::Critical => "critical",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatientEvent {
    pub id: surrealdb::RecordId,
    pub patient_id: String,
    pub event_type: EventType,
    pub occurred_at: i64,
    pub payload: serde_json::Value,
    pub fingerprint: String,
    pub severity: EventSeverity,
    pub source: String,
    pub meta: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct TimelineQuery {
    pub since: Option<i64>,
    pub until: Option<i64>,
    pub event_type: Option<EventType>,
    pub severity: Option<EventSeverity>,
    pub cursor: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct TimelineResponse {
    pub events: Vec<PatientEvent>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

impl PatientEvent {
    pub fn new(
        patient_id: String,
        event_type: EventType,
        payload: serde_json::Value,
        severity: EventSeverity,
        source: String,
    ) -> Self {
        let occurred_at = next_occurred_at();
        let canonical = serde_json::to_vec(&payload).unwrap_or_default();
        let fingerprint = format!("{:x}", Sha256::new().chain_update(&canonical).finalize());

        Self {
            id: surrealdb::RecordId::from(("patient_event", Uuid::new_v4().to_string())),
            patient_id,
            event_type,
            occurred_at,
            payload,
            fingerprint,
            severity,
            source,
            meta: serde_json::Value::Object(Default::default()),
        }
    }

    pub async fn store(&self, db: &Database) -> Result<(), Box<surrealdb::Error>> {
        let sql = r#"
            CREATE patient_event CONTENT {
                id: $id,
                patient_id: $patient_id,
                event_type: $event_type,
                occurred_at: $occurred_at,
                payload: $payload,
                fingerprint: $fingerprint,
                severity: $severity,
                source: $source,
                meta: $meta
            }
        "#;
        db.query(sql)
            .bind(("id", self.id.clone()))
            .bind(("patient_id", self.patient_id.clone()))
            .bind(("event_type", self.event_type.as_str()))
            .bind(("occurred_at", self.occurred_at))
            .bind(("payload", self.payload.clone()))
            .bind(("fingerprint", self.fingerprint.clone()))
            .bind(("severity", self.severity.as_str()))
            .bind(("source", self.source.clone()))
            .bind(("meta", self.meta.clone()))
            .await?;
        Ok(())
    }
}

#[allow(clippy::result_large_err)]
pub async fn query_timeline(
    db: &Database,
    patient_id: &str,
    query: TimelineQuery,
) -> Result<TimelineResponse, Box<surrealdb::Error>> {
    let limit = query.limit.unwrap_or(50).min(200) as usize;
    let mut conditions = vec!["patient_id = $patient_id".to_string()];

    if query.since.is_some() {
        conditions.push("occurred_at >= $since".to_string());
    }
    if query.until.is_some() {
        conditions.push("occurred_at <= $until".to_string());
    }
    if query.event_type.is_some() {
        conditions.push("event_type = $event_type".to_string());
    }
    if query.severity.is_some() {
        conditions.push("severity = $severity".to_string());
    }
    if query.cursor.is_some() {
        conditions.push("occurred_at < $cursor".to_string());
    }

    let where_clause = if conditions.len() == 1 {
        format!("WHERE {}", conditions[0])
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let sql = format!(
        r#"
            SELECT * FROM patient_event
            {where_clause}
            ORDER BY occurred_at DESC
            LIMIT $limit
        "#
    );

    let mut q = db
        .query(&sql)
        .bind(("patient_id", patient_id.to_string()))
        .bind(("limit", limit + 1));
    if let Some(since) = query.since {
        q = q.bind(("since", since));
    }
    if let Some(until) = query.until {
        q = q.bind(("until", until));
    }
    if let Some(event_type) = &query.event_type {
        q = q.bind(("event_type", event_type.as_str().to_string()));
    }
    if let Some(severity) = &query.severity {
        q = q.bind(("severity", severity.as_str().to_string()));
    }
    if let Some(cursor) = &query.cursor {
        let ts = cursor.parse::<i64>().unwrap_or(i64::MAX);
        q = q.bind(("cursor", ts));
    }

    let mut result = q.await?;
    let events_result: Result<Vec<PatientEvent>, _> = result.take(0);
    let events = events_result.unwrap_or_default();

    let has_more = events.len() > limit;
    let events = if has_more { &events[..limit] } else { &events };
    let next_cursor = events.last().map(|e| e.occurred_at.to_string());

    Ok(TimelineResponse {
        events: events.to_vec(),
        next_cursor,
        has_more,
    })
}

pub async fn record_vital_signs_event(
    db: &Database,
    patient_id: &str,
    vitals: &crate::hl7::parser::VitalsMessage,
) -> Result<(), Box<surrealdb::Error>> {
    let payload = serde_json::json!({
        "vitals": vitals.vitals.iter().map(|v| serde_json::json!({
            "loinc": v.loinc,
            "name": v.name,
            "value": v.value,
            "unit": v.unit
        })).collect::<Vec<_>>(),
        "source": format!("{:?}", vitals.source),
        "message_id": vitals.message_id,
        "sequence_number": vitals.sequence_number
    });

    let event = PatientEvent::new(
        patient_id.to_string(),
        EventType::VitalSigns,
        payload,
        EventSeverity::Info,
        format!("{:?}", vitals.source),
    );
    event.store(db).await
}

pub async fn record_score_event(
    db: &Database,
    patient_id: &str,
    scale: &str,
    score: f64,
    breakdown: serde_json::Value,
) -> Result<(), Box<surrealdb::Error>> {
    let payload = serde_json::json!({
        "scale": scale,
        "score": score,
        "breakdown": breakdown
    });

    let event = PatientEvent::new(
        patient_id.to_string(),
        EventType::ScoreCalculated,
        payload,
        EventSeverity::Info,
        "scales".to_string(),
    );
    scale_calculated(scale);
    event.store(db).await
}

/// Extrae la clave de un `RecordId` como string plano (sin quotes/table).
fn record_key(id: &surrealdb::RecordId) -> String {
    id.key().to_string()
}

/// Serializa `PatientEvent`s a un `Bundle` FHIR R4 `type=history` (SPEC-015).
/// Cada evento se proyecta a su recurso FHIR semántico
/// (Observation/Procedure/MedicationStatement/Encounter/DocumentReference)
/// con `subject` y fingerprint SPEC-029 en `meta.extension`.
pub fn to_fhir_history_bundle(events: &[PatientEvent]) -> serde_json::Value {
    let entries: Vec<serde_json::Value> = events
        .iter()
        .map(|e| {
            let fhir_type = match e.event_type {
                EventType::Admission | EventType::Discharge => "Encounter",
                EventType::VitalSigns
                | EventType::ScoreCalculated
                | EventType::Alert
                | EventType::CdsAction => "Observation",
                EventType::Intervention | EventType::Procedure => "Procedure",
                EventType::Medication => "MedicationStatement",
                EventType::Note => "DocumentReference",
            };
            let date = chrono::DateTime::from_timestamp_millis(e.occurred_at)
                .map(|d| d.to_rfc3339())
                .unwrap_or_default();
            let code_text = match e.event_type.as_str() {
                "VitalSigns" => "Vital signs",
                "ScoreCalculated" => "Clinical score",
                "Alert" => "Clinical alert",
                "CdsAction" => "Decision support action",
                "Admission" | "Discharge" => "Admission/Discharge",
                "Intervention" | "Procedure" => "Procedure/Intervention",
                "Medication" => "Medication statement",
                "Note" => "Clinical note",
                _ => "Event",
            };
            let interpretation = match e.severity {
                EventSeverity::Info => "N",
                EventSeverity::Warning => "A",
                EventSeverity::Critical => "HH",
            };

            let resource = serde_json::json!({
                "resourceType": fhir_type,
                "id": record_key(&e.id),
                "status": "final",
                "subject": {"reference": format!("Patient/{}", e.patient_id)},
                "effectiveDateTime": date,
                "occurredDateTime": date,
                "class": {"system": "http://terminology.hl7.org/CodeSystem/v3-ActCode", "code": "IMP"},
                "code": {"text": code_text},
                "interpretation": [{"coding": [{"system": "http://hl7.org/fhir/v2/0078", "code": interpretation}]}],
                "extension": [{"url": "http://dmart.local/fhir/StructureDefinition/patient-event", "valueReference": {"reference": format!("PatientEvent/{}", e.id)}}],
                "meta": {"extension": [{"url": "http://dmart.local/fhir/StructureDefinition/event-fingerprint", "valueString": e.fingerprint }]},
                "payload": e.payload,
                "event_type": e.event_type.as_str(),
                "severity": e.severity.as_str(),
                "source": e.source
            });

            serde_json::json!({
                "fullUrl": format!("{}/{}", fhir_type, record_key(&e.id)),
                "resource": resource,
                "request": {"method": "POST", "url": fhir_type}
            })
        })
        .collect();

    serde_json::json!({
        "resourceType": "Bundle",
        "type": "history",
        "total": events.len(),
        "entry": entries
    })
}

pub async fn record_admission_event(
    db: &Database,
    patient: &Patient,
    admission_type: &str,
) -> Result<(), Box<surrealdb::Error>> {
    let payload = serde_json::json!({
        "admission_type": admission_type,
        "patient": serde_json::to_value(patient).unwrap_or_default()
    });

    let event = PatientEvent::new(
        patient.patient_id.clone(),
        EventType::Admission,
        payload,
        EventSeverity::Info,
        "admission".to_string(),
    );
    event.store(db).await
}

pub async fn record_discharge_event(
    db: &Database,
    patient_id: &str,
    outcome: &str,
) -> Result<(), Box<surrealdb::Error>> {
    let payload = serde_json::json!({ "outcome": outcome });
    let event = PatientEvent::new(
        patient_id.to_string(),
        EventType::Discharge,
        payload,
        EventSeverity::Info,
        "discharge".to_string(),
    );
    event.store(db).await
}

pub async fn record_alert_event(
    db: &Database,
    patient_id: &str,
    alert_type: &str,
    message: &str,
    severity: EventSeverity,
) -> Result<(), Box<surrealdb::Error>> {
    let payload = serde_json::json!({
        "alert_type": alert_type,
        "message": message
    });
    let event = PatientEvent::new(
        patient_id.to_string(),
        EventType::Alert,
        payload,
        severity,
        "alerting".to_string(),
    );
    event.store(db).await
}
