//! Registro de auditoría PHI (HIPAA).
//!
//! El servicio se inicializa en `main()` y las consultas se exponen vía
//! `/api/admin/audit*`. La limpieza por retención (6 años) responde a
//! `POST /api/admin/audit/cleanup` usando `cleanup_old_logs`.
//! Algunos helpers auxiliares permanecen sin uso por ahora.
#![allow(dead_code)]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use surrealdb::Surreal;
use surrealdb::engine::local::Db;
use uuid::Uuid;

pub const AUDIT_RETENTION_YEARS: i64 = 6;

static GLOBAL_AUDIT: OnceLock<AuditService> = OnceLock::new();

pub fn init_global_audit(db: Surreal<Db>) {
    let _ = GLOBAL_AUDIT.set(AuditService::new(db));
}

pub fn audit() -> Option<&'static AuditService> {
    GLOBAL_AUDIT.get()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
        let log = AuditLog {
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
        };

        let created: Option<AuditLog> = self
            .db
            .create(("audit_logs", log.uid.clone()))
            .content(log)
            .await
            .map_err(|e| e.to_string())?;

        created.ok_or_else(|| "Failed to create audit log".to_string())
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
