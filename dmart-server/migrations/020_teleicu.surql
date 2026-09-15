-- SPEC-020: Tele-ICU — sesiones de telemedicina para monitorización remota UCI.
-- Tabla `teleicu_sessions`: una sesión por paciente activo (video/chat), con
-- timestamps RFC3339 y estado `active`/`ended`.

DEFINE TABLE IF NOT EXISTS teleicu_sessions SCHEMAFULL;

-- `session_id` (UUID v4) es el RecordId de la fila y la clave única pública.
DEFINE FIELD IF NOT EXISTS session_id ON TABLE teleicu_sessions TYPE string;
-- Paciente a monitorizar (referencia a patients.patient_id)
DEFINE FIELD IF NOT EXISTS patient_id ON TABLE teleicu_sessions TYPE string;
-- Especialista remoto responsable de la sesión
DEFINE FIELD IF NOT EXISTS specialist_id ON TABLE teleicu_sessions TYPE string;
-- Canal de la sesión (p.ej. "video" / "chat")
DEFINE FIELD IF NOT EXISTS channel ON TABLE teleicu_sessions TYPE string;
-- Estado del ciclo de vida: "active" | "ended"
DEFINE FIELD IF NOT EXISTS status ON TABLE teleicu_sessions TYPE string;
-- Inicio de la sesión (RFC3339)
DEFINE FIELD IF NOT EXISTS started_at ON TABLE teleicu_sessions TYPE string;
-- Fin de la sesión (RFC3339); NULL mientras siga activa
DEFINE FIELD IF NOT EXISTS ended_at ON TABLE teleicu_sessions TYPE option<string> DEFAULT NONE;
-- Notas opcionales de la sesión
DEFINE FIELD IF NOT EXISTS notes ON TABLE teleicu_sessions TYPE option<string> DEFAULT NONE;

-- session_id es único (defensa adicional sobre el RecordId)
DEFINE INDEX IF NOT EXISTS idx_teleicu_sessions_session_id ON TABLE teleicu_sessions COLUMNS session_id UNIQUE;
-- Una sola sesión activa por paciente
DEFINE INDEX IF NOT EXISTS idx_teleicu_sessions_patient_status ON TABLE teleicu_sessions COLUMNS patient_id, status;
-- Ordenación de sesiones recientes por inicio
DEFINE INDEX IF NOT EXISTS idx_teleicu_sessions_started_at ON TABLE teleicu_sessions COLUMNS started_at;