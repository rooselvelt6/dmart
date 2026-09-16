-- SPEC-033: Patient Similarity Engine — embeddings clínicos por paciente.
-- Tabla `patient_embedding`: vector 128d (array de floats), versión del modelo
-- de embedding y features usadas. Un embedding por paciente (UNIQUE).

DEFINE TABLE IF NOT EXISTS patient_embedding SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS patient_id ON TABLE patient_embedding TYPE string;
DEFINE FIELD IF NOT EXISTS tenant_id ON TABLE patient_embedding TYPE string;
DEFINE FIELD IF NOT EXISTS embedding ON TABLE patient_embedding TYPE array;
DEFINE FIELD IF NOT EXISTS model_version ON TABLE patient_embedding TYPE string;
DEFINE FIELD IF NOT EXISTS features_used ON TABLE patient_embedding TYPE array;
DEFINE FIELD IF NOT EXISTS generated_at ON TABLE patient_embedding TYPE string;
DEFINE FIELD IF NOT EXISTS updated_at ON TABLE patient_embedding TYPE string;
DEFINE INDEX IF NOT EXISTS idx_emb_patient ON TABLE patient_embedding COLUMNS patient_id UNIQUE;
DEFINE INDEX IF NOT EXISTS idx_emb_tenant ON TABLE patient_embedding COLUMNS tenant_id;