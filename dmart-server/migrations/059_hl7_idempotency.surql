-- PLAN39 P1.4: idempotencia de reintentos en la ingestión HL7.
-- HL7 v2 no garantiza entrega exactly-once: si un emisor MLLP reintenta, o si
-- se pierde el ACK y se reintenta, el pipeline ORU creaba una segunda
-- `Measurement` (Apache II / SOFA / GCS) y volvía a actualizar la severidad
-- del paciente. Duplicar un SOFA altera la evolución clínica, así que
-- `MSH.10` (Message Control ID) se usa como clave de idempotencia: existe
-- exactamente para eso.
--
-- La clave es `(tenant_id, message_id)`: `MSH.10` sólo es único **por emisor**,
-- y dos hospitales distintos generan el mismo `MSH.10` sin que sea un reintento.
--
-- La tabla no contiene PHI: sólo identificadores técnicos (`MSH.10`, tenant,
-- emisor, `measurement_id` UUID) más la marca temporal, para poder depurar un
-- reintento sin abrir una medición sellada.
--
-- El índice UNIQUE es lo que **decide**: la ingesta inserta la clave antes de
-- crear la medición y sólo la primera inserción tiene éxito, así que dos
-- réplicas simultáneas con el mismo `MSH.10` no pueden colarse las dos (un
-- `SELECT` previo no_valdría: siempre tiene una ventana). Ver `hl7::ingest`.
--
-- `measurement_id` se rellena **después** de crear la medición, para que un
-- reintento devuelva exactamente el mismo resultado. Si el proceso muere entre
-- ambos pasos, la fila queda sin enlace y el reintento se resuelve con la
-- última medición del paciente.

DEFINE TABLE IF NOT EXISTS hl7_ingest_key SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS tenant_id ON hl7_ingest_key TYPE string;
DEFINE FIELD IF NOT EXISTS message_id ON hl7_ingest_key TYPE string;
DEFINE FIELD IF NOT EXISTS measurement_id ON hl7_ingest_key TYPE option<string>;
DEFINE FIELD IF NOT EXISTS sender ON hl7_ingest_key TYPE string;
DEFINE FIELD IF NOT EXISTS source ON hl7_ingest_key TYPE string;
DEFINE FIELD IF NOT EXISTS created_at ON hl7_ingest_key TYPE datetime DEFAULT time::now();

DEFINE INDEX IF NOT EXISTS idx_hl7_ingest_key_tenant_message ON hl7_ingest_key COLUMNS tenant_id, message_id UNIQUE;
DEFINE INDEX IF NOT EXISTS idx_hl7_ingest_key_measurement ON hl7_ingest_key COLUMNS measurement_id;