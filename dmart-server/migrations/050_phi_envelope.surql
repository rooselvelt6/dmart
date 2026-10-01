-- SPEC-052 / auditoría H4: cifrado de PHI en reposo.
--
-- Los campos identificables del paciente (nombre, cédula, historia clínica,
-- fecha de nacimiento, dirección, contacto, etc.) dejan de persistirse en
-- claro: se guardan dentro del envelope AES-256-GCM de la columna `phi`, con
-- el tenant, el tipo de registro y el id como *associated data* (AAD), de modo
-- que el blob no pueda moverse a otro registro o a otro tenant.
--
-- Siguen en claro **sólo** las columnas necesarias para filtrar, ordenar o
-- agregar (ids, tenant, estado de gravedad, scores, fechas de ingreso/egreso y
-- desenlace). Ninguna de ellas identifica por sí sola a una persona, y todas
-- son necesarias para el filtrado en SQL.
--
-- La búsqueda exacta se resuelve con índices ciegos HMAC-SHA256
-- (`bi_hc`, `bi_ced`, `bi_nombre`): son deterministas pero no reversibles sin
-- la clave, así que permiten filtrar en la base sin exponer PHI. Los índices
-- se calculan con el tenant como parte del HMAC, por lo que el mismo MRN en
-- dos tenants produce valores distintos y no se puede correlacionar entre
-- tenants.
--
-- Nota: no se declaran `UNIQUE` a propósito. Las filas heredadas no tienen
-- `bi_*`, y un índice único sobre una columna ausente rechazaría los
-- INSERT de esas filas. La deduplicación queda en la capa de aplicación
-- (`phi_store`), que además distingue el fallback legacy del camino cifrado.

DEFINE INDEX IF NOT EXISTS idx_patients_phi_hc
    ON patients FIELDS tenant_id, bi_hc;

DEFINE INDEX IF NOT EXISTS idx_patients_phi_ced
    ON patients FIELDS tenant_id, bi_ced;

DEFINE INDEX IF NOT EXISTS idx_patients_phi_nombre
    ON patients FIELDS tenant_id, bi_nombre;

-- Búsqueda legacy (filas anteriores a la migración, sin índice ciego) y por
-- cama. Se mantiene la multicolumna existente de listados.
DEFINE INDEX IF NOT EXISTS idx_patients_tenant_created
    ON patients FIELDS tenant_id, created_at;
