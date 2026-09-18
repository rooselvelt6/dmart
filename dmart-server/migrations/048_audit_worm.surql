-- SPEC-048 / tarea 3.8 — Auditoría inmutable (WORM)
-- Tabla de lotes sellados con encadenamiento + firma HMAC, e índice de hash
-- de contenido por evento. Idempotente (IF NOT EXISTS).

DEFINE TABLE IF NOT EXISTS audit_batches SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS batch_id ON audit_batches TYPE string;
DEFINE FIELD IF NOT EXISTS sequence ON audit_batches TYPE int;
DEFINE FIELD IF NOT EXISTS first_uid ON audit_batches TYPE string;
DEFINE FIELD IF NOT EXISTS last_uid ON audit_batches TYPE string;
DEFINE FIELD IF NOT EXISTS count ON audit_batches TYPE int;
DEFINE FIELD IF NOT EXISTS first_ts ON audit_batches TYPE string;
DEFINE FIELD IF NOT EXISTS last_ts ON audit_batches TYPE string;
DEFINE FIELD IF NOT EXISTS prev_batch_hash ON audit_batches TYPE string;
DEFINE FIELD IF NOT EXISTS batch_hash ON audit_batches TYPE string;
DEFINE FIELD IF NOT EXISTS signature ON audit_batches TYPE string;
DEFINE FIELD IF NOT EXISTS created_at ON audit_batches TYPE string;

DEFINE INDEX IF NOT EXISTS idx_audit_batches_sequence ON TABLE audit_batches COLUMNS sequence;
DEFINE INDEX IF NOT EXISTS idx_audit_logs_content_hash ON TABLE audit_logs COLUMNS content_hash;
