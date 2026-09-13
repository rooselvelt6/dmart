# SPEC Template — Spec-Driven Development (SDD)

> Copia este archivo a `specs/XXX-nombre-feature.md` y completa todas las secciones.
> **No escribas código hasta que la spec sea aprobada en PR.**

---

# SPEC-XXX: [Nombre de la Feature]

## Contexto
- **Problema a resolver**: [Descripción clara del problema]
- **Usuario objetivo**: [Rol: admin / médico / enfermero / paciente / sistema externo]
- **Métrica de éxito (KPI)**: [Ej: "Tiempo de ingreso GCS < 30s", "Accuracy ML > 92%", "MTTR < 15min"]

## Acceptance Criteria (Gherkin)

```gherkin
Feature: [Nombre corto]
  As a [rol]
  I want [acción]
  So that [beneficio]

  Scenario: [Caso principal]
    Given [estado inicial: usuario autenticado, paciente existe, etc.]
    When [acción: click botón, POST API, input datos]
    Then [resultado: redirect, JSON response, DB updated, event emitted]

  Scenario: [Caso borde: validación falla]
    Given [datos inválidos]
    When [intento de acción]
    Then [error 422 con mensaje claro, no side effects]

  Scenario: [Caso seguridad: sin permisos]
    Given [usuario sin rol requerido]
    When [intento de acción]
    Then [403 Forbidden, audit log entry]
```

## API Contracts

### Endpoints nuevos/modificados

| Método | Path | Auth | Request | Response | Errores |
|--------|------|------|---------|----------|---------|
| POST | `/api/v1/...` | Bearer + rol | `{...}` | `201 {id:...}` | 400, 401, 403, 422 |

### Request Schema (JSON Schema)
```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "required": ["campo1", "campo2"],
  "properties": {
    "campo1": { "type": "string", "maxLength": 100 },
    "campo2": { "type": "number", "minimum": 0, "maximum": 100 }
  },
  "additionalProperties": false
}
```

### Response Schema
```json
{
  "type": "object",
  "properties": {
    "id": { "type": "string", "format": "uuid" },
    "created_at": { "type": "string", "format": "date-time" }
  }
}
```

## Data Models

### Nuevos campos/tablas SurrealQL
```sql
-- Ejemplo
DEFINE TABLE nuevo_recurso SCHEMAFULL;
DEFINE FIELD campo1 ON nuevo_recurso TYPE string;
DEFINE FIELD campo2 ON nuevo_recurso TYPE float;
DEFINE INDEX idx_campo1 ON nuevo_recurso COLUMNS campo1;
```

### Migraciones requeridas
- `migrations/XXX_add_nuevo_recurso.surql`

## Edge Cases
| # | Caso | Comportamiento esperado |
|---|------|------------------------|
| 1 | Input vacío | 422 validation error |
| 2 | Valor fuera de rango clínico | 422 con mensaje médico claro |
| 3 | Concurrencia (dos usuarios editan mismo paciente) | Optimistic lock / merge strategy |
| 4 | Fallo DB parcial | Transacción rollback, 500 sin data corruption |

## Security Considerations

### Threat Model (STRIDE)
| Threat | Mitigación |
|--------|------------|
| Spoofing | JWT + RBAC check en handler |
| Tampering | Input validation + DB constraints |
| Repudiation | Audit log inmutable (append-only) |
| Information Disclosure | RBAC field-level, no PHI en logs |
| Denial of Service | Rate limit + request size limit |
| Elevation of Privilege | `require_role` en todas las rutas |

### Data Classification
- [ ] PHI (Protected Health Information)
- [ ] PII (Personally Identifiable Information)
- [ ] Clinical Data
- [ ] Operational/Metadata

### Auth/Autz Requirements
- Roles permitidos: [admin, medico, enfermero, viewer]
- Permisos requeridos: [`patients:read`, `measurements:write`]
- Scopes JWT: [`read:patients`, `write:measurements`]

## Testing Strategy

### Unit Tests (target: >90% coverage en lógica de dominio)
- [ ] `fn test_feature_happy_path()`
- [ ] `fn test_feature_validation_error()`
- [ ] `fn test_feature_edge_case_X()`

### Property-Based Tests (proptest)
- [ ] Bounds validation
- [ ] Monotonicidad / consistencia
- [ ] Round-trip serialization

### Fuzzing Targets (cargo-fuzz)
- [ ] JSON parsing endpoint
- [ ] Input sanitization

### Integration Tests
- [ ] Full HTTP flow con DB real (testcontainers o SurrealKV embedded)
- [ ] Auth flow (login → action → logout)

### Load Test (k6)
- [ ] Scenario: 100 VUs, 2 min, p95 < 500ms

## Rollout Plan

### Feature Flag
```rust
// En config
feature_flags: {
  nueva_feature: false  // default off
}
```

### Canary Deployment
| Paso | Tráfico | Duración | Criterio go/no-go |
|------|---------|----------|-------------------|
| 1 | 5% (internal) | 30 min | 0 errores 5xx, latencia p95 < baseline |
| 2 | 25% | 1 h | Métricas negocio estables |
| 3 | 100% | — | Rollout completo |

### Rollback Procedure
1. `feature_flags.nueva_feature = false` (instantáneo via config reload)
2. Si migración DB: `dmart migrate down XXX` (tested en staging)
3. Verificar métricas vuelven a baseline

## Definition of Done
- [ ] Spec aprobada en PR (1+ reviewer)
- [ ] Código implementado + `cargo clippy -D warnings` limpio
- [ ] Tests unit + prop + fuzz + integration pasando
- [ ] `cargo test --workspace` verde
- [ ] `cargo build --release` exitoso
- [ ] Documentación actualizada (README, CHANGELOG, ROADMAP)
- [ ] Manual QA contra acceptance criteria
- [ ] Deploy staging verificado
- [ ] Feature flag OFF en main (activar en release PR separado)