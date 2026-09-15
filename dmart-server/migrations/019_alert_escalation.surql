-- SPEC-019: Alert Escalation — política de escalamiento y escalaciones activas.
-- Tabla `escalation_policies`: configuración por severidad (clave única) que
-- gobierna la escalera de cada alerta (tiempos de respuesta y rol objetivo).
-- Tabla `escalations`: incidentes de alerta con máquina de estados
-- created/acknowledged/escalated/resolved y timestamps RFC3339 nullable.

DEFINE TABLE IF NOT EXISTS escalation_policies SCHEMAFULL;

-- Severidad a la que aplica la política (low/medium/high/critical)
DEFINE FIELD IF NOT EXISTS severity ON escalation_policies TYPE string;
-- Tiempo máximo de respuesta (minutos) antes de transicionar de estado
DEFINE FIELD IF NOT EXISTS max_response_minutes ON escalation_policies TYPE int;
-- Timeout (minutos) del nivel antes de escalar al siguiente estado
DEFINE FIELD IF NOT EXISTS timeout_minutes ON escalation_policies TYPE int;
-- Rol objetivo al que se asigna la escalera ("Dr", "Enfermera", ...)
DEFINE FIELD IF NOT EXISTS target_role ON escalation_policies TYPE string;
-- Si la política está activa para nuevas escalaciones
DEFINE FIELD IF NOT EXISTS enabled ON escalation_policies TYPE bool DEFAULT true;
-- Timestamps RFC3339 de creación/última modificación
DEFINE FIELD IF NOT EXISTS created_at ON escalation_policies TYPE string;
DEFINE FIELD IF NOT EXISTS updated_at ON escalation_policies TYPE string;

-- Una única política por severidad (upsert estable)
DEFINE INDEX IF NOT EXISTS idx_escalation_policies_severity ON escalation_policies COLUMNS severity UNIQUE;

-- Defaults sembrados (una por severidad; el record id == severity para que el
-- upsert `set_policy` por severidad encuentre/actualice la fila sembrada)
CREATE escalation_policies:low CONTENT { severity: 'low', max_response_minutes: 60, timeout_minutes: 120, target_role: 'Enfermera', enabled: true, created_at: '2026-01-01T00:00:00Z', updated_at: '2026-01-01T00:00:00Z' };
CREATE escalation_policies:medium CONTENT { severity: 'medium', max_response_minutes: 30, timeout_minutes: 90, target_role: 'Enfermera', enabled: true, created_at: '2026-01-01T00:00:00Z', updated_at: '2026-01-01T00:00:00Z' };
CREATE escalation_policies:high CONTENT { severity: 'high', max_response_minutes: 15, timeout_minutes: 60, target_role: 'Dr', enabled: true, created_at: '2026-01-01T00:00:00Z', updated_at: '2026-01-01T00:00:00Z' };
CREATE escalation_policies:critical CONTENT { severity: 'critical', max_response_minutes: 5, timeout_minutes: 30, target_role: 'Dr', enabled: true, created_at: '2026-01-01T00:00:00Z', updated_at: '2026-01-01T00:00:00Z' };

DEFINE TABLE IF NOT EXISTS escalations SCHEMAFULL;

-- Paciente origen de la alerta (referencia a patient id)
DEFINE FIELD IF NOT EXISTS patient_id ON escalations TYPE string;
-- Tipo de alerta ("ews", "cds", "monitor", ...)
DEFINE FIELD IF NOT EXISTS alert_type ON escalations TYPE string;
-- Severidad de la alerta (low/medium/high/critical)
DEFINE FIELD IF NOT EXISTS severity ON escalations TYPE string;
-- Nivel actual de la escalera (1+, se incrementa en cada escalamiento)
DEFINE FIELD IF NOT EXISTS level ON escalations TYPE int DEFAULT 1;
-- Estado del incidente (created/acknowledged/escalated/resolved)
DEFINE FIELD IF NOT EXISTS status ON escalations TYPE string;
-- Severidad de la política que gobernó el incidente
DEFINE FIELD IF NOT EXISTS policy_severity ON escalations TYPE string;
-- Timestamp RFC3339 de creación
DEFINE FIELD IF NOT EXISTS created_at ON escalations TYPE string;
-- Timestamps RFC3339 nullable por transición
DEFINE FIELD IF NOT EXISTS acknowledged_at ON escalations TYPE option<string>;
DEFINE FIELD IF NOT EXISTS escalated_at ON escalations TYPE option<string>;
DEFINE FIELD IF NOT EXISTS resolved_at ON escalations TYPE option<string>;
-- Actor opcional que acusó / rol al que se escaló
DEFINE FIELD IF NOT EXISTS acknowledged_by ON escalations TYPE option<string>;
DEFINE FIELD IF NOT EXISTS escalated_to ON escalations TYPE option<string>;

DEFINE INDEX IF NOT EXISTS idx_escalations_patient ON escalations COLUMNS patient_id;
DEFINE INDEX IF NOT EXISTS idx_escalations_status_severity ON escalations COLUMNS status, severity;
DEFINE INDEX IF NOT EXISTS idx_escalations_created_at ON escalations COLUMNS created_at;