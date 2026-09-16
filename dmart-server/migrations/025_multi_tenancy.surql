-- SPEC-025: Multi-tenancy — aislamiento de datos por hospital.
-- Tabla `tenant`: catálogo de hospitales/tenants con slug único.
-- `tenant_id` en tablas de datos para el filtrado RLS (patients, measurements, users).

DEFINE TABLE IF NOT EXISTS tenant SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS name ON TABLE tenant TYPE string;
DEFINE FIELD IF NOT EXISTS slug ON TABLE tenant TYPE string;
DEFINE FIELD IF NOT EXISTS active ON TABLE tenant TYPE bool DEFAULT true;
DEFINE FIELD IF NOT EXISTS created_at ON TABLE tenant TYPE string;
DEFINE INDEX IF NOT EXISTS idx_tenant_slug ON TABLE tenant COLUMNS slug UNIQUE;

-- Aislamiento por tenant en las tablas de datos (pacientes, mediciones, usuarios).
DEFINE FIELD IF NOT EXISTS tenant_id ON TABLE patients TYPE string;
DEFINE INDEX IF NOT EXISTS idx_patients_tenant ON TABLE patients COLUMNS tenant_id;
DEFINE FIELD IF NOT EXISTS tenant_id ON TABLE measurements TYPE string;
DEFINE INDEX IF NOT EXISTS idx_measurements_tenant ON TABLE measurements COLUMNS tenant_id;
DEFINE FIELD IF NOT EXISTS tenant_id ON TABLE users TYPE string;
DEFINE INDEX IF NOT EXISTS idx_users_tenant ON TABLE users COLUMNS tenant_id;