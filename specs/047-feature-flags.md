# SPEC-047: Feature Flags Server

## Contexto
- **Problema a resolver**: Necesitamos activar/desactivar features por tenant sin redeploy. Actualmente no hay sistema de feature flags.
- **Usuario objetivo**: Admins (configuración), Sistema (evaluación en runtime)
- **Métrica de éxito (KPI)**: Feature flag evaluation < 1ms p99; cambios propagados < 5s

## Acceptance Criteria (Gherkin)

```gherkin
Feature: Feature Flags por Tenant
  As an admin
  I want gestionar feature flags por tenant desde la API
  So that pueda activar/desactivar funcionalidades sin redeploy

  Scenario: Crear feature flag
    Given admin autenticado
    When POST /api/v1/admin/flags con {key: "nueva_ui", enabled: true, tenant: "hospital-a"}
    Then 201 Created con flag creado

  Scenario: Evaluar flag en request
    Given flag "nueva_ui" enabled para tenant "hospital-a"
    When request con tenant "hospital-a" a endpoint protegido
    Then middleware inyecta flag en request extensions

  Scenario: Flag deshabilitada por defecto
    Given flag no existe para tenant
    When evalua flag
    Then devuelve false (fail-closed)

  Scenario: Override global
    Given flag global enabled=true, tenant override enabled=false
    When evalua para ese tenant
    Then devuelve false (tenant override gana)

  Scenario: Cache TTL
    Given flag actualizado en DB
    When pasan 5 segundos
    Then nuevo valor visible (TTL cache 5s)
```

## API Contracts

### Endpoints nuevos

| Método | Path | Auth | Request | Response | Errores |
|--------|------|------|---------|----------|---------|
| GET | `/api/v1/admin/flags` | Bearer + admin | - | `FlagListResponse` | 401, 403 |
| POST | `/api/v1/admin/flags` | Bearer + admin | `CreateFlagRequest` | `FlagResponse` | 400, 401, 403, 422 |
| GET | `/api/v1/admin/flags/{key}` | Bearer + admin | - | `FlagResponse` | 401, 403, 404 |
| PUT | `/api/v1/admin/flags/{key}` | Bearer + admin | `UpdateFlagRequest` | `FlagResponse` | 400, 401, 403, 404, 422 |
| DELETE | `/api/v1/admin/flags/{key}` | Bearer + admin | - | 204 | 401, 403, 404 |

### Request/Response Schemas

```json
{
  "FlagResponse": {
    "type": "object",
    "properties": {
      "key": {"type": "string"},
      "description": {"type": "string"},
      "enabled": {"type": "boolean"},
      "tenant_id": {"type": ["string", "null"]},
      "created_at": {"type": "string", "format": "date-time"},
      "updated_at": {"type": "string", "format": "date-time"}
    }
  },
  "CreateFlagRequest": {
    "type": "object",
    "required": ["key", "enabled"],
    "properties": {
      "key": {"type": "string", "pattern": "^[a-z0-9_]+$"},
      "description": {"type": "string"},
      "enabled": {"type": "boolean"},
      "tenant_id": {"type": ["string", "null"]}
    }
  },
  "UpdateFlagRequest": {
    "type": "object",
    "properties": {
      "description": {"type": "string"},
      "enabled": {"type": "boolean"},
      "tenant_id": {"type": ["string", "null"]}
    }
  }
}
```

## Data Models

### Tabla SurrealQL

```sql
DEFINE TABLE feature_flag SCHEMAFULL;
DEFINE FIELD key ON feature_flag TYPE string;
DEFINE FIELD description ON feature_flag TYPE string;
DEFINE FIELD enabled ON feature_flag TYPE bool;
DEFINE FIELD tenant_id ON feature_flag TYPE option<string>;
DEFINE FIELD created_at ON feature_flag TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON feature_flag TYPE datetime DEFAULT time::now();

DEFINE INDEX idx_feature_flag_key_tenant ON feature_flag COLUMNS key, tenant_id UNIQUE;
DEFINE INDEX idx_feature_flag_tenant ON feature_flag COLUMNS tenant_id;
```

### Migración
- `migrations/047_add_feature_flags.surql`

## Edge Cases

| # | Caso | Comportamiento esperado |
|---|------|------------------------|
| 1 | Key duplicada (mismo tenant) | 409 Conflict |
| 2 | Tenant no existe | 422 Validation error |
| 3 | Cache stale | TTL 5s, invalidación en write |
| 4 | Flag key inválida (caracteres especiales) | 422 Validation error |
| 5 | Middleware sin tenant en context | Usa global (tenant_id = null) |

## Security Considerations

### Threat Model (STRIDE)

| Threat | Mitigación |
|--------|------------|
| Spoofing | JWT + RBAC admin only |
| Tampering | Validación input + DB constraints |
| Repudiation | Audit log en create/update/delete |
| Information Disclosure | Solo admin ve flags; tenant solo ve sus flags |
| Denial of Service | Rate limit + cache TTL |
| Elevation of Privilege | `require_role("admin")` en todas las rutas |

### Data Classification
- [ ] PHI
- [ ] PII
- [ ] Clinical Data
- [x] Operational/Metadata

### Auth/Autz Requirements
- Roles permitidos: [admin]
- Permisos requeridos: [`flags:read`, `flags:write`]

## Testing Strategy

### Unit Tests
- [ ] `test_flag_crud()`
- [ ] `test_flag_evaluation_global()`
- [ ] `test_flag_evaluation_tenant_override()`
- [ ] `test_flag_cache_ttl()`
- [ ] `test_flag_default_false()`

### Integration Tests
- [ ] Full HTTP flow: create flag → evaluate via middleware → update → verify
- [ ] Multi-tenant isolation

### Load Test
- [ ] 1000 VUs evaluando flags, p99 < 1ms overhead

## Rollout Plan

### Feature Flag
```rust
// En config/env
DMART_FEATURE_FLAGS_ENABLED=true
DMART_FEATURE_FLAGS_CACHE_TTL_SECS=5
```

### Canary Deployment
| Paso | Tráfico | Duración | Criterio go/no-go |
|------|---------|----------|-------------------|
| 1 | 10% (internal) | 30 min | 0 errores, cache hit rate > 90% |
| 2 | 100% | — | Rollout completo |

### Rollback Procedure
1. `DMART_FEATURE_FLAGS_ENABLED=false` (instantáneo via env reload)
2. Verificar métricas vuelven a baseline

## Definition of Done
- [ ] Spec aprobada en PR (1+ reviewer)
- [ ] Código implementado + `cargo clippy -D warnings` limpio
- [ ] Tests unit + integration pasando
- [ ] `cargo test -p dmart-server --test api_tests` verde
- [ ] Documentación actualizada (README, CHANGELOG, ROADMAP)
- [ ] Manual QA contra acceptance criteria