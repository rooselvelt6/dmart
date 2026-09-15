//! SPEC-018: Data Quality — validación de calidad del dato vital.
//!
//! Correlaciona la calidad de cada `VitalsMessage`: valida el rango fisiológico
//! de cada signo vital, rechaza valores no finitos, detecta timestamps futuros
//! o demasiado antiguos, exige `patient_ref` y controla el orden de
//! `sequence_number` por sender. Los issues detectados se persisten en la tabla
//! `quality_events` (append-only) y son consultables mediante reporte y
//! agregación por severidad/tipo.

use crate::db::Database;
use crate::hl7::parser::VitalsMessage;
use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::sync::atomic::{AtomicI64, Ordering};
use uuid::Uuid;

/// Severidad de un issue de calidad.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IssueSeverity {
    Low,
    Medium,
    High,
}

impl IssueSeverity {
    pub fn as_str(self) -> &'static str {
        match self {
            IssueSeverity::Low => "low",
            IssueSeverity::Medium => "medium",
            IssueSeverity::High => "high",
        }
    }
}

/// Códigos estables de issue (consultables vía el campo `code`).
pub mod codes {
    pub const VITAL_OUT_OF_RANGE: &str = "VITAL_OUT_OF_RANGE";
    pub const VITAL_NON_FINITE: &str = "VITAL_NON_FINITE";
    pub const TIMESTAMP_FUTURE: &str = "TIMESTAMP_FUTURE";
    pub const TIMESTAMP_STALE: &str = "TIMESTAMP_STALE";
    pub const PATIENT_REF_EMPTY: &str = "PATIENT_REF_EMPTY";
    pub const SEQUENCE_OUT_OF_ORDER: &str = "SEQUENCE_OUT_OF_ORDER";
}

/// Issue de calidad persistido en `quality_events`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityIssue {
    pub id: surrealdb::RecordId,
    pub message_id: String,
    pub patient_ref: String,
    pub severity: String,
    pub code: String,
    pub detail: String,
    pub timestamp: String,
}

const STALE_SECS: i64 = 600;
const MAX_REPORT_LIMIT: u32 = 500;

static LAST_ISSUE_TS_MS: AtomicI64 = AtomicI64::new(0);

/// RFC 3339 estrictamente creciente por proceso: garantiza un orden estable en
/// `quality_report` incluso con issues creados en el mismo milisegundo.
fn next_issue_timestamp() -> String {
    let now = Utc::now().timestamp_millis();
    loop {
        let last = LAST_ISSUE_TS_MS.load(Ordering::Relaxed);
        let next = now.max(last + 1);
        if LAST_ISSUE_TS_MS
            .compare_exchange_weak(last, next, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
        {
            return DateTime::from_timestamp_millis(next)
                .unwrap_or_else(Utc::now)
                .to_rfc3339_opts(SecondsFormat::Millis, true);
        }
    }
}

impl QualityIssue {
    pub fn new(
        message_id: String,
        patient_ref: String,
        severity: IssueSeverity,
        code: &str,
        detail: String,
    ) -> Self {
        Self {
            id: surrealdb::RecordId::from(("quality_events", Uuid::new_v4().to_string())),
            message_id,
            patient_ref,
            severity: severity.as_str().to_string(),
            code: code.to_string(),
            detail,
            timestamp: next_issue_timestamp(),
        }
    }

    pub async fn store(&self, db: &Database) -> Result<(), Box<surrealdb::Error>> {
        db.query(
            r#"
            CREATE quality_events CONTENT {
                id: $id,
                message_id: $message_id,
                patient_ref: $patient_ref,
                severity: $severity,
                code: $code,
                detail: $detail,
                timestamp: $timestamp
            }
            "#,
        )
        .bind(("id", self.id.clone()))
        .bind(("message_id", self.message_id.clone()))
        .bind(("patient_ref", self.patient_ref.clone()))
        .bind(("severity", self.severity.clone()))
        .bind(("code", self.code.clone()))
        .bind(("detail", self.detail.clone()))
        .bind(("timestamp", self.timestamp.clone()))
        .await?;
        Ok(())
    }
}

/// Rango fisiológico [min, max] por signo vital (resuelto por LOINC y, como
/// rebote, por nombre). Aplica a valores finitos; `None` si no se reconoce.
fn vital_bounds(loinc: Option<&str>, name: &str) -> Option<(f32, f32)> {
    let code = loinc.unwrap_or("");
    match code {
        "8867-4" => Some((20.0, 250.0)), // HR bpm
        "2708-6" => Some((50.0, 100.0)), // SpO2 %
        "8310-5" => Some((30.0, 45.0)),  // Temp °C
        "9279-1" => Some((4.0, 80.0)),   // RR /min
        "8480-6" => Some((40.0, 300.0)), // SBP mmHg
        "8460-8" => Some((20.0, 200.0)), // DBP mmHg
        "8478-0" => Some((40.0, 200.0)), // MAP mmHg
        _ => {
            let hay = format!("{} {}", code, name).to_lowercase();
            let (min, max) = if hay.contains("heart rate") || hay.contains("frecuencia card") {
                (20.0, 250.0)
            } else if hay.contains("spo2") || hay.contains("saturacion") {
                (50.0, 100.0)
            } else if hay.contains("temperat") {
                (30.0, 45.0)
            } else if hay.contains("respirat") {
                (4.0, 80.0)
            } else if hay.contains("sistol") || hay.contains("systolic") {
                (40.0, 300.0)
            } else if hay.contains("diastol") || hay.contains("diastolic") {
                (20.0, 200.0)
            } else if hay.contains("arterial media") || hay.contains("mean arterial") {
                (40.0, 200.0)
            } else {
                return None;
            };
            Some((min, max))
        }
    }
}

fn check_timestamp(msg: &VitalsMessage, issues: &mut Vec<QualityIssue>) {
    let Ok(ts) = DateTime::parse_from_rfc3339(&msg.timestamp) else {
        return;
    };
    let ts_utc = ts.with_timezone(&Utc);
    let now = Utc::now();
    let age_secs = (now - ts_utc).num_seconds();
    if ts_utc > now {
        issues.push(QualityIssue::new(
            msg.message_id.clone(),
            msg.patient_ref.clone(),
            IssueSeverity::Medium,
            codes::TIMESTAMP_FUTURE,
            format!(
                "timestamp {} está en el futuro ({}s adelantado)",
                msg.timestamp, -age_secs
            ),
        ));
    } else if age_secs > STALE_SECS {
        issues.push(QualityIssue::new(
            msg.message_id.clone(),
            msg.patient_ref.clone(),
            IssueSeverity::Medium,
            codes::TIMESTAMP_STALE,
            format!(
                "timestamp {} demasiado viejo ({}s)",
                msg.timestamp, age_secs
            ),
        ));
    }
}

static LAST_SEQUENCE: LazyLock<Mutex<HashMap<String, u32>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Registro en proceso del último `sequence_number` por sender. Un valor
/// duplicado o retrocediendo respecto al último conocido genera un issue medio
/// (posible pérdida/duplicación de mensajes del monitor).
fn check_sequence(
    sender: &str,
    message_id: &str,
    patient_ref: &str,
    seq: u32,
    issues: &mut Vec<QualityIssue>,
) {
    let mut last_seqs = LAST_SEQUENCE.lock().expect("sequence map poisoned");
    match last_seqs.get(sender) {
        Some(&last) if seq <= last => {
            issues.push(QualityIssue::new(
                message_id.to_string(),
                patient_ref.to_string(),
                IssueSeverity::Medium,
                codes::SEQUENCE_OUT_OF_ORDER,
                format!("sequence_number {seq} duplicado o retrocediendo (último conocido {last})"),
            ));
        }
        _ => {
            last_seqs.insert(sender.to_string(), seq);
        }
    }
}

/// Reinicia el tracker de secuencias (útil en tests e ingesta de procesos).
pub fn reset_sequence_tracker() {
    LAST_SEQUENCE.lock().expect("sequence map poisoned").clear();
}

/// Corre los validadores de calidad sobre un `VitalsMessage` y devuelve los
/// issues detectados (sin persistir).
pub fn validate_vitals_message(msg: &VitalsMessage) -> Vec<QualityIssue> {
    let mut issues = Vec::new();

    if msg.patient_ref.trim().is_empty() {
        issues.push(QualityIssue::new(
            msg.message_id.clone(),
            msg.patient_ref.clone(),
            IssueSeverity::High,
            codes::PATIENT_REF_EMPTY,
            "patient_ref vacío o solo espacios".to_string(),
        ));
    }

    for vital in &msg.vitals {
        if !vital.value.is_finite() {
            issues.push(QualityIssue::new(
                msg.message_id.clone(),
                msg.patient_ref.clone(),
                IssueSeverity::High,
                codes::VITAL_NON_FINITE,
                format!("{} value no finito: {}", vital.name, vital.value),
            ));
        } else if let Some((min, max)) = vital_bounds(vital.loinc.as_deref(), &vital.name)
            && (vital.value < min || vital.value > max)
        {
            issues.push(QualityIssue::new(
                msg.message_id.clone(),
                msg.patient_ref.clone(),
                IssueSeverity::High,
                codes::VITAL_OUT_OF_RANGE,
                format!(
                    "{} = {}{} fuera de rango fisiológico [{}, {}]",
                    vital.name, vital.value, vital.unit, min, max
                ),
            ));
        }
    }

    check_timestamp(msg, &mut issues);

    if let Some(seq) = msg.sequence_number {
        check_sequence(
            &msg.sender,
            &msg.message_id,
            &msg.patient_ref,
            seq,
            &mut issues,
        );
    }

    issues
}

/// Valida el mensaje y persiste los issues en `quality_events`.
pub async fn validate_and_store(
    db: &Database,
    msg: &VitalsMessage,
) -> Result<Vec<QualityIssue>, Box<surrealdb::Error>> {
    let issues = validate_vitals_message(msg);
    for issue in &issues {
        issue.store(db).await?;
    }
    Ok(issues)
}

/// Parámetros de `quality_report`: filtros opcionales por `severity`/`code` y
/// paginación con `limit` (por defecto 100, máx. 500) + `offset`.
#[derive(Debug, Clone, Default)]
pub struct ReportQuery {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub severity: Option<String>,
    pub code: Option<String>,
}

/// Últimos issues ordenados por `timestamp` desc.
pub async fn quality_report(
    db: &Database,
    query: ReportQuery,
) -> Result<Vec<QualityIssue>, Box<surrealdb::Error>> {
    let limit = (query.limit.unwrap_or(100)).min(MAX_REPORT_LIMIT) as i64;
    let offset = query.offset.unwrap_or(0) as i64;

    let mut conditions: Vec<&str> = Vec::new();
    if query.severity.is_some() {
        conditions.push("severity = $severity");
    }
    if query.code.is_some() {
        conditions.push("code = $code");
    }
    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", conditions.join(" AND "))
    };
    let sql = format!(
        "SELECT * FROM quality_events{where_clause} \
         ORDER BY timestamp DESC LIMIT $limit START $offset"
    );

    let mut q = db
        .query(&sql)
        .bind(("limit", limit))
        .bind(("offset", offset));
    if let Some(severity) = query.severity {
        q = q.bind(("severity", severity));
    }
    if let Some(code) = query.code {
        q = q.bind(("code", code));
    }
    let issues: Result<Vec<QualityIssue>, _> = q.await?.take(0);
    Ok(issues.unwrap_or_default())
}

/// Fila de agregación por severidad.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SeverityCount {
    pub severity: String,
    pub count: u64,
}

/// Fila de agregación por código de issue.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CodeCount {
    pub code: String,
    pub count: u64,
}

/// Agregado de issues por severidad y por tipo/código.
#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct QualitySummary {
    pub total: u64,
    pub by_severity: Vec<SeverityCount>,
    pub by_code: Vec<CodeCount>,
}

/// Agregaciones `GROUP BY` sobre `quality_events` por severidad y por código.
pub async fn quality_summary(db: &Database) -> Result<QualitySummary, Box<surrealdb::Error>> {
    let by_severity: Vec<serde_json::Value> = db
        .query("SELECT severity, count() AS count FROM quality_events GROUP BY severity")
        .await?
        .take(0)?;
    let by_code: Vec<serde_json::Value> = db
        .query(
            "SELECT code, count() AS count FROM quality_events GROUP BY code ORDER BY count DESC",
        )
        .await?
        .take(0)?;

    let mut summary = QualitySummary::default();
    for row in by_severity {
        let (Some(sev), Some(count)) = (
            row.get("severity").and_then(|v| v.as_str()),
            row.get("count").and_then(|v| v.as_u64()),
        ) else {
            continue;
        };
        summary.total += count;
        summary.by_severity.push(SeverityCount {
            severity: sev.to_string(),
            count,
        });
    }
    for row in by_code {
        let (Some(code), Some(count)) = (
            row.get("code").and_then(|v| v.as_str()),
            row.get("count").and_then(|v| v.as_u64()),
        ) else {
            continue;
        };
        summary.by_code.push(CodeCount {
            code: code.to_string(),
            count,
        });
    }
    summary
        .by_severity
        .sort_by_key(|a| std::cmp::Reverse(a.count));
    Ok(summary)
}
