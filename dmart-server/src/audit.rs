//! Registro de auditoría PHI (HIPAA).
//!
//! El servicio se inicializa en `main()` y las consultas se exponen vía
//! `/api/admin/audit*`. La limpieza por retención (6 años) responde a
//! `POST /api/admin/audit/cleanup` usando `cleanup_old_logs`.
//! Algunos helpers auxiliares permanecen sin uso por ahora.
#![allow(dead_code)]

use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::{Mutex, OnceLock};
use surrealdb::Surreal;
use surrealdb::engine::local::Db;
use uuid::Uuid;

pub const AUDIT_RETENTION_YEARS: i64 = 6;

/// Hash génesis de la cadena de auditoría (64 ceros = SHA-256 vacío lógico).
pub const AUDIT_GENESIS_HASH: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

/// Máximo de eventos por lote firmado (WORM, SPEC-048/3.8).
pub const AUDIT_BATCH_MAX: usize = 1000;

type HmacSha256 = Hmac<Sha256>;

static GLOBAL_AUDIT: OnceLock<AuditService> = OnceLock::new();
static AUDIT_CHAIN: OnceLock<Mutex<ChainState>> = OnceLock::new();

/// Estado en memoria de la cadena de auditoría (se rehidrata en `init_chain`).
#[derive(Debug, Clone)]
pub struct ChainState {
    pub last_hash: String,
    pub tip_batch_hash: String,
    pub initialized: bool,
}

fn chain() -> &'static Mutex<ChainState> {
    AUDIT_CHAIN.get_or_init(|| {
        Mutex::new(ChainState {
            last_hash: AUDIT_GENESIS_HASH.to_string(),
            tip_batch_hash: AUDIT_GENESIS_HASH.to_string(),
            initialized: false,
        })
    })
}

fn to_hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(out, "{:02x}", b);
    }
    out
}

fn sha256_hex(data: &[u8]) -> String {
    to_hex(&Sha256::digest(data))
}

fn audit_signing_key() -> [u8; 32] {
    let secret = std::env::var("DMART_AUDIT_HMAC_KEY")
        .or_else(|_| std::env::var("DMART_MASTER_KEY"))
        .unwrap_or_else(|_| "dmart-dev-audit-hmac-key-000000000000".to_string());
    Sha256::digest(secret.as_bytes()).into()
}

fn hmac_sign(message: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(&audit_signing_key())
        .expect("HMAC acepta claves de cualquier longitud");
    mac.update(message.as_bytes());
    to_hex(&mac.finalize().into_bytes())
}

fn opt_marker(value: &Option<String>) -> String {
    value.clone().unwrap_or_else(|| "-".to_string())
}

/// Representación canónica y determinista de un evento (orden fijo de campos).
fn canonical_log_payload(log: &AuditLog) -> String {
    format!(
        "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
        log.uid,
        log.timestamp,
        log.action.as_str(),
        log.resource,
        opt_marker(&log.resource_id),
        opt_marker(&log.user_id),
        opt_marker(&log.username),
        opt_marker(&log.details),
        opt_marker(&log.ip_address),
        opt_marker(&log.user_agent),
        log.success,
        opt_marker(&log.error_message),
    )
}

fn compute_content_hash(log: &AuditLog, prev_hash: &str) -> String {
    sha256_hex(format!("{}|{}", prev_hash, canonical_log_payload(log)).as_bytes())
}

/// Hash efectivo para sellado (usa el almacenado o lo deriva para registros legados).
fn effective_content_hash(log: &AuditLog) -> String {
    match &log.content_hash {
        Some(h) => h.clone(),
        None => sha256_hex(canonical_log_payload(log).as_bytes()),
    }
}

pub fn init_global_audit(db: Surreal<Db>) {
    let _ = GLOBAL_AUDIT.set(AuditService::new(db));
}

pub fn audit() -> Option<&'static AuditService> {
    GLOBAL_AUDIT.get()
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AuditLog {
    pub uid: String,
    pub timestamp: String,
    pub user_id: Option<String>,
    pub username: Option<String>,
    pub action: AuditAction,
    pub resource: String,
    pub resource_id: Option<String>,
    pub details: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub success: bool,
    pub error_message: Option<String>,
    /// Hash del evento anterior en la cadena (WORM, SPEC-048/3.8).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prev_hash: Option<String>,
    /// SHA-256 canónico del evento encadenado a `prev_hash`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
}

/// Lote de eventos de auditoría sellado e inmutable (WORM).
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AuditBatch {
    pub batch_id: String,
    pub sequence: u64,
    pub first_uid: String,
    pub last_uid: String,
    pub count: u64,
    pub first_ts: String,
    pub last_ts: String,
    pub prev_batch_hash: String,
    pub batch_hash: String,
    pub signature: String,
    pub created_at: String,
}

/// Resultado de la verificación de integridad de la cadena de auditoría.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct IntegrityReport {
    pub logs_total: usize,
    pub logs_hashed: usize,
    pub logs_valid: usize,
    pub logs_unhashed: usize,
    pub batches_total: usize,
    pub signatures_valid: usize,
    pub chain_valid: bool,
    pub head_batch_hash: String,
    pub sealable_logs: usize,
    pub ok: bool,
}

/// Export de lectura para auditoría externa (logs + lotes firmados).
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AuditExport {
    pub generated_at: String,
    pub retention_years: i64,
    pub head_batch_hash: String,
    pub logs: Vec<AuditLog>,
    pub batches: Vec<AuditBatch>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub enum AuditAction {
    Login,
    Logout,
    LoginFailed,
    LogoutFailed,
    Create,
    Read,
    Update,
    Delete,
    Export,
    ConfigChange,
    AuthChange,
    AccessDenied,
    DataAccess,
    DataModification,
}

impl AuditAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            AuditAction::Login => "LOGIN",
            AuditAction::Logout => "LOGOUT",
            AuditAction::LoginFailed => "LOGIN_FAILED",
            AuditAction::LogoutFailed => "LOGOUT_FAILED",
            AuditAction::Create => "CREATE",
            AuditAction::Read => "READ",
            AuditAction::Update => "UPDATE",
            AuditAction::Delete => "DELETE",
            AuditAction::Export => "EXPORT",
            AuditAction::ConfigChange => "CONFIG_CHANGE",
            AuditAction::AuthChange => "AUTH_CHANGE",
            AuditAction::AccessDenied => "ACCESS_DENIED",
            AuditAction::DataAccess => "DATA_ACCESS",
            AuditAction::DataModification => "DATA_MODIFICATION",
        }
    }

    pub fn is_critical(&self) -> bool {
        matches!(
            self,
            AuditAction::LoginFailed
                | AuditAction::LogoutFailed
                | AuditAction::Delete
                | AuditAction::ConfigChange
                | AuditAction::AuthChange
                | AuditAction::AccessDenied
        )
    }

    pub fn is_phi_related(&self) -> bool {
        matches!(
            self,
            AuditAction::Read
                | AuditAction::Create
                | AuditAction::Update
                | AuditAction::Delete
                | AuditAction::DataAccess
                | AuditAction::DataModification
        )
    }
}

impl std::str::FromStr for AuditAction {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "LOGIN" => Ok(AuditAction::Login),
            "LOGOUT" => Ok(AuditAction::Logout),
            "LOGIN_FAILED" => Ok(AuditAction::LoginFailed),
            "LOGOUT_FAILED" => Ok(AuditAction::LogoutFailed),
            "CREATE" => Ok(AuditAction::Create),
            "READ" => Ok(AuditAction::Read),
            "UPDATE" => Ok(AuditAction::Update),
            "DELETE" => Ok(AuditAction::Delete),
            "EXPORT" => Ok(AuditAction::Export),
            "CONFIG_CHANGE" => Ok(AuditAction::ConfigChange),
            "AUTH_CHANGE" => Ok(AuditAction::AuthChange),
            "ACCESS_DENIED" => Ok(AuditAction::AccessDenied),
            "DATA_ACCESS" => Ok(AuditAction::DataAccess),
            "DATA_MODIFICATION" => Ok(AuditAction::DataModification),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditQuery {
    pub user_id: Option<String>,
    pub action: Option<AuditAction>,
    pub resource: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Clone)]
pub struct AuditService {
    db: Surreal<Db>,
}

impl AuditService {
    pub fn new(db: Surreal<Db>) -> Self {
        Self { db }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn log(
        &self,
        action: AuditAction,
        resource: &str,
        resource_id: Option<&str>,
        user_id: Option<&str>,
        username: Option<&str>,
        details: Option<&str>,
        ip_address: Option<&str>,
        user_agent: Option<&str>,
        success: bool,
        error_message: Option<&str>,
    ) -> Result<AuditLog, String> {
        let prev_hash = {
            let guard = chain().lock().expect("audit chain lock");
            guard.last_hash.clone()
        };

        let mut log = AuditLog {
            uid: Uuid::new_v4().to_string(),
            timestamp: Utc::now().to_rfc3339(),
            user_id: user_id.map(String::from),
            username: username.map(String::from),
            action,
            resource: resource.to_string(),
            resource_id: resource_id.map(String::from),
            details: details.map(String::from),
            ip_address: ip_address.map(String::from),
            user_agent: user_agent.map(String::from),
            success,
            error_message: error_message.map(String::from),
            prev_hash: Some(prev_hash.clone()),
            content_hash: None,
        };
        let content = compute_content_hash(&log, &prev_hash);
        log.content_hash = Some(content.clone());

        {
            let mut guard = chain().lock().expect("audit chain lock");
            guard.last_hash = content.clone();
            guard.initialized = true;
        }

        let created: Option<AuditLog> = self
            .db
            .create(("audit_logs", log.uid.clone()))
            .content(log)
            .await
            .map_err(|e| {
                let mut guard = chain().lock().expect("audit chain lock");
                if guard.last_hash == content {
                    guard.last_hash = prev_hash.clone();
                }
                crate::support::note("audit", false);
                e.to_string()
            })?;

        crate::support::note("audit", true);
        created.ok_or_else(|| {
            crate::support::note("audit", false);
            "Failed to create audit log".to_string()
        })
    }

    pub async fn log_login_success(
        &self,
        user_id: &str,
        username: &str,
        ip_address: Option<&str>,
    ) -> Result<AuditLog, String> {
        self.log(
            AuditAction::Login,
            "auth",
            None,
            Some(user_id),
            Some(username),
            None,
            ip_address,
            None,
            true,
            None,
        )
        .await
    }

    pub async fn log_login_failed(
        &self,
        username: &str,
        reason: &str,
        ip_address: Option<&str>,
    ) -> Result<AuditLog, String> {
        self.log(
            AuditAction::LoginFailed,
            "auth",
            None,
            None,
            Some(username),
            Some(reason),
            ip_address,
            None,
            false,
            Some(reason),
        )
        .await
    }

    pub async fn log_logout(&self, user_id: &str, username: &str) -> Result<AuditLog, String> {
        self.log(
            AuditAction::AuthChange,
            "auth/logout",
            None,
            Some(user_id),
            Some(username),
            Some("logout"),
            None,
            None,
            true,
            None,
        )
        .await
    }

    /// Registra un evento de sistema (p.ej. impersonación de tenant, SPEC-025).
    pub async fn log_system_event(
        &self,
        user_id: &str,
        username: &str,
        resource: &str,
        details: &str,
    ) -> Result<AuditLog, String> {
        self.log(
            AuditAction::ConfigChange,
            resource,
            None,
            Some(user_id),
            Some(username),
            Some(details),
            None,
            None,
            true,
            None,
        )
        .await
    }

    pub async fn log_patient_access(
        &self,
        user_id: &str,
        username: &str,
        patient_id: &str,
        action: AuditAction,
        success: bool,
    ) -> Result<AuditLog, String> {
        self.log(
            action,
            "patients",
            Some(patient_id),
            Some(user_id),
            Some(username),
            None,
            None,
            None,
            success,
            None,
        )
        .await
    }

    pub async fn log_measurement_access(
        &self,
        user_id: &str,
        username: &str,
        patient_id: &str,
        action: AuditAction,
    ) -> Result<AuditLog, String> {
        self.log(
            action,
            "measurements",
            Some(patient_id),
            Some(user_id),
            Some(username),
            None,
            None,
            None,
            true,
            None,
        )
        .await
    }

    pub async fn log_access_denied(
        &self,
        user_id: Option<&str>,
        username: Option<&str>,
        resource: &str,
        reason: &str,
        ip_address: Option<&str>,
    ) -> Result<AuditLog, String> {
        self.log(
            AuditAction::AccessDenied,
            resource,
            None,
            user_id,
            username,
            Some(reason),
            ip_address,
            None,
            false,
            Some(reason),
        )
        .await
    }

    pub async fn log_export(
        &self,
        user_id: &str,
        username: &str,
        resource: &str,
        format: &str,
    ) -> Result<AuditLog, String> {
        self.log(
            AuditAction::Export,
            resource,
            None,
            Some(user_id),
            Some(username),
            Some(format),
            None,
            None,
            true,
            None,
        )
        .await
    }

    pub async fn query(&self, query: AuditQuery) -> Result<Vec<AuditLog>, String> {
        let mut sql = String::from("SELECT * FROM audit_logs WHERE true");
        if query.user_id.is_some() {
            sql.push_str(" AND user_id = $user_id");
        }
        if query.action.is_some() {
            sql.push_str(" AND action = $action");
        }
        if query.resource.is_some() {
            sql.push_str(" AND resource = $resource");
        }
        if query.start_date.is_some() {
            sql.push_str(" AND timestamp >= $start_date");
        }
        if query.end_date.is_some() {
            sql.push_str(" AND timestamp <= $end_date");
        }
        sql.push_str(" ORDER BY timestamp DESC");
        if query.limit.is_some() {
            sql.push_str(" LIMIT $limit");
        }

        let mut s = self.db.query(sql);
        if let Some(user_id) = &query.user_id {
            s = s.bind(("user_id", user_id.clone()));
        }
        if let Some(action) = &query.action {
            s = s.bind(("action", *action));
        }
        if let Some(resource) = &query.resource {
            s = s.bind(("resource", resource.clone()));
        }
        if let Some(start_date) = &query.start_date {
            s = s.bind(("start_date", start_date.clone()));
        }
        if let Some(end_date) = &query.end_date {
            s = s.bind(("end_date", end_date.clone()));
        }
        if let Some(limit) = query.limit {
            s = s.bind(("limit", limit as i64));
        }

        let logs: Vec<AuditLog> = s
            .await
            .map_err(|e| e.to_string())?
            .take(0)
            .map_err(|e| e.to_string())?;
        Ok(logs)
    }

    pub async fn get_recent(&self, limit: usize) -> Result<Vec<AuditLog>, String> {
        let logs: Vec<AuditLog> = self
            .db
            .query("SELECT * FROM audit_logs ORDER BY timestamp DESC LIMIT $limit")
            .bind(("limit", limit as i64))
            .await
            .map_err(|e| e.to_string())?
            .take(0)
            .map_err(|e| e.to_string())?;
        Ok(logs)
    }

    pub async fn get_failed_logins(&self, limit: usize) -> Result<Vec<AuditLog>, String> {
        let logs: Vec<AuditLog> = self
            .db
            .query("SELECT * FROM audit_logs WHERE action = $action ORDER BY timestamp DESC LIMIT $limit")
            .bind(("action", AuditAction::LoginFailed))
            .bind(("limit", limit as i64))
            .await
            .map_err(|e| e.to_string())?
            .take(0)
            .map_err(|e| e.to_string())?;
        Ok(logs)
    }

    pub async fn get_critical_events(&self, limit: usize) -> Result<Vec<AuditLog>, String> {
        let critical: Vec<AuditAction> = vec![
            AuditAction::LoginFailed,
            AuditAction::LogoutFailed,
            AuditAction::Delete,
            AuditAction::ConfigChange,
            AuditAction::AuthChange,
            AuditAction::AccessDenied,
        ];
        let logs: Vec<AuditLog> = self
            .db
            .query("SELECT * FROM audit_logs WHERE action IN $critical ORDER BY timestamp DESC LIMIT $limit")
            .bind(("critical", critical))
            .bind(("limit", limit as i64))
            .await
            .map_err(|e| e.to_string())?
            .take(0)
            .map_err(|e| e.to_string())?;
        Ok(logs)
    }

    pub async fn get_retention_days(&self) -> i64 {
        AUDIT_RETENTION_YEARS * 365
    }

    pub async fn should_retain(&self, timestamp: &str) -> bool {
        if let Ok(dt) = DateTime::parse_from_rfc3339(timestamp) {
            let cutoff = Utc::now() - chrono::Duration::days(AUDIT_RETENTION_YEARS * 365);
            dt.with_timezone(&Utc) > cutoff
        } else {
            true
        }
    }

    pub async fn cleanup_old_logs(&self) -> Result<usize, String> {
        let cutoff =
            (Utc::now() - chrono::Duration::days(AUDIT_RETENTION_YEARS * 365)).to_rfc3339();
        // Contar cuántos se van a borrar
        let count: Vec<serde_json::Value> = self
            .db
            .query("SELECT count() FROM audit_logs WHERE timestamp < $cutoff")
            .bind(("cutoff", cutoff.clone()))
            .await
            .map_err(|e| e.to_string())?
            .take(0)
            .map_err(|e| e.to_string())?;
        let to_delete = count
            .first()
            .and_then(|v| v.get("count"))
            .and_then(|c| c.as_u64())
            .unwrap_or(0) as usize;

        if to_delete > 0 {
            self.db
                .query("DELETE FROM audit_logs WHERE timestamp < $cutoff")
                .bind(("cutoff", cutoff))
                .await
                .map_err(|e| e.to_string())?;
        }
        Ok(to_delete)
    }

    /// Rehidrata la cadena desde la BD (último evento + último lote sellado).
    pub async fn init_chain(&self) {
        let last_logs: Vec<AuditLog> = self
            .db
            .query("SELECT * FROM audit_logs ORDER BY timestamp DESC LIMIT 1")
            .await
            .ok()
            .and_then(|mut r| r.take(0).ok())
            .unwrap_or_default();
        let last_batches: Vec<AuditBatch> = self
            .db
            .query("SELECT * FROM audit_batches ORDER BY sequence DESC LIMIT 1")
            .await
            .ok()
            .and_then(|mut r| r.take(0).ok())
            .unwrap_or_default();

        let mut guard = chain().lock().expect("audit chain lock");
        if let Some(h) = last_logs.first().and_then(|l| l.content_hash.clone()) {
            guard.last_hash = h;
        }
        if let Some(b) = last_batches.first() {
            guard.tip_batch_hash = b.batch_hash.clone();
        }
        guard.initialized = true;
    }

    async fn last_batch(&self) -> Result<Option<AuditBatch>, String> {
        let batches: Vec<AuditBatch> = self
            .db
            .query("SELECT * FROM audit_batches ORDER BY sequence DESC LIMIT 1")
            .await
            .map_err(|e| e.to_string())?
            .take(0)
            .map_err(|e| e.to_string())?;
        Ok(batches.into_iter().next())
    }

    /// Sella un lote inmutable con encadenamiento y firma HMAC-SHA256.
    pub async fn seal_batch(&self, limit: usize) -> Result<Option<AuditBatch>, String> {
        let limit = limit.clamp(1, AUDIT_BATCH_MAX);
        let prev = self.last_batch().await?;
        let (prev_seq, prev_hash, after_ts) = match &prev {
            Some(b) => (b.sequence, b.batch_hash.clone(), b.last_ts.clone()),
            None => (0u64, AUDIT_GENESIS_HASH.to_string(), String::new()),
        };

        let logs: Vec<AuditLog> = if after_ts.is_empty() {
            self.db
                .query("SELECT * FROM audit_logs ORDER BY timestamp ASC LIMIT $limit")
                .bind(("limit", limit as i64))
                .await
                .map_err(|e| e.to_string())?
                .take(0)
                .map_err(|e| e.to_string())?
        } else {
            self.db
                .query("SELECT * FROM audit_logs WHERE timestamp > $after ORDER BY timestamp ASC LIMIT $limit")
                .bind(("after", after_ts.clone()))
                .bind(("limit", limit as i64))
                .await
                .map_err(|e| e.to_string())?
                .take(0)
                .map_err(|e| e.to_string())?
        };
        if logs.is_empty() {
            return Ok(None);
        }

        let first = logs.first().expect("no vacío");
        let last = logs.last().expect("no vacío");
        let concat: String = logs.iter().map(effective_content_hash).collect();
        let sequence = prev_seq + 1;
        let material = format!(
            "{}|{}|{}|{}|{}|{}|{}|{}",
            prev_hash,
            sequence,
            logs.len(),
            first.uid,
            last.uid,
            first.timestamp,
            last.timestamp,
            concat,
        );
        let batch_hash = sha256_hex(material.as_bytes());
        let signature = hmac_sign(&batch_hash);
        let batch = AuditBatch {
            batch_id: Uuid::new_v4().to_string(),
            sequence,
            first_uid: first.uid.clone(),
            last_uid: last.uid.clone(),
            count: logs.len() as u64,
            first_ts: first.timestamp.clone(),
            last_ts: last.timestamp.clone(),
            prev_batch_hash: prev_hash,
            batch_hash: batch_hash.clone(),
            signature,
            created_at: Utc::now().to_rfc3339(),
        };

        let _: Option<AuditBatch> = self
            .db
            .create(("audit_batches", batch.batch_id.clone()))
            .content(batch.clone())
            .await
            .map_err(|e| e.to_string())?;

        {
            let mut guard = chain().lock().expect("audit chain lock");
            guard.tip_batch_hash = batch_hash;
        }
        crate::support::note("audit", true);
        Ok(Some(batch))
    }

    /// Verifica integridad de la cadena de eventos, lotes y firmas.
    pub async fn verify_integrity(&self) -> Result<IntegrityReport, String> {
        let logs: Vec<AuditLog> = self
            .db
            .query("SELECT * FROM audit_logs ORDER BY timestamp ASC")
            .await
            .map_err(|e| e.to_string())?
            .take(0)
            .map_err(|e| e.to_string())?;
        let batches: Vec<AuditBatch> = self
            .db
            .query("SELECT * FROM audit_batches ORDER BY sequence ASC")
            .await
            .map_err(|e| e.to_string())?
            .take(0)
            .map_err(|e| e.to_string())?;

        let mut logs_hashed = 0usize;
        let mut logs_valid = 0usize;
        let mut logs_unhashed = 0usize;
        let mut expected_prev = AUDIT_GENESIS_HASH.to_string();
        let mut prev_was_hashed = false;
        let mut chain_valid = true;
        for log in &logs {
            match &log.content_hash {
                Some(stored) => {
                    let prev = log.prev_hash.as_deref().unwrap_or(AUDIT_GENESIS_HASH);
                    let recomputed = compute_content_hash(log, prev);
                    logs_hashed += 1;
                    if &recomputed == stored {
                        logs_valid += 1;
                    } else {
                        chain_valid = false;
                    }
                    if prev_was_hashed && prev != expected_prev {
                        chain_valid = false;
                    }
                    expected_prev = stored.clone();
                    prev_was_hashed = true;
                }
                None => {
                    logs_unhashed += 1;
                    prev_was_hashed = false;
                }
            }
        }

        let mut signatures_valid = 0usize;
        let mut prev_batch_hash = AUDIT_GENESIS_HASH.to_string();
        for batch in &batches {
            if batch.prev_batch_hash == prev_batch_hash
                && hmac_sign(&batch.batch_hash) == batch.signature
            {
                signatures_valid += 1;
            } else {
                chain_valid = false;
            }
            prev_batch_hash = batch.batch_hash.clone();
        }

        let sealed: u64 = batches.iter().map(|b| b.count).sum();
        let sealable_logs = (logs.len() as u64).saturating_sub(sealed) as usize;
        let head_batch_hash = batches
            .last()
            .map(|b| b.batch_hash.clone())
            .unwrap_or_else(|| AUDIT_GENESIS_HASH.to_string());
        let ok = chain_valid
            && logs_valid == logs_total_hashed(&logs)
            && signatures_valid == batches.len();

        Ok(IntegrityReport {
            logs_total: logs.len(),
            logs_hashed,
            logs_valid,
            logs_unhashed,
            batches_total: batches.len(),
            signatures_valid,
            chain_valid,
            head_batch_hash,
            sealable_logs,
            ok,
        })
    }

    /// Export de lectura (logs + lotes firmados) para verificación externa.
    pub async fn export(&self, limit: usize) -> Result<AuditExport, String> {
        let logs: Vec<AuditLog> = self
            .db
            .query("SELECT * FROM audit_logs ORDER BY timestamp ASC LIMIT $limit")
            .bind(("limit", limit as i64))
            .await
            .map_err(|e| e.to_string())?
            .take(0)
            .map_err(|e| e.to_string())?;
        let batches: Vec<AuditBatch> = self
            .db
            .query("SELECT * FROM audit_batches ORDER BY sequence ASC")
            .await
            .map_err(|e| e.to_string())?
            .take(0)
            .map_err(|e| e.to_string())?;
        let head_batch_hash = batches
            .last()
            .map(|b| b.batch_hash.clone())
            .unwrap_or_else(|| AUDIT_GENESIS_HASH.to_string());
        Ok(AuditExport {
            generated_at: Utc::now().to_rfc3339(),
            retention_years: AUDIT_RETENTION_YEARS,
            head_batch_hash,
            logs,
            batches,
        })
    }
}

fn logs_total_hashed(logs: &[AuditLog]) -> usize {
    logs.iter().filter(|l| l.content_hash.is_some()).count()
}

pub mod macros {
    #[macro_export]
    macro_rules! audit_login {
        ($service:expr_2021, $user_id:expr_2021, $username:expr_2021, $($args:tt)*) => {
            $service.log_login_success($user_id, $username, None $(, $args)*).await
        };
    }

    #[macro_export]
    macro_rules! audit_login_failed {
        ($service:expr_2021, $username:expr_2021, $reason:expr_2021, $($args:tt)*) => {
            $service.log_login_failed($username, $reason, None $(, $args)*).await
        };
    }

    #[macro_export]
    macro_rules! audit_patient {
        ($service:expr_2021, $user_id:expr_2021, $username:expr_2021, $patient_id:expr_2021, $action:expr_2021 $(, $success:expr_2021)?) => {
            $service.log_patient_access($user_id, $username, $patient_id, $action, true $(, $success)?).await
        };
    }
}
