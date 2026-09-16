-- SPEC-030: Retención y downsampling de mediciones.
-- Tablas de agregados por hora y por día (granularidad decreciente) + trail de
-- ejecuciones del job de retención. Idempotente (IF NOT EXISTS).
-- Los agregados NUNCA conservan datos PHI: solo scores agregados + patient_id +
-- timestamp del bucket + count (nº de lecturas raw).

DEFINE TABLE IF NOT EXISTS measurements_hourly SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS patient_id ON TABLE measurements_hourly TYPE string;
DEFINE FIELD IF NOT EXISTS timestamp ON TABLE measurements_hourly TYPE string;
DEFINE FIELD IF NOT EXISTS count ON TABLE measurements_hourly TYPE int;
DEFINE FIELD IF NOT EXISTS apache_avg ON TABLE measurements_hourly TYPE float;
DEFINE FIELD IF NOT EXISTS apache_min ON TABLE measurements_hourly TYPE float;
DEFINE FIELD IF NOT EXISTS apache_max ON TABLE measurements_hourly TYPE float;
DEFINE FIELD IF NOT EXISTS gcs_avg ON TABLE measurements_hourly TYPE float;
DEFINE FIELD IF NOT EXISTS gcs_min ON TABLE measurements_hourly TYPE float;
DEFINE FIELD IF NOT EXISTS gcs_max ON TABLE measurements_hourly TYPE float;
DEFINE FIELD IF NOT EXISTS saps3_avg ON TABLE measurements_hourly TYPE float;
DEFINE FIELD IF NOT EXISTS saps3_min ON TABLE measurements_hourly TYPE float;
DEFINE FIELD IF NOT EXISTS saps3_max ON TABLE measurements_hourly TYPE float;
DEFINE FIELD IF NOT EXISTS news2_avg ON TABLE measurements_hourly TYPE float;
DEFINE FIELD IF NOT EXISTS news2_min ON TABLE measurements_hourly TYPE float;
DEFINE FIELD IF NOT EXISTS news2_max ON TABLE measurements_hourly TYPE float;
DEFINE FIELD IF NOT EXISTS sofa_avg ON TABLE measurements_hourly TYPE float;
DEFINE FIELD IF NOT EXISTS sofa_min ON TABLE measurements_hourly TYPE float;
DEFINE FIELD IF NOT EXISTS sofa_max ON TABLE measurements_hourly TYPE float;
-- Un bucket = (patient_id, timestamp-hora); UNIQUE ⇒ el upsert nunca duplica.
DEFINE INDEX IF NOT EXISTS idx_measurements_hourly_patient_bucket ON TABLE measurements_hourly COLUMNS patient_id, timestamp UNIQUE;

DEFINE TABLE IF NOT EXISTS measurements_daily SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS patient_id ON TABLE measurements_daily TYPE string;
DEFINE FIELD IF NOT EXISTS timestamp ON TABLE measurements_daily TYPE string;
DEFINE FIELD IF NOT EXISTS count ON TABLE measurements_daily TYPE int;
DEFINE FIELD IF NOT EXISTS apache_avg ON TABLE measurements_daily TYPE float;
DEFINE FIELD IF NOT EXISTS apache_min ON TABLE measurements_daily TYPE float;
DEFINE FIELD IF NOT EXISTS apache_max ON TABLE measurements_daily TYPE float;
DEFINE FIELD IF NOT EXISTS gcs_avg ON TABLE measurements_daily TYPE float;
DEFINE FIELD IF NOT EXISTS gcs_min ON TABLE measurements_daily TYPE float;
DEFINE FIELD IF NOT EXISTS gcs_max ON TABLE measurements_daily TYPE float;
DEFINE FIELD IF NOT EXISTS saps3_avg ON TABLE measurements_daily TYPE float;
DEFINE FIELD IF NOT EXISTS saps3_min ON TABLE measurements_daily TYPE float;
DEFINE FIELD IF NOT EXISTS saps3_max ON TABLE measurements_daily TYPE float;
DEFINE FIELD IF NOT EXISTS news2_avg ON TABLE measurements_daily TYPE float;
DEFINE FIELD IF NOT EXISTS news2_min ON TABLE measurements_daily TYPE float;
DEFINE FIELD IF NOT EXISTS news2_max ON TABLE measurements_daily TYPE float;
DEFINE FIELD IF NOT EXISTS sofa_avg ON TABLE measurements_daily TYPE float;
DEFINE FIELD IF NOT EXISTS sofa_min ON TABLE measurements_daily TYPE float;
DEFINE FIELD IF NOT EXISTS sofa_max ON TABLE measurements_daily TYPE float;
DEFINE INDEX IF NOT EXISTS idx_measurements_daily_patient_bucket ON TABLE measurements_daily COLUMNS patient_id, timestamp UNIQUE;

-- Trail del job (spec 030: "se registra en la tabla retention_job"). Una sola
-- fila `last` guarda el reporte de la última ejecución, para GET /status.
DEFINE TABLE IF NOT EXISTS retention_jobs SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS run_at ON TABLE retention_jobs TYPE string;
DEFINE FIELD IF NOT EXISTS report ON TABLE retention_jobs TYPE object;