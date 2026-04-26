#![allow(dead_code)]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::engine::local::Db;
use surrealdb::Surreal;
use uuid::Uuid;

pub const AUDIT_RETENTION_YEARS: i64 = 6;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLog {
    pub id: String,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditQuery {
    pub user_id: Option<String>,
    pub action: Option<AuditAction>,
    pub resource: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub limit: Option<usize>,
}

pub struct AuditService {
    db: Surreal<Db>,
}

impl AuditService {
    pub fn new(db: Surreal<Db>) -> Self {
        Self { db }
    }

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
            id: Uuid::new_v4().to_string(),
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
            .create(("audit_logs", log.id.clone()))
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
            AuditAction::Logout,
            "auth",
            None,
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
        let mut logs: Vec<AuditLog> = self.db.select("audit_logs")
            .await
            .map_err(|e| e.to_string())?;

        if let Some(user_id) = &query.user_id {
            logs.retain(|l| l.user_id.as_ref() == Some(user_id));
        }

        if let Some(action) = &query.action {
            logs.retain(|l| l.action == *action);
        }

        if let Some(resource) = &query.resource {
            logs.retain(|l| l.resource == *resource);
        }

        if let Some(start_date) = &query.start_date {
            logs.retain(|l| l.timestamp >= *start_date);
        }

        if let Some(end_date) = &query.end_date {
            logs.retain(|l| l.timestamp <= *end_date);
        }

        logs.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        if let Some(limit) = query.limit {
            logs.truncate(limit);
        }

        Ok(logs)
    }

    pub async fn get_recent(&self, limit: usize) -> Result<Vec<AuditLog>, String> {
        let logs: Vec<AuditLog> = self.db.select("audit_logs")
            .await
            .map_err(|e| e.to_string())?;

        let mut logs = logs;
        logs.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        logs.truncate(limit);

        Ok(logs)
    }

    pub async fn get_failed_logins(&self, limit: usize) -> Result<Vec<AuditLog>, String> {
        let logs: Vec<AuditLog> = self.db.select("audit_logs")
            .await
            .map_err(|e| e.to_string())?;

        let mut failed: Vec<AuditLog> = logs
            .into_iter()
            .filter(|l| matches!(l.action, AuditAction::LoginFailed))
            .collect();

        failed.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        failed.truncate(limit);

        Ok(failed)
    }

    pub async fn get_critical_events(&self, limit: usize) -> Result<Vec<AuditLog>, String> {
        let logs: Vec<AuditLog> = self.db.select("audit_logs")
            .await
            .map_err(|e| e.to_string())?;

        let mut critical: Vec<AuditLog> = logs
            .into_iter()
            .filter(|l| l.action.is_critical())
            .collect();

        critical.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        critical.truncate(limit);

        Ok(critical)
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
        let logs: Vec<AuditLog> = self.db.select("audit_logs")
            .await
            .map_err(|e| e.to_string())?;

        let cutoff = Utc::now() - chrono::Duration::days(AUDIT_RETENTION_YEARS * 365);
        let mut deleted = 0;

        for log in logs {
            if let Ok(dt) = DateTime::parse_from_rfc3339(&log.timestamp) {
                if dt.with_timezone(&Utc) < cutoff {
                    let _: Option<AuditLog> = self.db.delete(("audit_logs", log.id.clone()))
                        .await
                        .map_err(|e| e.to_string())
                        .ok()
                        .flatten();
                    deleted += 1;
                }
            }
        }

        Ok(deleted)
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