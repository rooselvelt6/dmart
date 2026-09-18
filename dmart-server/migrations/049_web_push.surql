-- SPEC-052 / tarea 3.9 — Web Push (VAPID)
-- Suscripciones del navegador por usuario y almacén de claves VAPID.
-- Idempotente (IF NOT EXISTS).

DEFINE TABLE IF NOT EXISTS push_subscription SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS user_id ON push_subscription TYPE string;
DEFINE FIELD IF NOT EXISTS endpoint ON push_subscription TYPE string;
DEFINE FIELD IF NOT EXISTS p256dh ON push_subscription TYPE string;
DEFINE FIELD IF NOT EXISTS auth ON push_subscription TYPE string;
DEFINE FIELD IF NOT EXISTS user_agent ON push_subscription TYPE option<string>;
DEFINE FIELD IF NOT EXISTS created_at ON push_subscription TYPE string;

DEFINE INDEX IF NOT EXISTS idx_push_user_endpoint ON TABLE push_subscription
    COLUMNS user_id, endpoint UNIQUE;

DEFINE TABLE IF NOT EXISTS push_config SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS key ON push_config TYPE string;
DEFINE FIELD IF NOT EXISTS value ON push_config TYPE string;

DEFINE INDEX IF NOT EXISTS idx_push_config_key ON TABLE push_config COLUMNS key UNIQUE;
