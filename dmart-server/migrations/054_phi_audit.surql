-- SPEC-052 / auditoría H4: cifrado de PHI en `audit_logs` payload.
--
-- El campo `details` (que puede contener PHI como nombres de pacientes, IDs, etc.)
-- se mueve al envelope AES-256-GCM `phi` con AAD (tenant + audit_log.uid).
-- **IMPORTANTE**: Se preservan `prev_hash`, `content_hash`, `timestamp`, `uid`,
-- `user_id`, `action`, `resource`, `resource_id`, `success`, `ip_address`,
-- `user_agent` en claro para mantener el encadenamiento WORM y verificación.

DEFINE FIELD IF NOT EXISTS phi ON TABLE audit_logs TYPE option<string>;

DEFINE INDEX IF NOT EXISTS idx_audit_logs_phi_lookup
    ON audit_logs FIELDS tenant_id, uid;