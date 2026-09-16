# SPEC-025: Multi-tenancy — Aislamiento de datos por hospital

## Contexto

- **Problema a resolver**: dMart actualmente opera como sistema single-tenant. Para desplegar en múltiples hospitales (o múltiples unidades dentro de un hospital) se necesita aislamiento de datos: cada tenant solo ve sus pacientes, mediciones y scores. El aislamiento debe ser a nivel de DB (no solo aplicación).
- **Usuario objetivo**: Administrador hospitalario / DevOps
- **Métrica de éxito (KPI)**: 0 datos cruzados entre tenants; latencia sin overhead medible (< 2%); un tenant no puede acceder a datos de otro ni siquiera con request malicioso

## Acceptance Criteria (Gherkin)

```gherkin
Feature: Multi-tenancy
  As a administrador hospitalario
  I want que mi hospital solo vea sus propios datos
  So that se garantice la privacidad y confidencialidad de los pacientes

  Scenario: Tenant isolation a nivel DB
    Given hospital "Hospital A" (tenant=hosp-a) con pacientes
    And hospital "Hospital B" (tenant=hosp-b) con pacientes
    When un usuario de hosp-a consulta GET /api/patients
    Then solo ve pacientes de hosp-a
    And no hay forma de acceder a pacientes de hosp-b

  Scenario: Creación de paciente con tenant
    Given usuario autenticado de tenant "hosp-a"
    When crea un paciente POST /api/patients
    Then el campo `tenant_id` se asigna automáticamente a "hosp-a"
    And el paciente no es visible para otros tenants

  Scenario: Tenant admin crea usuario
    Given tenant admin de "hosp-a"
    When crea un usuario para hosp-a
    Then el usuario queda vinculado a hosp-a
    And no puede ser reasignado a hosp-b

  Scenario: Cross-tenant access attempt
    Given usuario autenticado de "hosp-a"
    When intenta acceder a `/api/patients/hosp-b:paciente-123`
    Then recibe 404 (no 403, para no revelar existencia)
    And audit log registra el intento

  Scenario: Admin global ve todos los tenants
    Given usuario con rol `super_admin`
    When consulta tenants
    Then ve la lista de todos los hospitales
    And puede impersonar un tenant para soporte

  Scenario: Migración de datos entre tenants
    Given paciente incorrectlyamente asignado a hosp-b
    When super_admin ejecuta migración a hosp-a
    Then el paciente y todos sus datos se mueven a hosp-a
    And hosp-b ya no tiene acceso a esos datos
    And audit log registra la migración
```

## API Contracts

### Tenant Context (middleware)

```rust
// Extraído del JWT o header
struct TenantContext {
    tenant_id: String,     // ej: "hosp-a"
    user_id: String,
    roles: Vec<String>,    // ["medico", "viewer"]
}

// Se inyecta en cada request handler
// Se usa en cada query SurrealDB: WHERE tenant_id = $tenant
```

### Endpoints modificados

| Método | Path | Cambio | Ejemplo |
|--------|------|--------|---------|
| GET | `/api/patients` | Filtra por tenant_id automático | Solo pacientes del tenant |
| POST | `/api/patients` | Inyecta tenant_id | tenant_id del JWT |
| GET | `/api/patients/{id}` | Verifica tenant | 404 si id no pertenece al tenant |
| GET | `/api/measurements` | Filtra por tenant (vía patient) | Solo mediciones del tenant |
| POST | `/api/teleicu/sessions` | Filtra por tenant | Solo sesiones del tenant |

### Tenant Management API (solo super_admin)

| Método | Path | Auth | Request | Response |
|--------|------|------|---------|----------|
| GET | `/api/admin/tenants` | Bearer + super_admin | — | `[{id, name, patient_count}]` |
| POST | `/api/admin/tenants` | Bearer + super_admin | `{name, slug}` | `201 {id}` |
| POST | `/api/admin/tenants/{id}/impersonate` | Bearer + super_admin | — | `200 {tenant_id, expires_at}` |

### Request Schema (Tenant creation)

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "required": ["name", "slug"],
  "properties": {
    "name": { "type": "string", "minLength": 3, "maxLength": 100 },
    "slug": { "type": "string", "pattern": "^[a-z0-9-]{3,50}$" }
  },
  "additionalProperties": false
}
```

## Data Models

### SurrealDB Schema

```sql
-- Tabla de tenants
DEFINE TABLE tenant SCHEMAFULL;
DEFINE FIELD name ON tenant TYPE string;
DEFINE FIELD slug ON tenant TYPE string ASSERT string::is::slug($value);
DEFINE FIELD created_at ON tenant TYPE datetime;
DEFINE FIELD active ON tenant TYPE bool DEFAULT true;
DEFINE INDEX idx_tenant_slug ON tenant COLUMNS slug UNIQUE;

-- Campo tenant_id en todas las tablas de datos
-- patients:
DEFINE FIELD tenant_id ON patient TYPE string;
DEFINE INDEX idx_patient_tenant ON patient COLUMNS tenant_id;

-- measurements:
DEFINE FIELD tenant_id ON measurement TYPE string;
DEFINE INDEX idx_measurement_tenant ON measurement COLUMNS tenant_id;

-- scores:
DEFINE FIELD tenant_id ON score TYPE string;
DEFINE INDEX idx_score_tenant ON score COLUMNS tenant_id;

-- timeline_events:
DEFINE FIELD tenant_id ON timeline_event TYPE string;
DEFINE INDEX idx_timeline_tenant ON timeline_event COLUMNS tenant_id;

-- users:
DEFINE FIELD tenant_id ON user TYPE string;
DEFINE INDEX idx_user_tenant ON user COLUMNS tenant_id;
```

### RBAC con Tenant Scope

```rust
// JWT claims extendidos
struct Claims {
    sub: String,          // user_id
    tenant_id: String,    // hospital slug
    roles: Vec<String>,   // ["medico"]
    exp: u64,
    iat: u64,
}

// Permisos con scope de tenant
// patients:read:hosp-a   → solo lectura en hosp-a
// patients:write:hosp-a  → solo escritura en hosp-a
```

### Query Pattern

```sql
-- Todas las queries incluyen tenant_id
SELECT * FROM patient WHERE tenant_id = $tenant AND active = true;
SELECT * FROM measurement WHERE patient_id = $patient_id AND tenant_id = $tenant;
-- Ninguna query puede omitir tenant_id (RLS policy)
```

## Edge Cases

| # | Caso | Comportamiento esperado |
|---|------|------------------------|
| 1 | Patient ID collision entre tenants | IDs son RecordId globales (surreal::rand()); tenant_id previene acceso |
| 2 | Backup contiene datos de todos los tenants | Restore mantiene aislamiento; tenant_id preservado |
| 3 | Múltiples tabs del mismo usuario en 2 tenants | Impossible: JWT tiene 1 tenant_id |
| 4 | Tenant desactivado | Todos los usuarios del tenant quedan out; datos preservados |
| 5 | Usuario sin tenant_id | Rechazado por middleware; 401 |
| 6 | Admin impersona tenant | Crea JWT temporal con tenant_id del target; audit log |
| 7 | Query sin tenant_id (legacy) | Middleware injecta; si no → rechazo |

## Security Considerations

### Threat Model (STRIDE)

| Threat | Mitigación |
|--------|------------|
| Spoofing | JWT firmado; tenant_id validated against DB |
| Tampering | tenant_id en JWT (no modificable por cliente); server-side validation |
| Repudiation | Audit log de cada cross-tenant access attempt |
| Information Disclosure | Queries siempre filtradas por tenant; 404 (no 403) en cross-tenant |
| Denial of Service | Rate limit por tenant; quota de pacientes por tenant |
| Elevation of Privilege | RBAC check antes de tenant impersonation; super_admin only |

### Row-Level Security Pattern

```rust
// Toda query pasa por este wrapper
fn with_tenant_filter(query: &str, tenant: &str) -> String {
    // Inyecta WHERE tenant_id = $tenant
    // Nunca permite queries sin filtro de tenant
}
```

### Data Classification
- [x] PHI (Protected Health Information) — aislado por tenant
- [x] Clinical Data — aislado por tenant
- [x] Operational/Metadata — tenant_id es metadata

## Testing Strategy

### Unit Tests
- [ ] TenantContext se extrae correctamente del JWT
- [ ] with_tenant_filter inyecta WHERE correcto
- [ ] JWT sin tenant_id es rechazado

### Integration Tests
- [ ] 2 tenants creados con datos separados
- [ ] Usuario de tenant A no ve datos de tenant B
- [ ] Cross-tenant access retorna 404
- [ ] Admin impersonation funciona
- [ ] Tenant desactivado bloquea acceso
- [ ] Creación de paciente asigna tenant_id automáticamente

### Security Tests
- [ ] SQL/SurrealQL injection con tenant_id manipulado
- [ ] JWT con tenant_id modificado → rechazado
- [ ] Brute-force cross-tenant enumeration → rate limited

### Load Test (k6)
- [ ] Multi-tenant: 100 usuarios de 10 tenants simultáneos
- [ ] Overhead de tenant filter < 2% vs single-tenant

## Rollout Plan

### Feature Flag
```rust
// config.rs
multi_tenant: {
    enabled: false,   // default off (single-tenant mode)
    default_tenant: "default",
}
```

### Canary Deployment
| Paso | Tráfico | Duración | Criterio go/no-go |
|------|---------|----------|-------------------|
| 1 | Single-tenant (sin tenant_id) | — | Baseline |
| 2 | Multi-tenant habilitado, 1 tenant | 1 semana | Datos aislados correctamente |
| 3 | 2+ tenants en staging | 2 semanas | Cross-tenant protection funciona |
| 4 | Producción con 1 tenant real | — | Overhead < 2% |

### Rollback Procedure
1. `multi_tenant.enabled = false` → vuelve a single-tenant
2. Los datos existentes mantienen tenant_id (no loss)
3. Queries vuelven a no filtrar por tenant

## Definition of Done

- [ ] `specs/025-multi-tenancy.md` (este archivo)
- [ ] Tabla `tenant` definida en SurrealDB
- [ ] Campo `tenant_id` en todas las tablas de datos
- [ ] Middleware inyecta tenant_id en cada request
- [ ] Queries filtradas por tenant (RLS pattern)
- [ ] Tenant Management API (solo super_admin)
- [ ] JWT claims con tenant_id
- [ ] Tests: unit + integration + security
- [ ] Documentación: `docs/MULTI_TENANCY.md`
- [ ] ROADMAP, CHANGELOG actualizados

## Gate CI SPEC-025
```bash
cargo test -p dmart-server --test multi_tenant 2>&1 | grep "test result: ok"
cargo clippy -p dmart-server --lib --bin dmart-server -- -D warnings 2>&1 | grep '^error' | wc -l  # = 0
```

## Métricas
- Tenant isolation: 0 cross-tenant data leaks
- Query overhead: < 2% (con tenant filter)
- Tenant creation: < 5s
- Impersonation audit: 100% logged
