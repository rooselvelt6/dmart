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
            Role::Viewer => vec!["patients:read".to_string(), "measurements:read".to_string(), "scales:read".to_string()],
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