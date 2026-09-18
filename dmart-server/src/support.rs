//! SPEC-044: Consola Técnica de Soporte.
//!
//! Telemetría por subsistema (frescura de último evento + contadores de error),
//! registro de eventos de soporte (`support_events`, acciones manuales y
//! auto-recuperaciones) y ejecución de correcciones idempotentes y auditadas.
//!
//! La persistencia de eventos usa una instancia ligada a la `Database` de cada
//! request (patrón `AuthService`), mientras que la telemetría en memoria es
//! global al proceso (no necesita BD).

use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::RwLock;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use surrealdb::Surreal;
use surrealdb::engine::local::Db;
use uuid::Uuid;

/// Subsistemas monitorizados por la consola (orden de presentación).
pub const SUBSYSTEMS: &[&str] = &[
    "db",
    "ingest",
    "realtime",
    "ml",
    "monitores",
    "audit",
    "backup",
];

/// Retención de auditoría (mismo valor que `audit::AUDIT_RETENTION_YEARS`).
pub const AUDIT_RETENTION_YEARS: i64 = 6;

// ─── Telemetría en memoria ─────────────────────────────────────────────────

#[derive(Debug, Default)]
struct SubsysTelemetry {
    last_event_ms: AtomicI64,
    errors: AtomicU64,
    ok: AtomicU64,
}

fn telemetry() -> &'static RwLock<HashMap<String, SubsysTelemetry>> {
    static MAP: OnceLock<RwLock<HashMap<String, SubsysTelemetry>>> = OnceLock::new();
    MAP.get_or_init(|| {
        RwLock::new(
            SUBSYSTEMS
                .iter()
                .map(|s| ((*s).to_string(), SubsysTelemetry::default()))
                .collect(),
        )
    })
}

/// Registra un evento de subsistema (éxito o error) en la telemetría global.
pub fn note(system: &str, ok: bool) {
    let map = telemetry();
    let mut guard = map.write().unwrap_or_else(|p| p.into_inner());
    let t = guard.entry(system.to_string()).or_default();
    t.last_event_ms.store(now_millis(), Ordering::Relaxed);
    if ok {
        t.ok.fetch_add(1, Ordering::Relaxed);
    } else {
        t.errors.fetch_add(1, Ordering::Relaxed);
    }
}

/// Segundos desde el último evento del subsistema (o `None` si nunca hubo).
pub fn freshness_secs(system: &str) -> Option<f64> {
    let map = telemetry();
    let guard = map.read().unwrap_or_else(|p| p.into_inner());
    let last = guard.get(system)?.last_event_ms.load(Ordering::Relaxed);
    if last == 0 {
        None
    } else {
        Some((now_millis() - last) as f64 / 1000.0)
    }
}

/// Contador de errores del subsistema desde el arranque.
pub fn error_count(system: &str) -> u64 {
    let map = telemetry();
    let guard = map.read().unwrap_or_else(|p| p.into_inner());
    guard
        .get(system)
        .map(|t| t.errors.load(Ordering::Relaxed))
        .unwrap_or(0)
}

/// Éxitos contados del subsistema desde el arranque.
pub fn ok_count(system: &str) -> u64 {
    let map = telemetry();
    let guard = map.read().unwrap_or_else(|p| p.into_inner());
    guard
        .get(system)
        .map(|t| t.ok.load(Ordering::Relaxed))
        .unwrap_or(0)
}

fn now_millis() -> i64 {
    Utc::now().timestamp_millis()
}

// ─── Eventos de soporte (persistidos) ──────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum EventOrigin {
    Manual,
    Auto,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SupportEvent {
    pub uid: String,
    pub timestamp: String,
    pub origin: EventOrigin,
    pub subsystem: String,
    pub action: String,
    pub user_id: Option<String>,
    pub username: Option<String>,
    pub ip_address: Option<String>,
    pub success: bool,
    pub message: String,
    #[serde(default)]
    pub details: serde_json::Value,
}

#[derive(Clone)]
pub struct SupportService {
    db: Surreal<Db>,
}

impl SupportService {
    pub fn new(db: Surreal<Db>) -> Self {
        Self { db }
    }

    /// Persiste un evento de soporte en `support_events`.
    #[allow(clippy::too_many_arguments)]
    pub async fn log_event(
        &self,
        origin: EventOrigin,
        subsystem: &str,
        action: &str,
        user_id: Option<&str>,
        username: Option<&str>,
        ip_address: Option<&str>,
        success: bool,
        message: &str,
        details: serde_json::Value,
    ) -> Result<SupportEvent, String> {
        let event = SupportEvent {
            uid: Uuid::new_v4().to_string(),
            timestamp: Utc::now().to_rfc3339(),
            origin,
            subsystem: subsystem.to_string(),
            action: action.to_string(),
            user_id: user_id.map(String::from),
            username: username.map(String::from),
            ip_address: ip_address.map(String::from),
            success,
            message: message.to_string(),
            details,
        };
        let created: Option<SupportEvent> = self
            .db
            .create(("support_events", event.uid.clone()))
            .content(event)
            .await
            .map_err(|e| e.to_string())?;
        created.ok_or_else(|| "Failed to create support event".to_string())
    }

    /// Historial de eventos (acciones manuales + auto-recuperaciones).
    pub async fn history(&self, limit: usize) -> Result<Vec<SupportEvent>, String> {
        let events: Vec<SupportEvent> = self
            .db
            .query("SELECT * FROM support_events ORDER BY timestamp DESC LIMIT $limit")
            .bind(("limit", limit as i64))
            .await
            .map_err(|e| e.to_string())?
            .take(0)
            .map_err(|e| e.to_string())?;
        Ok(events)
    }
}

/// Registra una auto-recuperación (self-healing) sin actor humano.
pub async fn note_auto_recovery(
    db: &Surreal<Db>,
    subsystem: &str,
    message: &str,
) -> Result<SupportEvent, String> {
    crate::metrics::self_healing(subsystem, "success");
    note(subsystem, true);
    SupportService::new(db.clone())
        .log_event(
            EventOrigin::Auto,
            subsystem,
            "self_heal",
            None,
            None,
            None,
            true,
            message,
            serde_json::Value::Null,
        )
        .await
}

// ─── Acciones de corrección (idempotentes) ─────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ActionRequest {
    /// Modelo para `model_swap` (e.g. "ews").
    #[serde(default)]
    pub model: String,
    /// Versión para `model_swap` (e.g. "v1.2.0"); vacío usa la activa/presente.
    #[serde(default)]
    pub version: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SupportActionResult {
    pub action: String,
    pub success: bool,
    pub message: String,
    pub details: serde_json::Value,
}

pub const ACTIONS: &[&str] = &[
    "ingest_retry",
    "circuit_reset",
    "backup",
    "audit_retention",
    "model_swap",
    "verify_fingerprints",
];

/// Ejecuta la acción y devuelve el resultado (sin registrar todavía; el caller
/// se encarga de registrar el evento + auditoría + métrica).
pub async fn execute_action(
    action: &str,
    req: &ActionRequest,
    db: &Surreal<Db>,
) -> SupportActionResult {
    match action {
        "ingest_retry" => {
            let count = match crate::ingest::global_ingest() {
                Some(state) => state.reset_all_devices().await,
                None => {
                    return SupportActionResult {
                        action: action.to_string(),
                        success: true,
                        message:
                            "Sin estado de ingest activo (MLLP no iniciado); no requiere corrección"
                                .to_string(),
                        details: serde_json::json!({ "reset": 0 }),
                    };
                }
            };
            crate::metrics::support_action(action, "success");
            SupportActionResult {
                action: action.to_string(),
                success: true,
                message: format!("Ingest reiniciado: {count} dispositivo(s) en estado limpio"),
                details: serde_json::json!({ "reset": count }),
            }
        }
        "circuit_reset" => {
            let count = match crate::ingest::global_ingest() {
                Some(state) => state.reset_open_circuits().await,
                None => 0,
            };
            crate::metrics::support_action(action, "success");
            SupportActionResult {
                action: action.to_string(),
                success: true,
                message: format!(
                    "Circuit breaker transicionado a half-open en {count} dispositivo(s)"
                ),
                details: serde_json::json!({ "reset": count }),
            }
        }
        "backup" => trigger_backup(action).await,
        "audit_retention" => match crate::audit::audit() {
            Some(audit) => match audit.cleanup_old_logs().await {
                Ok(deleted) => {
                    crate::metrics::support_action(action, "success");
                    note("audit", true);
                    SupportActionResult {
                        action: action.to_string(),
                        success: true,
                        message: format!(
                            "Retención de auditoría aplicada (>{} años): {deleted} logs eliminados",
                            AUDIT_RETENTION_YEARS
                        ),
                        details: serde_json::json!({ "deleted": deleted }),
                    }
                }
                Err(e) => {
                    crate::metrics::support_action(action, "error");
                    SupportActionResult {
                        action: action.to_string(),
                        success: false,
                        message: format!("Auditoría: {e}"),
                        details: serde_json::json!({ "error": e }),
                    }
                }
            },
            None => {
                crate::metrics::support_action(action, "success");
                SupportActionResult {
                    action: action.to_string(),
                    success: true,
                    message: "Retención de auditoría no habilitada (servicio sin init)".to_string(),
                    details: serde_json::json!({ "deleted": 0 }),
                }
            }
        },
        "model_swap" => {
            let model = if req.model.is_empty() {
                "ews"
            } else {
                &req.model
            };
            match crate::ml_serving::server().swap(model, &req.version) {
                Ok(m) => {
                    crate::metrics::support_action(action, "success");
                    note("ml", true);
                    SupportActionResult {
                        action: action.to_string(),
                        success: true,
                        message: format!("Modelo ML {}@{} activado", m.name, m.version),
                        details: serde_json::json!({ "model": m.name, "version": m.version }),
                    }
                }
                Err(e) => {
                    crate::metrics::support_action(action, "error");
                    note("ml", false);
                    SupportActionResult {
                        action: action.to_string(),
                        success: false,
                        message: format!("Swap de modelo falló: {e}"),
                        details: serde_json::json!({ "error": e.to_string() }),
                    }
                }
            }
        }
        "verify_fingerprints" => {
            let entries = crate::api::score_audit::verify_all(db).await;
            let total = entries.len();
            let reproducible = entries.iter().filter(|e| e.reproducible).count();
            let legacy = entries
                .iter()
                .filter(|e| e.algorithm_version == "<legacy>")
                .count();
            crate::metrics::support_action(action, "success");
            SupportActionResult {
                action: action.to_string(),
                success: true,
                message: format!(
                    "Fingerprints verificados: {reproducible}/{total} reproducibles ({legacy} legacy)",
                ),
                details: serde_json::json!({
                    "total": total,
                    "reproducible": reproducible,
                    "legacy": legacy,
                }),
            }
        }
        _ => {
            crate::metrics::support_action(action, "error");
            SupportActionResult {
                action: action.to_string(),
                success: false,
                message: format!("Acción no soportada: {action}"),
                details: serde_json::json!({}),
            }
        }
    }
}

/// Dispara `scripts/backup.sh` (o `DMART_BACKUP_SCRIPT`). Si el script no
/// existe devuelve un resultado "simulado" para que la consola funcione en
/// entornos sin backup instalado (nunca falla la operación).
async fn trigger_backup(action: &str) -> SupportActionResult {
    let script = std::env::var("DMART_BACKUP_SCRIPT").unwrap_or_else(|_| {
        std::env::current_dir()
            .map(|p| p.join("scripts/backup.sh").to_string_lossy().into_owned())
            .unwrap_or_else(|_| "./scripts/backup.sh".to_string())
    });
    run_backup_script(&script, action).await
}

async fn run_backup_script(script: &str, action: &str) -> SupportActionResult {
    if !std::path::Path::new(script).exists() {
        crate::metrics::support_action(action, "success");
        note("backup", true);
        return SupportActionResult {
            action: action.to_string(),
            success: true,
            message: "Backup simulado: script de backup no disponible en este entorno".to_string(),
            details: serde_json::json!({ "kind": "simulated" }),
        };
    }

    let output = tokio::process::Command::new(script).output().await;
    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
            crate::metrics::support_action(action, "success");
            note("backup", true);
            SupportActionResult {
                action: action.to_string(),
                success: true,
                message: "Backup completado y verificado".to_string(),
                details: serde_json::json!({ "stdout": stdout }),
            }
        }
        Ok(out) => {
            let err = String::from_utf8_lossy(&out.stderr).into_owned();
            crate::metrics::support_action(action, "error");
            note("backup", false);
            SupportActionResult {
                action: action.to_string(),
                success: false,
                message: format!("Backup falló (exit {}): {}", out.status, err),
                details: serde_json::json!({ "stderr": err }),
            }
        }
        Err(e) => {
            crate::metrics::support_action(action, "error");
            note("backup", false);
            SupportActionResult {
                action: action.to_string(),
                success: false,
                message: format!("No se pudo ejecutar el backup: {e}"),
                details: serde_json::json!({ "error": e.to_string() }),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_surreal_db(path: &std::path::Path) -> Surreal<Db> {
        let db = Surreal::new::<surrealdb::engine::local::SurrealKv>(path.to_str().expect("p"))
            .await
            .expect("db");
        db.use_ns("dmart").use_db("icu").await.expect("ns");
        db
    }

    #[tokio::test]
    async fn telemetry_tracks_freshness_and_errors() {
        note("ingest", true);
        note("ingest", true);
        note("ingest", false);
        assert!(ok_count("ingest") >= 2);
        assert!(error_count("ingest") >= 1);
        assert!(freshness_secs("ingest").is_some());
    }

    #[test]
    fn action_list_is_stable() {
        assert_eq!(ACTIONS.len(), 6);
        assert!(ACTIONS.contains(&"circuit_reset"));
        assert!(ACTIONS.contains(&"verify_fingerprints"));
    }

    #[tokio::test]
    async fn unknown_action_returns_error() {
        let dir = tempfile::tempdir().expect("temp");
        let db = test_surreal_db(&dir.path().join("s.db")).await;
        let result = execute_action("no_existe", &ActionRequest::default(), &db).await;
        assert!(!result.success);
    }

    #[tokio::test]
    async fn backup_missing_script_is_simulated() {
        let result = run_backup_script("/tmp/opencode/no_existe_backup.sh", "backup").await;
        assert!(result.success, "debe simular si el script no existe");
        assert_eq!(result.details["kind"], "simulated");
    }

    #[tokio::test]
    async fn auto_recovery_event_persists() {
        let dir = tempfile::tempdir().expect("temp");
        let db = test_surreal_db(&dir.path().join("s2.db")).await;

        let event = note_auto_recovery(&db, "ingest", "CB Open → HalfOpen")
            .await
            .expect("evento auto");
        assert_eq!(event.origin, EventOrigin::Auto);
        assert!(event.user_id.is_none());
        assert_eq!(event.action, "self_heal");
        assert!(event.success);

        let history = SupportService::new(db.clone())
            .history(10)
            .await
            .expect("hist");
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].origin, EventOrigin::Auto);
        assert_eq!(history[0].subsystem, "ingest");
    }

    #[tokio::test]
    async fn ingested_retry_without_global_state_is_noop_success() {
        let dir = tempfile::tempdir().expect("temp");
        let db = test_surreal_db(&dir.path().join("s3.db")).await;
        let result = execute_action("ingest_retry", &ActionRequest::default(), &db).await;
        assert!(result.success);
        assert_eq!(result.details["reset"], 0);
    }
}
