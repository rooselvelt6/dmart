-- Baseline: indices del esquema Fase 1 (estado actual de la app).
-- Define indices idempotentes sobre las tablas existentes; las migraciones
-- de Fase 3+ añadirán más archivos 00N_*.surql que corren en orden.

DEFINE INDEX IF NOT EXISTS idx_patients_created_at ON TABLE patients COLUMNS created_at;
DEFINE INDEX IF NOT EXISTS idx_patients_estado_gravedad ON TABLE patients COLUMNS estado_gravedad;
DEFINE INDEX IF NOT EXISTS idx_measurements_patient_id ON TABLE measurements COLUMNS patient_id;
DEFINE INDEX IF NOT EXISTS idx_measurements_timestamp ON TABLE measurements COLUMNS timestamp;
DEFINE INDEX IF NOT EXISTS idx_users_username ON TABLE users COLUMNS username;
DEFINE INDEX IF NOT EXISTS idx_camas_estado ON TABLE camas COLUMNS estado;
DEFINE INDEX IF NOT EXISTS idx_camas_tipo ON TABLE camas COLUMNS tipo;
DEFINE INDEX IF NOT EXISTS idx_equipos_cama_id ON TABLE equipos COLUMNS cama_id;
DEFINE INDEX IF NOT EXISTS idx_equipos_estado ON TABLE equipos COLUMNS estado;
DEFINE INDEX IF NOT EXISTS idx_audit_logs_timestamp ON TABLE audit_logs COLUMNS timestamp;
DEFINE INDEX IF NOT EXISTS idx_diagnosticos_codigo ON TABLE diagnosticos COLUMNS codigo;