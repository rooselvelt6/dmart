-- SPEC-052 / auditoría H4: cifrado de PHI en `device_registry`.
--
-- `fabricante`, `modelo`, `serial`, `firmware`, `ubicacion` pueden ser PII/identificadores.
-- Se mueven al envelope AES-256-GCM `phi` con AAD (tenant + device_id).
-- Queda en claro `device_type`, `cama_id`, `estado`, `registered_at`, `last_seen_at`,
-- `heartbeat_interval_secs` para filtrado y monitoreo.

DEFINE FIELD IF NOT EXISTS phi ON TABLE device_registry TYPE option<string>;

DEFINE INDEX IF NOT EXISTS idx_device_registry_phi_lookup
    ON device_registry FIELDS tenant_id, device_id;