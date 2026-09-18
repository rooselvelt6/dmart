# SPEC-052: Web Push (VAPID) para alertas clínicas — tarea 3.9

## Contexto
- **Problema a resolver**: las alertas de escalamiento clínico solo se ven dentro de
  la app (SSE). Si el usuario no tiene la pestaña abierta, no se entera. Se necesita
  notificación push del navegador disparada por el backend cuando se crea una
  escalación.
- **Usuario objetivo**: médico / enfermero / soporte suscritos desde la app.
- **Métrica de éxito (KPI)**: notificación push entregada < 10 s tras crear la
  escalación; 0 PHI en el payload visible de la notificación.
- **Nota de roadmap**: el `roadmapFinal.md` referencia "SPEC-013", pero SPEC-013 es
  *FHIR bundle ingestion*. Esta spec es la fuente de verdad para Web Push.

## Acceptance Criteria (Gherkin)

```gherkin
Feature: Web Push de alertas clínicas
  As a clínico suscrito a notificaciones
  I want recibir una notificación del navegador al dispararse una escalación
  So that reacciono aunque no tenga la app abierta

  Scenario: Obtener la clave pública VAPID
    Given el servidor está arriba con claves VAPID cargadas o generadas
    When hago GET /push/vapid
    Then recibo 200 { data: { public_key: "<base64url uncompressed P-256>" } }
    And no se expone la clave privada

  Scenario: Registrar una suscripción del navegador
    Given un usuario autenticado con permiso notifications:write
    When hago POST /push/subscribe con { endpoint, p256dh, auth }
    Then recibo 201 y la suscripción queda asociada al user_id
    And el endpoint se guarda normalizado por (user_id, endpoint)

  Scenario: Reemplazar una suscripción existente
    Given ya existe una suscripción para el mismo (user_id, endpoint)
    When repito POST /push/subscribe con el mismo endpoint
    Then recibo 200 y no se duplica el registro

  Scenario: Baja de suscripción
    Given una suscripción registrada
    When hago DELETE /push/unsubscribe con { endpoint }
    Then recibo 200 y la suscripción desaparece

  Scenario: Envío ante escalación
    Given un paciente con una regla de escalamiento activa
    And al menos una suscripción registrada
    When se crea una escalación
    Then el backend cifra el payload (RFC 8291 aes128gcm) y lo envía al endpoint
    And la notificación no incluye nombre/cedula del paciente (solo tipo + nivel)

  Scenario: Suscripción expirada
    Given el servicio push responde 404/410 Gone
    When el backend intenta enviar
    Then la suscripción se elimina y el error no rompe el flujo clínico

  Scenario: Sin permisos
    Given un usuario sin notifications:write
    When intenta POST /push/subscribe
    Then recibo 403 y no se crea registro
```

## API Contracts

### Endpoints nuevos

| Método | Path | Auth | Request | Response | Errores |
|--------|------|------|---------|----------|---------|
| GET | `/api/push/vapid` | Bearer `notifications:read` | — | `{data:{public_key}}` | 401, 403 |
| POST | `/api/push/subscribe` | Bearer `notifications:write` | `{endpoint,p256dh,auth}` | `201 {id}` | 400, 401, 403, 422 |
| DELETE | `/api/push/unsubscribe` | Bearer `notifications:write` | `{endpoint}` | `200 {removed}` | 401, 403, 422 |
| POST | `/api/push/test` | Bearer `support:act` | `{title,body}` | `200 {sent,failed}` | 401, 403 |

### Request Schema
```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "required": ["endpoint", "p256dh", "auth"],
  "properties": {
    "endpoint": { "type": "string", "minLength": 8, "maxLength": 2048 },
    "p256dh": { "type": "string", "minLength": 8, "maxLength": 256 },
    "auth": { "type": "string", "minLength": 8, "maxLength": 256 }
  },
  "additionalProperties": false
}
```

## Data Models

### Nuevos campos/tablas SurrealQL
```sql
DEFINE TABLE push_subscription SCHEMAFULL;
DEFINE FIELD user_id ON push_subscription TYPE string;
DEFINE FIELD endpoint ON push_subscription TYPE string;
DEFINE FIELD p256dh ON push_subscription TYPE string;
DEFINE FIELD auth ON push_subscription TYPE string;
DEFINE FIELD user_agent ON push_subscription TYPE option<string>;
DEFINE FIELD created_at ON push_subscription TYPE string;
DEFINE INDEX idx_push_user_endpoint ON push_subscription COLUMNS user_id, endpoint UNIQUE;

DEFINE TABLE push_config SCHEMAFULL;
DEFINE FIELD key ON push_config TYPE string;
DEFINE FIELD value ON push_config TYPE string;
DEFINE INDEX idx_push_config_key ON push_config COLUMNS key UNIQUE;
```

### Migraciones requeridas
- `migrations/049_web_push.surql`

## Edge Cases
| # | Caso | Comportamiento esperado |
|---|------|------------------------|
| 1 | Endpoint no http(s) | 422; no se guarda |
| 2 | Sin suscripciones para el usuario | envío no-op, sin error clínico |
| 3 | Fallo de red hacia el servicio push | se registra y se continúa; nunca propaga al request clínico |
| 4 | 404/410 del push service | se borra la suscripción |
| 5 | Claves VAPID ausentes | se generan y persisten en `push_config` al inicializar |

## Security Considerations

### Threat Model (STRIDE)
| Threat | Mitigación |
|--------|------------|
| Spoofing | suscripción ligada al `user_id` del token |
| Tampering | payload firmado VAPID ES256 + cifrado aes128gcm |
| Repudiation | evento de auditoría por alta/baja y por envío |
| Information Disclosure | sin PHI en el payload; claves privadas solo en DB/estado servidor |
| Denial of Service | timeout corto en reqwest; envío best-effort fuera del camino crítico |
| Elevation of Privilege | `notifications:read/write`, `support:act` en RBAC |

### Data Classification
- [x] PHI (solo indirecta: tipo de alerta + nivel, sin identificadores)
- [ ] PII
- [x] Clinical Data
- [x] Operational/Metadata (suscripciones)

### Auth/Autz Requirements
- Permisos: `notifications:read`, `notifications:write`, `support:act`.
- Roles: Admin (wildcard), Soporte (`notifications:*`), Medico/Enfermero
  (`notifications:read`).

## Testing Strategy

### Unit Tests
- [ ] `vapid_jwt_has_es256_header_and_aud_matching_endpoint`
- [ ] `aes128gcm_body_has_expected_header_len_and_roundtrip_fields`
- [ ] `url_b64_roundtrip`

### Integration Tests (`dmart-server/tests/api_tests.rs`)
- [ ] `test_push_vapid_and_subscribe_flow`
- [ ] `test_push_unsubscribe`
- [ ] `test_push_requires_permission`

> El envío a un servicio push real (FCM/Mozilla) no se prueba en CI; se prueba la
> criptografía (JWT + cuerpo RFC 8291) y el flujo de suscripciones.

## Rollout Plan

### Feature Flag
`PUSH_ENABLED` (env, default `true` en dev). Sin VAPID configurado, el envío es no-op.

### Rollback Procedure
1. `PUSH_ENABLED=false` y reinicio.
2. Las suscripciones quedan inertes (no se borran datos).

## Definition of Done
- [ ] Spec aprobada
- [ ] `push.rs` implementado (VAPID ES256 + RFC 8291 con `ring`)
- [ ] Endpoints + RBAC + hook de escalación
- [ ] `sw.js` con handler `push` + `notificationclick`
- [ ] UI de activación/desactivación de notificaciones
- [ ] `cargo test -p dmart-server` (targets acotados) verde
- [ ] `trunk build --release` verde
- [ ] CHANGELOG actualizado
