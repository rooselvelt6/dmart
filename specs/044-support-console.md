# SPEC-044: Support Console — Consola Técnica de Soporte

## Contexto

- **Problema a resolver**: El equipo de soporte (o el administrador de turno)
  no tiene visibilidad del estado de **cada subsistema** del producto (base de
  datos, ingest HL7/MLLP, SSE/realtime, ML, monitores, auditoría, backups) ni
  forma de **corregir incidentes sin acceso a CLI/SSH**. Cada diagnóstico exige
  conexión al servidor y conocimiento tácito de runbooks. Se necesita una
  consola técnica en `/admin/soporte` (roles Admin/Soporte) que mida cada
  subsistema en vivo, sugiera la acción correcta según el síntoma y la ejecute
  de forma **auditable e idempotente**.
- **Usuario objetivo**: Administrador / Soporte técnico (sin acceso a shell).
- **Métrica de éxito (KPI)**: Tiempo medio de "minuto del problema → acción
  correcta" < 5 min sin SSH; cada subsistema con SLI medible (latencia p95,
  error rate, freshness SSE, gap/fault ingest); 100 % de las acciones
  registradas en auditoría (usuario + IP + resultado).

## Acceptance Criteria (Gherkin)

```gherkin
Feature: Consola Técnica de Soporte
  As a administrador o técnico de soporte
  I want medir cada subsistema y ejecutar correcciones desde la web
  So that pueda operar el sistema sin CLI/SSH y dejar trazabilidad completa

  Scenario: Estado de subsistemas en vivo
    Given un servidor con base de datos, ingest MLLP, SSE, ML, auditoría y backups
    When un Admin abre /admin/soporte
    Then veo una tarjeta por subsistema (DB, HL7/ingest, SSE/realtime, ML,
         monitores, auditoría, backups)
    And cada tarjeta muestra: estado (ok/degraded/error), latencia de la
         última sonda, contador de errores, y frescura del último evento
    And el estado se calcula consultando el sistema en vivo (no se inventa)

  Scenario: Diagnóstico guiado (runbook)
    Given un subsistema degradado (p.ej. ingest con circuit breaker abierto)
    When consulto el diagnóstico de ese subsistema
    Then recibo los SLIs medibles (latencia p95, error rate, freshness, gaps/faults)
    And recibo una lista de "minutos del problema → acción sugerida" priorizada

  Scenario: Acciones de corrección idempotentes y auditadas
    Given un problema diagnosticado
    When ejecuto una acción (reintentar ingest, reset circuit breaker, trigger
         backup, retención de auditoría on-demand, swap de modelo ML,
         verificación de fingerprints)
    Then la acción se ejecuta sin error repetirla
    And se registra en auditoría con usuario, IP, subsistema, resultado y detalle
    And la consola muestra el resultado y un histórico de acciones previas

  Scenario: Acceso restringido
    Given un usuario Viewer (sin permiso support:read)
    When intenta llamar GET /admin/support/systems
    Then recibe 403 Forbidden
    Given un usuario con permiso support:read pero sin support:act
    When intenta ejecutar una acción
    Then recibe 403 Forbidden

  Scenario: Self-healing de ingest
    Given un circuit breaker en Open con ventana half-open expirada
    When una nueva conexión MLLP envía el siguiente mensaje
    Then el circuit breaker transiciona automáticamente a half-open
    And se registra un evento de auto-recuperación en el histórico de la consola
    And el sistema sigue aceptando mensajes si recupera (sin pérdida de datos)

  Scenario: Autosnapshot historial
    Given el sistema en operación normal
    When la consola consulta el histórico
    Then veo los eventos de auto-recuperación y las acciones manuales ordenados
         por timestamp con origen (auto/manual)
```

## API Contracts

### Endpoints nuevos

| Método | Path | Auth | Permiso | Request | Response | Errores |
|--------|------|------|---------|---------|----------|---------|
| GET | `/api/v1/admin/support/systems` | Bearer | `support:read` | — | `Vec<SupportSystem>` | 401/403 |
| GET | `/api/v1/admin/support/diagnostics` | Bearer | `support:read` | — | `Vec<SupportDiagnostic>` | 401/403 |
| POST | `/api/v1/admin/support/actions/{action}` | Bearer | `support:act` | `ActionRequest` (opcional) | `SupportActionResult` | 401/403/400 |
| GET | `/api/v1/admin/support/history` | Bearer | `support:read` | `?limit=` | `Vec<SupportEvent>` | 401/403 |

#### `SupportSystem`
```json
{
  "key": "ingest",
  "name": "Ingest HL7/MLLP",
  "status": "ok | degraded | error",
  "latency_ms": 12.5,
  "error_count": 3,
  "freshness_secs": 4.2,
  "last_event": "2026-11-12T10:00:00Z",
  "details": "2 dispositivos activos, 1 en fault"
}
```

#### `SupportDiagnostic`
```json
{
  "key": "ingest",
  "system": "Ingest HL7/MLLP",
  "status": "degraded",
  "slis": [
    {"name": "gap_ingest_total", "value": 3, "target": "0", "slo_critical": false},
    {"name": "fault_devices", "value": 1, "target": "0", "slo_critical": true}
  ],
  "minutes": [
    {"minute": "R1: circuit breaker Open", "probable_causa": "Fallo repetido del monitor",
     "accion": "POST /admin/support/actions/circuit_reset",
     "impacto": "Mitigación", "prioridad": 1}
  ]
}
```

#### `SupportEvent`
```json
{
  "uid": "uuid",
  "timestamp": "RFC3339",
  "origin": "manual | auto",
  "subsystem": "ingest",
  "action": "circuit_reset",
  "user_id": "...",
  "username": "...",
  "ip_address": "...",
  "success": true,
  "message": "Circuit breaker transicionado a half-open",
  "details": {}
}
```

### Acciones soportadas (`POST /admin/support/actions/{action}`)

| action | Comportamiento | Idempotente |
|--------|----------------|-------------|
| `ingest_retry` | Resetea el estado de hardening de todos los dispositivos MLLP (fresh `IngestState`, persistido a cache) | Sí |
| `circuit_reset` | Transiciona a half-open todo circuit breaker `Open` | Sí |
| `backup` | Dispara `scripts/backup.sh` (o equivalente) con resultado (path/tamaño) | Sí (nueva copia por llamada) |
| `audit_retention` | Ejecuta `AuditService::cleanup_old_logs()` (retención 6 años on-demand) | Sí |
| `model_swap` | Activa `{name}@{version}` en el registry ML (body: `{"model": "ews", "version": "v1.2.0"}`) | Sí |
| `verify_fingerprints` | Re-ejecuta la verificación de fingerprints de scores y devuelve conteo `reproducible/total` | Sí |

### Errores
- `400` si el `system` limita el diagnóstico (param no soportado) o body inválido.
- `401` sin token válido; `403` sin permiso `support:read` / `support:act`.

### Impacto en métricas Prometheus (`metrics::register`)
- `support_actions_total{action,result}` — acciones de soporte ejecutadas.
- `self_healing_total{subsystem,result}` — recuperaciones automáticas.
- `support_systems_status{system}` gauge 1/0 según estado actual (scrape).

### RBAC
- Permiso nuevo `support:read` (ver tarjetas/diagnóstico/historial).
- Permiso nuevo `support:act` (ejecutar correcciones).
- Rol `Soporte` con `support:read` + `support:act` (sin resto de administración).
- `Admin` tiene ambos vía `"*"`.

## Validación (no romper contratos)

- Añade `UserRole::Soporte` (aditivo). Los roles existentes y registros
  previos en `data/dmart.db` siguen deserializando.
- No modifica endpoints existentes; solo añade `/admin/support/*`.
- `metrics::survey_db` conserva la superficie de `/metrics` (SPEC-005).
- Gate: `cargo test -p dmart-server --test api_tests --test hl7_integration --lib`.