-- SPEC-017: Device Registry — registro y estado de dispositivos clínicos.
-- Tabla `device_registry` almacena monitores, ventiladores y bombas de
-- infusión conectados a la UCI: identidad (fabricante/modelo/serial),
-- ubicación física (cama opcional), firmware y ciclo de vida de conexión
-- (estado online/offline/mantenimiento + ventana de heartbeat).
--
-- Los estados se modelan como string (no enum) para que la tabla siga siendo
-- legible/auditable en SurrealQL y el mapeo se valide a nivel de aplicación
-- (mismo patrón que patient_event.event_type en SPEC-015).

DEFINE TABLE IF NOT EXISTS device_registry SCHEMAFULL;

-- Tipo de dispositivo clínico: monitor | ventilador | bomba
DEFINE FIELD IF NOT EXISTS device_type ON device_registry TYPE string;
-- Fabricante del dispositivo (ej: Philips, Draeger, B.Braun)
DEFINE FIELD IF NOT EXISTS fabricante ON device_registry TYPE string;
-- Modelo comercial (ej: IntelliVue MX450, Evita V300)
DEFINE FIELD IF NOT EXISTS modelo ON device_registry TYPE string;
-- Versión de firmware instalada
DEFINE FIELD IF NOT EXISTS firmware ON device_registry TYPE string;
-- Número de serie único del dispositivo
DEFINE FIELD IF NOT EXISTS serial ON device_registry TYPE string;
-- Cama asignada (referencia a tabla camas; opcional mientras está en stock)
DEFINE FIELD IF NOT EXISTS cama_id ON device_registry TYPE option<string>;
-- Ubicación libre (ej: "UCI-02 | Modulo B") cuando no hay cama formal
DEFINE FIELD IF NOT EXISTS ubicacion ON device_registry TYPE option<string>;
-- Estado operativo: online | offline | mantenimiento
DEFINE FIELD IF NOT EXISTS estado ON device_registry TYPE string DEFAULT 'online';
-- Timestamp de registro (epoch millis UTC)
DEFINE FIELD IF NOT EXISTS registered_at ON device_registry TYPE int;
-- Último heartbeat recibido (epoch millis UTC; NULL si nunca heartbeateó)
DEFINE FIELD IF NOT EXISTS last_seen_at ON device_registry TYPE option<int>;
-- Intervalo nominal de heartbeat en segundos (define la ventana de
-- actividad; una ausencia > 3× el intervalo marca el dispositivo offline)
DEFINE FIELD IF NOT EXISTS heartbeat_interval_secs ON device_registry TYPE int DEFAULT 60;

-- Índices: filtros por estado, agregación por tipo, serial único y cama.
DEFINE INDEX IF NOT EXISTS idx_device_registry_estado ON device_registry COLUMNS estado;
DEFINE INDEX IF NOT EXISTS idx_device_registry_type ON device_registry COLUMNS device_type;
DEFINE INDEX IF NOT EXISTS idx_device_registry_serial ON device_registry COLUMNS serial UNIQUE;
DEFINE INDEX IF NOT EXISTS idx_device_registry_cama ON device_registry COLUMNS cama_id;
DEFINE INDEX IF NOT EXISTS idx_device_registry_last_seen ON device_registry COLUMNS last_seen_at;