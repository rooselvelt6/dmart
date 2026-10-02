-- SPEC-052 / auditoría H4: cifrado de PHI en `care_plan`.
--
-- Los datos del plan de cuidados (actividad, anotaciones, referencias a paciente)
-- se mueven al envelope AES-256-GCM `phi` con AAD (tenant + care_plan_id).
-- Queda en claro solo `patient_id` + `status` + `intent` para filtrado.

DEFINE FIELD IF NOT EXISTS phi ON TABLE care_plan TYPE option<string>;

DEFINE INDEX IF NOT EXISTS idx_care_plan_phi_lookup
    ON care_plan FIELDS tenant_id, care_plan_id;