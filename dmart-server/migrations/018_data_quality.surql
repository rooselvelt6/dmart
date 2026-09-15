-- SPEC-018: Data Quality — issues de calidad del dato vital.
-- Tabla append-only `quality_events`: cada issue detectado por los
-- validadores de calidad (rango fisiológico, valor no finito, timestamp
-- futuro/viejo, patient_ref ausente, secuencia fuera de orden).

DEFINE TABLE IF NOT EXISTS quality_events SCHEMAFULL;

-- Referencia al mensaje HL7 que originó el issue.
DEFINE FIELD IF NOT EXISTS message_id ON quality_events TYPE string;
-- Identificador del paciente (vacío si nunca se pudo resolver).
DEFINE FIELD IF NOT EXISTS patient_ref ON quality_events TYPE string;
-- Severidad del issue: low | medium | high.
DEFINE FIELD IF NOT EXISTS severity ON quality_events TYPE string;
-- Código estable del issue (p.ej. VITAL_OUT_OF_RANGE, TIMESTAMP_STALE).
DEFINE FIELD IF NOT EXISTS code ON quality_events TYPE string;
-- Descripción legible del problema detectado.
DEFINE FIELD IF NOT EXISTS detail ON quality_events TYPE string;
-- Timestamp RFC 3339 UTC de detección.
DEFINE FIELD IF NOT EXISTS timestamp ON quality_events TYPE string;

-- Consultas de reporte y filtros por severidad, código y mensaje.
DEFINE INDEX IF NOT EXISTS idx_quality_events_timestamp ON quality_events COLUMNS timestamp;
DEFINE INDEX IF NOT EXISTS idx_quality_events_severity ON quality_events COLUMNS severity;
DEFINE INDEX IF NOT EXISTS idx_quality_events_code ON quality_events COLUMNS code;
DEFINE INDEX IF NOT EXISTS idx_quality_events_message_id ON quality_events COLUMNS message_id;