-- SPEC-032: ML Serving — registro de modelos de inferencia.
-- Tabla `ml_model`: cada modelo registrado (ews, severity, readmission) con
-- versión, formato (onnx/wasm), checksum de integridad y estado de carga.

DEFINE TABLE IF NOT EXISTS ml_model SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS model_id ON TABLE ml_model TYPE string;
DEFINE FIELD IF NOT EXISTS name ON TABLE ml_model TYPE string;
DEFINE FIELD IF NOT EXISTS version ON TABLE ml_model TYPE string;
DEFINE FIELD IF NOT EXISTS format ON TABLE ml_model TYPE string;
DEFINE FIELD IF NOT EXISTS path ON TABLE ml_model TYPE string;
DEFINE FIELD IF NOT EXISTS sha256 ON TABLE ml_model TYPE string;
DEFINE FIELD IF NOT EXISTS loaded ON TABLE ml_model TYPE bool DEFAULT false;
DEFINE FIELD IF NOT EXISTS loaded_at ON TABLE ml_model TYPE option<string> DEFAULT NONE;
DEFINE FIELD IF NOT EXISTS active ON TABLE ml_model TYPE bool DEFAULT false;
DEFINE INDEX IF NOT EXISTS idx_ml_model_name ON TABLE ml_model COLUMNS name, version UNIQUE;