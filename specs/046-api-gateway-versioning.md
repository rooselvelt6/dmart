# SPEC-046: API Gateway + Versionado v1

## Contexto
- **Problema a resolver**: La API actual está montada en `/api` sin versionado. Necesitamos prefijo `/api/v1`, cabecera de versión, OpenAPI (utoipa) generado en CI, y SDK cliente TypeScript generado automáticamente.
- **Usuario objetivo**: Desarrolladores frontend (dmart-app), integraciones externas, CI/CD.
- **Métrica de éxito (KPI)**: `openapi.json` válido publicado en CI; SDK generado y usado en dmart-app; 0 breaking changes sin bump de versión.

## Acceptance Criteria (Gherkin)

```gherkin
Feature: API Versioning v1
  As a desarrollador
  I want todas las rutas bajo /api/v1 con OpenAPI
  So that el frontend y clientes externos tengan contrato estable y versionado

  Scenario: Health check en v1
    Given servidor corriendo
    When GET /api/v1/health
    Then 200 OK con versión en header X-API-Version: v1

  Scenario: Endpoints existentes funcionan bajo /api/v1
    Given usuario autenticado con token
    When GET /api/v1/patients
    Then 200 OK con lista paginada

  Scenario: OpenAPI spec accesible
    Given servidor corriendo
    When GET /api/v1/openapi.json
    Then 200 OK con spec válido utoipa

  Scenario: Header de versión presente en todas las respuestas
    Given cualquier request a /api/v1/*
    Then response header X-API-Version: v1

  Scenario: Deprecation warning en v0 (legacy /api/*)
    Given request a /api/patients (sin v1)
    Then 200 OK + header X-API-Deprecated: true + warning en logs
```

## API Contracts

### Endpoints nuevos/modificados

| Método | Path | Auth | Request | Response | Errores |
|--------|------|------|---------|----------|---------|
| GET | `/api/v1/health` | Bearer opcional | - | `HealthResponse` | - |
| GET | `/api/v1/openapi.json` | Público | - | OpenAPI 3.1 JSON | - |
| * | `/api/v1/*` | Bearer + rol | según endpoint | según endpoint | 400, 401, 403, 422, 500 |
| * | `/api/*` (legacy) | Bearer + rol | según endpoint | según endpoint + header deprecation | 400, 401, 403, 422, 500 |

### Request/Response Schema

```json
{
  "openapi": "3.1.0",
  "info": {
    "title": "dMart UCI API",
    "version": "1.0.0",
    "description": "API para gestión de UCI - dMart"
  },
  "servers": [{ "url": "/api/v1" }],
  "components": {
    "securitySchemes": {
      "BearerAuth": { "type": "http", "scheme": "bearer", "bearerFormat": "JWT" }
    }
  },
  "security": [{ "BearerAuth": [] }]
}
```

## Data Models

No hay nuevos modelos de datos. Solo wrapping de rutas existentes.

## Edge Cases

| # | Caso | Comportamiento esperado |
|---|------|------------------------|
| 1 | Request a `/api/v2/...` (versión no existe) | 404 Not Found |
| 2 | Request a `/api/...` legacy | Funciona + header `X-API-Deprecated: true` + log warning |
| 3 | OpenAPI incluye solo rutas v1 | Filtra rutas legacy automáticamente |
| 4 | SDK generación falla en CI | CI fail; no se publica release |

## Security Considerations

### Threat Model (STRIDE)
| Threat | Mitigación |
|--------|------------|
| Spoofing | JWT validation igual que antes |
| Tampering | Input validation sin cambios |
| Repudiation | Audit log mantiene trazabilidad |
| Information Disclosure | OpenAPI no expone schemas internos sensibles |
| Denial of Service | Rate limit existente aplica a v1 |
| Elevation of Privilege | RBAC sin cambios |

### Data Classification
- [ ] PHI
- [ ] PII
- [ ] Clinical Data
- [x] Operational/Metadata (version headers, OpenAPI)

### Auth/Autz Requirements
- Roles permitidos: [admin, medico, enfermero, viewer, soporte]
- Permisos: sin cambios (hereda de endpoints actuales)
- Scopes JWT: sin cambios

## Testing Strategy

### Unit Tests
- [ ] `test_health_v1_returns_version_header`
- [ ] `test_openapi_json_valid_schema`
- [ ] `test_legacy_routes_deprecation_header`
- [ ] `test_version_header_on_all_v1_responses`

### Integration Tests
- [ ] Full HTTP flow: login → GET /api/v1/patients → logout
- [ ] OpenAPI spec served correctly
- [ ] Legacy routes still work with deprecation header

### Load Test (k6)
- [ ] Scenario: 100 VUs, 2 min, p95 < 100ms overhead vs legacy

## Rollout Plan

### Feature Flag
```rust
// En config/env
DMART_API_VERSION=v1  // default
DMART_API_LEGACY_ENABLED=true  // para transición
```

### Canary Deployment
| Paso | Tráfico | Duración | Criterio go/no-go |
|------|---------|----------|-------------------|
| 1 | 10% (internal) | 30 min | 0 errores 5xx, header version presente |
| 2 | 50% | 1 h | Métricas estables, frontend usa v1 |
| 3 | 100% | — | Legacy deprecated, solo v1 activo |

### Rollback Procedure
1. `DMART_API_LEGACY_ENABLED=true` + `DMART_API_VERSION=v0` (instantáneo via env reload)
2. Verificar métricas vuelven a baseline

## Definition of Done
- [ ] Spec aprobada en PR (1+ reviewer)
- [ ] Código implementado + `cargo clippy -D warnings` limpio
- [ ] Tests unit + integration pasando (`cargo test -p dmart-server --test api_tests`)
- [ ] `cargo build --release` exitoso
- [ ] `openapi.json` generado en CI artifact
- [ ] SDK TypeScript generado y commitado en `dmart-app/src/api.generated.ts`
- [ ] Frontend (dmart-app) migra llamadas a `/api/v1/*`
- [ ] CHANGELOG actualizado