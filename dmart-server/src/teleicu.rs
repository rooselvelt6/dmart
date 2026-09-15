//! SPEC-020: Tele-ICU — monitorización remota y sesiones de telemedicina.
//! Modelos y Store de la tabla `teleicu_sessions`; los handlers HTTP viven en
//! `api::teleicu` y reutilizan lecturas clínicas existentes (get_patient,
//! get_last_measurement, query_timeline).

use anyhow::Result;
use surrealdb::Surreal;
use surrealdb::engine::local::Db;

use chrono::Utc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    Active,
    Ended,
}

impl SessionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            SessionStatus::Active => "active",
            SessionStatus::Ended => "ended",
        }
    }
}

/// Una sesión de tele-ICU. `session_id` (UUID v4) actúa como RecordId de la
/// fila y como clave pública usada en las rutas `/teleicu/sessions/{id}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeleIcuSession {
    #[serde(skip_serializing, skip_deserializing, default)]
    pub id: Option<String>,
    pub session_id: String,
    pub patient_id: String,
    pub specialist_id: String,
    pub channel: String,
    pub status: SessionStatus,
    /// RFC3339
    pub started_at: String,
    /// RFC3339; `None` mientras la sesión siga activa.
    #[serde(default)]
    pub ended_at: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

impl TeleIcuSession {
    pub fn new(patient_id: String, specialist_id: String, channel: String) -> Self {
        Self {
            id: None,
            session_id: uuid::Uuid::new_v4().to_string(),
            patient_id,
            specialist_id,
            channel,
            status: SessionStatus::Active,
            started_at: Utc::now().to_rfc3339(),
            ended_at: None,
            notes: None,
        }
    }
}

// ─── Store (tabla teleicu_sessions) ────────────────────────────────────

pub async fn create_session(db: &Surreal<Db>, session: &TeleIcuSession) -> Result<TeleIcuSession> {
    let session_id = session.session_id.clone();
    let created: Option<TeleIcuSession> = db
        .create(("teleicu_sessions", session_id))
        .content(session.clone())
        .await?;
    created.ok_or_else(|| anyhow::anyhow!("Failed to create teleicu session"))
}

pub async fn get_session(db: &Surreal<Db>, session_id: &str) -> Result<Option<TeleIcuSession>> {
    let session: Option<TeleIcuSession> = db.select(("teleicu_sessions", session_id)).await?;
    Ok(session)
}

/// Busca la sesión activa del paciente (máximo una por especificación).
pub async fn find_active_session_for_patient(
    db: &Surreal<Db>,
    patient_id: &str,
) -> Result<Option<TeleIcuSession>> {
    let pid = patient_id.to_string();
    let sessions: Vec<TeleIcuSession> = db
        .query(
            "SELECT * FROM teleicu_sessions WHERE patient_id = $pid AND status = $status \
             ORDER BY started_at DESC LIMIT 1",
        )
        .bind(("pid", pid))
        .bind(("status", SessionStatus::Active.as_str()))
        .await?
        .take(0)?;
    Ok(sessions.into_iter().next())
}

/// Todas las sesiones, con las activas primero y luego las recientes por
/// `started_at` descendente (orden estable).
pub async fn list_sessions(db: &Surreal<Db>) -> Result<Vec<TeleIcuSession>> {
    let mut sessions: Vec<TeleIcuSession> =
        db.query("SELECT * FROM teleicu_sessions").await?.take(0)?;
    sessions.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    sessions.sort_by_key(|s| s.status != SessionStatus::Active);
    Ok(sessions)
}

/// Marca la sesión como `ended` y registra `ended_at = now`. Devuelve `None`
/// si la sesión no existe.
pub async fn close_session(db: &Surreal<Db>, session_id: &str) -> Result<Option<TeleIcuSession>> {
    let Some(mut session) = get_session(db, session_id).await? else {
        return Ok(None);
    };
    session.status = SessionStatus::Ended;
    session.ended_at = Some(Utc::now().to_rfc3339());
    let updated: Option<TeleIcuSession> = db
        .update(("teleicu_sessions", session_id.to_string()))
        .content(session)
        .await?;
    Ok(updated)
}

/// Duración de una sesión en minutos (0 si aún no ha transcurrido un minuto).
pub fn duration_minutes(started_at: &str, ended_at: &str) -> Option<i64> {
    use chrono::DateTime;
    let start = DateTime::parse_from_rfc3339(started_at).ok()?;
    let end = DateTime::parse_from_rfc3339(ended_at).ok()?;
    Some((end - start).num_minutes())
}
