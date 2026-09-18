# SPEC-051: UIs operativas de Fase 3 (dispositivos, calidad, escalamiento, auditoría, dashboard)

## Contexto
- **Problema a resolver**: las capacidades de la Fase 3 (SPEC-017 dispositivos,
  SPEC-018 calidad de datos, SPEC-019 escalamiento, auditoría HIPAA, KPIs de
  operación) existen solo como API. El roadmap 3.10–3.14 exige pantallas
  navegables para operarlas sin CLI/SQL.
- **Usuario objetivo**: Admin (operación y configuración), Médico/Enfermero
  (escalamiento clínico), Soporte (lectura de dispositivos/calidad).
- **Métrica de éxito (KPI)**: 5 pantallas accesibles desde la barra lateral,
  gated por RBAC, con datos en vivo desde las APIs existentes; `trunk build
  --release` y el gate de tests del server verdes.

## Acceptance Criteria (Gherkin)

```gherkin
Feature: UIs operativas Fase 3

  Scenario: Dispositivos (/devices) — 3.10
    Given un usuario con devices:read
    When abre /devices
    Then ve el resumen (total, por estado, por tipo) y la tabla de dispositivos
    And si tiene devices:write puede registrar un dispositivo y enviar heartbeat
    But un usuario sin devices:read es redirigido a /

  Scenario: Calidad de datos (/data-quality) — 3.11
    Given un usuario con quality:read
    When abre /data-quality
    Then ve el total de issues, el desglose por severidad y por código,
         y la lista de los últimos issues
    And un usuario sin quality:read es redirigido a /

  Scenario: Escalamiento (/escalation) — 3.12
    Given un usuario con escalation:read
    When abre /escalation
    Then ve las escalaciones activas ordenadas por severidad
         y las políticas por severidad
    And si tiene escalation:act puede acusar (ack) y escalar una alerta
    And solo con config:write puede editar políticas

  Scenario: Auditoría HIPAA en Admin — 3.13
    Given un administrador
    When abre la pestaña "Auditoría" de /admin
    Then navega los eventos de auditoría (filtro por severidad) y
         ve las estadísticas de uci_stats

  Scenario: Dashboard 2.0 — 3.14
    Given un usuario autenticado
    When abre /
    Then ve 6 KPIs operativos y el estado de salud del sistema

  Scenario: Seguridad
    Given un usuario sin el permiso requerido
    When invoca directamente la API de la sección
    Then el servidor responde 403 (rbac::permission_for)
```

## API Contracts

Endpoints ya existentes, ahora gobernados por RBAC:

| Método | Path | Permiso | Response |
|--------|------|---------|----------|
| GET | `/api/devices` | `devices:read` | `200 Vec<ClinicalDevice>` |
| POST | `/api/devices` | `devices:write` | `201 ClinicalDevice` |
| GET | `/api/devices/status` | `devices:read` | `200 DeviceStatusSummary` |
| POST | `/api/devices/{id}/heartbeat` | `devices:write` | `200 ClinicalDevice` |
| GET | `/api/data-quality/report` | `quality:read` | `200 Vec<QualityIssue>` |
| GET | `/api/data-quality/summary` | `quality:read` | `200 QualitySummary` |
| POST | `/api/data-quality/validate` | `quality:write` | `200 Vec<QualityIssue>` |
| GET | `/api/escalation/active` | `escalation:read` | `200 Vec<Escalation>` |
| GET | `/api/escalation/policies` | `escalation:read` | `200 Vec<EscalationPolicy>` |
| POST | `/api/escalation/policies` | `config:write` | `200 EscalationPolicy` |
| POST | `/api/escalation/{id}/ack` | `escalation:act` | `200 Escalation` |
| POST | `/api/escalation/{id}/escalate` | `escalation:act` | `200 Escalation` |

Cambio de contrato: `Escalation.id` y `QualityIssue.id` se serializan como su
clave plana (`"abc-123"`) en JSON, no como el `RecordId` interno de SurrealDB.

## Data Models
- Sin tablas ni migraciones nuevas: se reutilizan `devices`, `quality_events`,
  `escalation_policies`, `escalations`, `audit_log`.
- RBAC: nuevos permisos `devices:read|write`, `quality:read|write`,
  `escalation:read|act` en `UserRole::permissions` (Admin); `escalation:*` para
  Médico/Enfermero, `escalation:read` para Viewer, `devices:read` y
  `quality:read` para Soporte.

## Edge Cases
| # | Caso | Comportamiento esperado |
|---|------|------------------------|
| 1 | Sin datos | Estado vacío claro, sin errores |
| 2 | API caída | `ErrorState` con reintento |
| 3 | Usuario sin permiso | Redirect a `/` (UI) y 403 (API) |
| 4 | Ack de escalación resuelta | 409 mostrado como feedback |

## Security Considerations
- Toda ruta nueva pasa por `rbac::permission_for`; deny by default.
- La UI solo oculta entradas; la autorización real es del servidor.
- No se expone PHI adicional ni la forma interna de `RecordId`.

## Testing Strategy
- Unit: `rbac::tests::permission_for_operational_routes` y
  `role_permissions_are_coherent`.
- Integración: suite existente de SPEC-017/018/019 sigue verde.
- Build: `trunk build --release` para las pantallas.
- Gate: `cargo test -p dmart-server --test api_tests --test hl7_integration --lib`.

## Definition of Done
- [ ] Spec aprobada.
- [ ] Pantallas 3.10–3.14 implementadas y ruteadas con gating.
- [ ] RBAC aplicado y testeado.
- [ ] `trunk build --release` + gate de tests verdes.
- [ ] CHANGELOG actualizado.
