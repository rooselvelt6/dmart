-- SPEC-052 / auditoría H4: cifrado de PHI en `push_subscription`.
--
-- El campo `user_agent` y `endpoint` pueden revelar información del usuario/dispositivo.
-- Se mueven al envelope AES-256-GCM `phi` con AAD (tenant + user_id + endpoint hash).
-- Queda en claro `user_id` + `created_at` para listado por usuario.

DEFINE FIELD IF NOT EXISTS phi ON TABLE push_subscription TYPE option<string>;

DEFINE INDEX IF NOT EXISTS idx_push_subscription_phi_lookup
    ON push_subscription FIELDS user_id, endpoint;