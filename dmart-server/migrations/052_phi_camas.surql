-- SPEC-052 / auditoría H4: cifrado de PHI en `camas`.
--
-- El nombre del paciente asignado a la cama (`paciente_nombre`) es PHI.
-- Se mueve al envelope AES-256-GCM `phi` con AAD (tenant + cama_id).
-- Queda en claro solo `paciente_id` (referencia) para poder unir con patients.

DEFINE FIELD IF NOT EXISTS phi ON TABLE camas TYPE option<string>;

-- Índice para lookup por tenant + cama_id
DEFINE INDEX IF NOT EXISTS idx_camas_phi_lookup
    ON camas FIELDS tenant_id, cama_id;