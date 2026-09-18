use dmart_shared::models::UserRole;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    Admin,
    Doctor,
    Nurse,
    Viewer,
    /// Soporte técnico (SPEC-044): consola técnica, sin administración clínica.
    Support,
}

impl From<UserRole> for Role {
    fn from(rol: UserRole) -> Self {
        match rol {
            UserRole::Admin => Role::Admin,
            UserRole::Medico => Role::Doctor,
            UserRole::Enfermero => Role::Nurse,
            UserRole::Viewer => Role::Viewer,
            UserRole::Soporte => Role::Support,
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
            Role::Support => UserRole::Soporte,
        }
    }
}

impl Role {
    pub fn permissions(&self) -> Vec<String> {
        UserRole::from(*self)
            .permissions()
            .into_iter()
            .map(String::from)
            .collect()
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
            Role::Support => "Soporte Técnico",
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

    // SPEC-044: Consola técnica de soporte. Sin distinguir el action, la
    // lectura exige `support:read` y la ejecución `support:act`.
    if path.starts_with("/admin/support") {
        return match m.as_str() {
            "GET" => Some("support:read"),
            "POST" | "PUT" | "DELETE" => Some("support:act"),
            _ => None,
        };
    }

    if path == "/stats" {
        return Some("patients:read");
    }

    // Rutas de cuidado clínico: accesibles para cualquier rol clínico autenticado.
    if path.starts_with("/admin/check-camas")
        || path.starts_with("/admin/camas/disponibles")
        || path.starts_with("/admin/equipos/disponibles")
        || path.starts_with("/admin/equipos/cama/")
    {
        return Some("patients:read");
    }

    if path.starts_with("/patients") {
        let rest = path.strip_prefix("/patients").unwrap_or(path);

        // Exportación: permisos específicos por formato.
        if rest.ends_with("/export/csv") && m == "GET" {
            return Some("export:csv");
        }
        if rest.ends_with("/export/pdf") && m == "GET" {
            return Some("export:pdf");
        }

        match m.as_str() {
            "GET" => Some("patients:read"),
            "POST" | "PUT" => {
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
            // Un DELETE de paciente requiere permiso de borrado (`patients:delete`),
            // no de alta (`patients:create`).
            "DELETE" => Some("patients:delete"),
            _ => None,
        }
    } else if path.starts_with("/admin") {
        if path.starts_with("/admin/staff") {
            return match m.as_str() {
                "GET" => Some("users:read"),
                "PUT" => Some("users:update"),
                "DELETE" => Some("users:delete"),
                // POST: creación de personal, salvo el toggle de estado.
                "POST" if path.ends_with("/toggle") => Some("users:update"),
                "POST" => Some("users:create"),
                _ => None,
            };
        }
        if path.starts_with("/admin/audit") {
            return match m.as_str() {
                "GET" => Some("audit:read"),
                "POST" | "PUT" | "DELETE" => Some("audit:act"),
                _ => None,
            };
        }
        if path.starts_with("/admin/camas") {
            return match m.as_str() {
                "GET" => Some("config:read"),
                _ => Some("config:write"),
            };
        }
        if path.starts_with("/admin/equipos") {
            return match m.as_str() {
                "GET" => Some("config:read"),
                _ => Some("config:write"),
            };
        }
        if path == "/admin/institucion" {
            return match m.as_str() {
                "GET" => Some("config:read"),
                _ => Some("config:write"),
            };
        }
        // Multi-tenancy (SPEC-025): solo administradores gestionan tenants.
        if path.starts_with("/admin/tenants") {
            return match m.as_str() {
                "GET" => Some("tenants:read"),
                "POST" => Some("tenants:manage"),
                _ => None,
            };
        }
        // /admin/stats — panel de operaciones (solo administradores).
        Some("config:read")
    } else if path.starts_with("/ml") {
        match m.as_str() {
            // Predicciones y búsqueda de similaridad: roles clínicos.
            "POST" => Some("ml:predict"),
            // GET /ml/models, /ml/similarity/status, explain → lectura.
            "GET" => Some("ml:read"),
            _ => None,
        }
    } else if path.starts_with("/auth/users") {
        match m.as_str() {
            "GET" => Some("users:read"),
            "POST" => Some("users:create"),
            "PUT" => Some("users:update"),
            "DELETE" => Some("users:delete"),
            _ => None,
        }
    } else if path.starts_with("/auth/register") {
        Some("users:create")
    } else if path.starts_with("/sandbox") {
        Some("config:write")
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
            Some("patients:delete")
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
    fn permission_for_admin_areas() {
        assert_eq!(permission_for("GET", "/admin/staff"), Some("users:read"));
        assert_eq!(permission_for("POST", "/admin/staff"), Some("users:create"));
        assert_eq!(
            permission_for("PUT", "/admin/staff/abc"),
            Some("users:update")
        );
        assert_eq!(
            permission_for("DELETE", "/admin/staff/abc"),
            Some("users:delete")
        );
        assert_eq!(
            permission_for("POST", "/admin/staff/abc/toggle"),
            Some("users:update")
        );
        assert_eq!(permission_for("GET", "/admin/audit"), Some("audit:read"));
        assert_eq!(
            permission_for("GET", "/admin/audit/critical"),
            Some("audit:read")
        );
        assert_eq!(
            permission_for("GET", "/admin/institucion"),
            Some("config:read")
        );
        assert_eq!(
            permission_for("PUT", "/admin/institucion"),
            Some("config:write")
        );
        assert_eq!(
            permission_for("POST", "/admin/camas/init"),
            Some("config:write")
        );
        assert_eq!(
            permission_for("POST", "/admin/equipos/asignar"),
            Some("config:write")
        );
        assert_eq!(permission_for("GET", "/admin/equipos"), Some("config:read"));
        assert_eq!(permission_for("GET", "/admin/stats"), Some("config:read"));
    }

    #[test]
    fn permission_for_care_paths_remain_clinical() {
        assert_eq!(
            permission_for("GET", "/admin/check-camas"),
            Some("patients:read")
        );
        assert_eq!(
            permission_for("GET", "/admin/camas/disponibles"),
            Some("patients:read")
        );
        assert_eq!(
            permission_for("GET", "/admin/equipos/disponibles"),
            Some("patients:read")
        );
        assert_eq!(
            permission_for("GET", "/admin/equipos/cama/abc"),
            Some("patients:read")
        );
    }

    #[test]
    fn permission_for_auth_users_and_sandbox() {
        assert_eq!(permission_for("GET", "/auth/users"), Some("users:read"));
        assert_eq!(
            permission_for("POST", "/auth/register"),
            Some("users:create")
        );
        assert_eq!(
            permission_for("POST", "/sandbox/generate"),
            Some("config:write")
        );
        assert_eq!(
            permission_for("POST", "/sandbox/clear"),
            Some("config:write")
        );
    }

    #[test]
    fn permission_for_exports_is_format_specific() {
        assert_eq!(
            permission_for("GET", "/patients/abc/export/csv"),
            Some("export:csv")
        );
        assert_eq!(
            permission_for("GET", "/patients/abc/export/pdf"),
            Some("export:pdf")
        );
    }

    #[test]
    fn permission_for_support_console() {
        assert_eq!(
            permission_for("GET", "/admin/support/systems"),
            Some("support:read")
        );
        assert_eq!(
            permission_for("GET", "/admin/support/diagnostics"),
            Some("support:read")
        );
        assert_eq!(
            permission_for("GET", "/admin/support/history"),
            Some("support:read")
        );
        assert_eq!(
            permission_for("POST", "/admin/support/actions/circuit_reset"),
            Some("support:act")
        );
        assert_eq!(
            permission_for("POST", "/admin/support/actions/backup"),
            Some("support:act")
        );
        assert_eq!(
            permission_for("DELETE", "/admin/support/whatever"),
            Some("support:act")
        );
        assert_eq!(
            permission_for("PUT", "/admin/support/x"),
            Some("support:act")
        );
        assert!(
            Role::Support.can("support:read"),
            "Soporte debe poder leer diagnósticos"
        );
        assert!(
            Role::Support.can("support:act"),
            "Soporte debe poder ejecutar acciones"
        );
        assert!(
            Role::Support.can("patients:read"),
            "Soporte mantiene contexto clínico de solo lectura"
        );
        assert!(!Role::Support.can("users:delete"));
        assert!(!Role::Support.can("config:write"));
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
        // Permisos administrativos restringidos a Admin.
        assert!(Role::Admin.can("users:create"));
        assert!(Role::Admin.can("users:update"));
        assert!(Role::Admin.can("users:delete"));
        assert!(Role::Admin.can("users:read"));
        assert!(Role::Admin.can("audit:read"));
        assert!(Role::Admin.can("config:read"));
        assert!(Role::Admin.can("config:write"));
        assert!(!Role::Doctor.can("users:read"));
        assert!(!Role::Doctor.can("audit:read"));
        assert!(!Role::Doctor.can("config:write"));
        assert!(!Role::Nurse.can("users:read"));
    }
}
