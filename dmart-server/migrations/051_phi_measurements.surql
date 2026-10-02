-- SPEC-052 / auditoría H4: cifrado de PHI en `measurements`.
--
-- Los signos vitales y datos clínicos detallados (ApacheIIData, GcsData) se
-- mueven al envelope AES-256-GCM `phi` con AAD (tenant + measurement_id).
-- Quedan en claro solo los campos necesarios para filtrar, ordenar y agregar:
-- ids, tenant, timestamps, scores calculados y severidad.

DEFINE FIELD IF NOT EXISTS phi ON TABLE measurements TYPE option<string>;

-- Índice para búsqueda por tenant + measurement_id (filtrado sin PHI)
DEFINE INDEX IF NOT EXISTS idx_measurements_phi_lookup
    ON measurements FIELDS tenant_id, measurement_id;