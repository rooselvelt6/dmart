-- SPEC-052 / auditoría H4: cifrado de PHI en `reports`.
--
-- Los reportes clínicos contienen datos sensibles del paciente (diagnósticos, tratamientos).
-- Se mueven al envelope AES-256-GCM `phi` con AAD (tenant + report_id).
-- Queda en claro `patient_id`, `report_type`, `status`, `created_at`, `generated_by`
-- para filtrado y auditoría.

DEFINE FIELD IF NOT EXISTS phi ON TABLE reports TYPE option<string>;

DEFINE INDEX IF NOT EXISTS idx_reports_phi_lookup
    ON reports FIELDS tenant_id, report_id;