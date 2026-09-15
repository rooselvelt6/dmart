-- SPEC-015: Patient Timeline API — Event sourcing append-only.
-- Tabla `patient_event` almacena cada evento clínico del paciente
-- ordenado temporalmente, inmutable, con fingerprint SPEC-029.

DEFINE TABLE IF NOT EXISTS patient_event SCHEMAFULL;

-- Identificador del paciente (referencia a patient.id)
DEFINE FIELD IF NOT EXISTS patient_id ON patient_event TYPE string;
-- Tipo de evento clínico
DEFINE FIELD IF NOT EXISTS event_type ON patient_event TYPE string;
-- Timestamp del evento (epoch millis UTC)
DEFINE FIELD IF NOT EXISTS occurred_at ON patient_event TYPE int;
-- Payload completo del evento (JSON)
DEFINE FIELD IF NOT EXISTS payload ON patient_event TYPE object;
-- Fingerprint SHA-256 del payload canónico (SPEC-029)
DEFINE FIELD IF NOT EXISTS fingerprint ON patient_event TYPE string;
-- Severidad del evento (info/warning/critical)
DEFINE FIELD IF NOT EXISTS severity ON patient_event TYPE string;
-- Origen del evento (monitor, manual, system, cds)
DEFINE FIELD IF NOT EXISTS source ON patient_event TYPE string;
-- Metadatos adicionales (tags, correlación)
DEFINE FIELD IF NOT EXISTS meta ON patient_event TYPE object DEFAULT {};

-- Índices para consultas temporales y por paciente
DEFINE INDEX IF NOT EXISTS idx_patient_event_patient_time ON patient_event COLUMNS patient_id, occurred_at;
DEFINE INDEX IF NOT EXISTS idx_patient_event_type ON patient_event COLUMNS event_type;
DEFINE INDEX IF NOT EXISTS idx_patient_event_fingerprint ON patient_event COLUMNS fingerprint UNIQUE;
DEFINE INDEX IF NOT EXISTS idx_patient_event_severity ON patient_event COLUMNS severity;

-- Eventos de ejemplo de tipos permitidos (validación a nivel app):
-- Admission, VitalSigns, ScoreCalculated, Intervention, Medication,
-- Procedure, Note, Discharge, Alert, CdsAction