use dmart_shared::models::UserRole;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    Admin,
    Doctor,
    Nurse,
    Viewer,
}

impl From<UserRole> for Role {
    fn from(rol: UserRole) -> Self {
        match rol {
            UserRole::Admin => Role::Admin,
            UserRole::Medico => Role::Doctor,
            UserRole::Enfermero => Role::Nurse,
            UserRole::Viewer => Role::Viewer,
        }
    }
}

impl From<Role> for UserRole {
    fn from(rol: Role) -> Self {
        match rol {
            Role::Admin => UserRole::Admin,
            Role::Doctor => UserRole::Medico,
            Role::Nurse => UserRole::Enfermero,
            Role::Viewer => UserRole::Viewer,
        }
    }
}

impl Role {
    pub fn permissions(&self) -> Vec<String> {
        match self {
            Role::Admin => vec![
                "*".to_string(),
                "users:create".to_string(),
                "users:read".to_string(),
                "users:update".to_string(),
                "users:delete".to_string(),
                "patients:create".to_string(),
                "patients:read".to_string(),
                "patients:update".to_string(),
                "patients:delete".to_string(),
                "measurements:create".to_string(),
                "measurements:read".to_string(),
                "measurements:update".to_string(),
                "measurements:delete".to_string(),
                "scales:read".to_string(),
                "scales:write".to_string(),
                "export:csv".to_string(),
                "export:pdf".to_string(),
                "audit:read".to_string(),
                "config:read".to_string(),
                "config:write".to_string(),
            ],
            Role::Doctor => vec![
                "patients:create".to_string(),
                "patients:read".to_string(),
                "patients:update".to_string(),
                "measurements:create".to_string(),
                "measurements:read".to_string(),
                "measurements:update".to_string(),
                "scales:read".to_string(),
                "scales:write".to_string(),
                "export:csv".to_string(),
                "export:pdf".to_string(),
            ],
            Role::Nurse => vec![
                "patients:read".to_string(),
                "measurements:create".to_string(),
                "measurements:read".to_string(),
                "scales:read".to_string(),
                "scales:write".to_string(),
                "export:csv".to_string(),
            ],
            Role::Viewer => vec![
                "patients:read".to_string(),
                "measurements:read".to_string(),
                "scales:read".to_string(),
            ],
        }
    }

    pub fn can(&self, permission: &str) -> bool {
        let perms = self.permissions();
        perms.iter().any(|p| p == "*" || p == permission)
    }

    pub fn label(&self) -> &'static str {
        match self {
            Role::Admin => "Administrador",
            Role::Doctor => "Médico",
            Role::Nurse => "Enfermero",
            Role::Viewer => "Visualizador",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Resource {
    Patient,
    Measurement,
    User,
    Scale,
    Audit,
    Config,
    Export,
}

impl Resource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Resource::Patient => "patients",
            Resource::Measurement => "measurements",
            Resource::User => "users",
            Resource::Scale => "scales",
            Resource::Audit => "audit",
            Resource::Config => "config",
            Resource::Export => "export",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    Create,
    Read,
    Update,
    Delete,
}

impl Action {
    pub fn as_str(&self) -> &'static str {
        match self {
            Action::Create => "create",
            Action::Read => "read",
            Action::Update => "update",
            Action::Delete => "delete",
        }
    }
}

pub fn check_permission(rol: Role, resource: Resource, action: Action) -> bool {
    let permission = format!("{}:{}", resource.as_str(), action.as_str());
    rol.can(&permission)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessDecision {
    pub allowed: bool,
    pub reason: Option<String>,
}

impl AccessDecision {
    pub fn allowed() -> Self {
        Self {
            allowed: true,
            reason: None,
        }
    }

    pub fn denied(reason: impl Into<String>) -> Self {
        Self {
            allowed: false,
            reason: Some(reason.into()),
        }
    }
}

pub fn authorize(rol: Role, resource: Resource, action: Action) -> AccessDecision {
    if check_permission(rol, resource, action) {
        AccessDecision::allowed()
    } else {
        AccessDecision::denied(format!(
            "Rol '{}' no tiene permiso para '{}' en '{}'",
            rol.label(),
            action.as_str(),
            resource.as_str()
        ))
    }
}

/// Maps an HTTP method + path (without the `/api` prefix, as seen by the auth
/// middleware) to the permission required to call it. Returns `None` for routes
/// that only require authentication (e.g. `/auth/me`, `/auth/logout`).
///
/// This is the single authorization table: every non-open route goes through
/// `Claims::has_permission` with the value returned here.
pub fn permission_for(method: &str, path: &str) -> Option<&'static str> {
    let m = method.to_ascii_uppercase();

    if path.starts_with("/diagnosticos") || path.starts_with("/fhir") {
        return Some("patients:read");
    }

    if path == "/stats" {
        return Some("patients:read");
    }

    if path.starts_with("/admin/check-camas")
        || path.starts_with("/admin/camas/disponibles")
        || path.starts_with("/admin/equipos/disponibles")
        || path.starts_with("/admin/equipos/cama/")
    {
        return Some("patients:read");
    }

    if path.starts_with("/patients") {
        match m.as_str() {
            "GET" => Some("patients:read"),
            "POST" | "PUT" | "DELETE" => {
                if path.contains("/measurements") {
                    Some("measurements:create")
                } else if path.contains("/scales") {
                    Some("scales:write")
                } else if path.contains("/egreso") {
                    Some("patients:update")
                } else {
                    Some("patients:create")
                }
            }
            _ => None,
        }
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_for_patient_reads() {
        assert_eq!(permission_for("GET", "/patients"), Some("patients:read"));
        assert_eq!(
            permission_for("GET", "/patients/abc-123"),
            Some("patients:read")
        );
        assert_eq!(
            permission_for("GET", "/patients/abc/measurements/last"),
            Some("patients:read")
        );
        assert_eq!(
            permission_for("GET", "/patients/abc/export/pdf"),
            Some("patients:read")
        );
        assert_eq!(permission_for("GET", "/stats"), Some("patients:read"));
        assert_eq!(
            permission_for("GET", "/diagnosticos/search"),
            Some("patients:read")
        );
    }

    #[test]
    fn permission_for_writes() {
        assert_eq!(permission_for("POST", "/patients"), Some("patients:create"));
        assert_eq!(
            permission_for("PUT", "/patients/abc"),
            Some("patients:create")
        );
        assert_eq!(
            permission_for("DELETE", "/patients/abc"),
            Some("patients:create")
        );
        assert_eq!(
            permission_for("POST", "/patients/abc/egreso"),
            Some("patients:update")
        );
        assert_eq!(
            permission_for("POST", "/patients/abc/measurements"),
            Some("measurements:create")
        );
        assert_eq!(
            permission_for("POST", "/patients/abc/scales/gcs"),
            Some("scales:write")
        );
    }

    #[test]
    fn permission_for_auth_routes_is_none() {
        assert_eq!(permission_for("GET", "/auth/me"), None);
        assert_eq!(permission_for("POST", "/auth/logout"), None);
        assert_eq!(permission_for("POST", "/auth/refresh"), None);
        assert_eq!(permission_for("GET", "/health"), None);
    }

    #[test]
    fn role_permissions_are_coherent() {
        assert!(Role::Admin.can("*"));
        assert!(Role::Doctor.can("patients:create"));
        assert!(Role::Doctor.can("measurements:create"));
        assert!(!Role::Doctor.can("users:delete"));
        assert!(Role::Nurse.can("measurements:create"));
        assert!(Role::Nurse.can("scales:write"));
        assert!(!Role::Nurse.can("patients:create"));
        assert!(Role::Viewer.can("patients:read"));
        assert!(!Role::Viewer.can("patients:create"));
        assert!(!Role::Viewer.can("scales:write"));
    }
}
