# SPEC-004: Auth/Autz Hardening (JWT Refresh, RBAC Granular)

## Contexto
- **Problema**: Auth actual tiene access tokens de larga duración (24h), sin refresh tokens, RBAC básico (solo role check). Necesario: access tokens cortos (15min) + refresh tokens rotativos, RBAC por permisos granulares (resource:action), revocación inmediata en logout.
- **Usuario objetivo**: Security Engineer / Backend
- **Métrica de éxito (KPI)**: 0 advisories `cargo audit`, access token TTL 15min, refresh rotation funcionando, RBAC granular en 100% endpoints.

## Acceptance Criteria (Gherkin)

```gherkin
Feature: Auth/Autz Hardening
  As a Security Engineer
  I want short-lived access tokens with refresh rotation and granular RBAC
  So that token compromise has minimal blast radius

  Scenario: Login issues access + refresh token pair
    Given valid credentials
    When POST /auth/login
    Then response contains access_token (15min TTL) + refresh_token (7d TTL, httpOnly cookie)
    And refresh_token stored hashed in DB with user_id, expires_at, revoked=false

  Scenario: Access token refresh rotates refresh token
    Given valid refresh_token (not revoked, not expired)
    When POST /auth/refresh with refresh_token
    Then new access_token (15min) + new refresh_token (rotated, old revoked)
    And old refresh_token marked revoked=true in DB

  Scenario: Expired access token rejected, refresh works
    Given access_token expired 20min ago, valid refresh_token
    When GET /api/patients with expired access_token
    Then 401 Unauthorized
    And POST /auth/refresh succeeds with new pair

  Scenario: Revoked refresh token rejected
    Given user logged out (refresh_token revoked)
    When POST /auth/refresh with revoked token
    Then 401 Unauthorized, "token revoked"

  Scenario: Granular RBAC enforced on all endpoints
    Given user with role "medico" (perms: patients:read, measurements:write)
    When GET /api/patients → 200
    And POST /api/measurements → 201
    And DELETE /api/users → 403 (no users:delete perm)
    And GET /api/audit → 403 (no audit:read perm)

  Scenario: Concurrent refresh prevented (reuse detection)
    Given valid refresh_token
    When two concurrent POST /auth/refresh with same token
    Then first succeeds, second 401 "token reused" (possible theft)
    And user session revoked (security event logged)

  Scenario: Logout revokes both tokens immediately
    Given authenticated session
    When POST /auth/logout
    Then access_token blacklisted (Valkey, TTL = remaining access TTL)
    And refresh_token marked revoked in DB
    And subsequent requests with either token → 401
```

## API Contracts

### Endpoints nuevos/modificados

| Método | Path | Auth | Request | Response | Errores |
|--------|------|------|---------|----------|---------|
| POST | `/auth/login` | None | `{email, password, mfa_code?}` | `{access_token, expires_in: 900}` + `refresh_token` cookie | 401, 422, 429 |
| POST | `/auth/refresh` | None | `{refresh_token}` (cookie o body) | `{access_token, expires_in: 900}` + new `refresh_token` cookie | 401, 422 |
| POST | `/auth/logout` | Access + Refresh | — | `204 No Content` | 401 |
| POST | `/auth/revoke-all` | Access | — | `204` (revoca todos los refresh del user) | 401 |

### Request/Response Schemas

```json
// POST /auth/login
{ "email": "string", "password": "string", "mfa_code": "string?" }

// Response 200
{ "access_token": "string", "token_type": "Bearer", "expires_in": 900 }
// + Set-Cookie: refresh_token=...; HttpOnly; Secure; SameSite=Strict; Max-Age=604800

// POST /auth/refresh (body or cookie)
{ "refresh_token": "string" }
// Response: same as login
```

### RBAC Permissions Matrix

| Resource | Actions | Admin | Médico | Enfermero | Viewer |
|----------|---------|-------|--------|-----------|--------|
| patients | read, write, delete | ✓ | ✓ | ✓ | read |
| measurements | read, write | ✓ | ✓ | write | read |
| users | read, write, delete | ✓ | — | — | — |
| audit | read | ✓ | — | — | — |
| scales | read, write | ✓ | ✓ | write | read |
| admin | read, write | ✓ | — | — | — |
| fhir | read | ✓ | ✓ | ✓ | read |

## Data Models

### Nuevas tablas SurrealQL
```sql
-- Refresh tokens table
DEFINE TABLE refresh_token SCHEMAFULL;
DEFINE FIELD token_hash ON refresh_token TYPE string;  -- Argon2id hash
DEFINE FIELD user_id ON refresh_token TYPE record<User>;
DEFINE FIELD expires_at ON refresh_token TYPE datetime;
DEFINE FIELD revoked ON refresh_token TYPE bool DEFAULT false;
DEFINE FIELD created_at ON refresh_token TYPE datetime DEFAULT time::now();
DEFINE FIELD user_agent ON refresh_token TYPE string?;
DEFINE FIELD ip_address ON refresh_token TYPE string?;
DEFINE INDEX idx_token_hash ON refresh_token COLUMNS token_hash UNIQUE;
DEFINE INDEX idx_user_id ON refresh_token COLUMNS user_id;

-- Access token blacklist (Valkey) - no SurrealDB, usar Valkey con TTL
-- Key: "blacklist:access:{jti}" Value: "1" EXPIRE = remaining_ttl
```

### Migraciones
- `migrations/XXX_add_refresh_tokens.surql`
- `migrations/XXX_add_permissions_to_roles.surql`

## Edge Cases
| # | Caso | Comportamiento esperado |
|---|------|------------------------|
| 1 | Refresh token robado + uso legítimo posterior | Detección de reuso → revocar todo, alerta seguridad |
| 2 | Clock skew servidor/cliente | Validar `exp` con margen 30s, usar `nbf` |
| 3 | MFA requerido para roles admin | Login exige `mfa_code` si rol tiene `require_mfa=true` |
| 4 | Refresh token expira durante request | Access token válido hasta expiry, refresh falla |
| 5 | Valkey caído (blacklist) | Fail-open para access tokens (log warning), fail-closed para refresh |

## Security Considerations
- **Token storage**: Refresh tokens hasheados con Argon2id (igual que passwords)
- **Rotation**: Refresh token rotado en cada uso (single-use)
- **Reuse detection**: Si refresh token usado 2 veces → revocar todas las sesiones del usuario
- **Blacklist**: Access tokens en Valkey con TTL = tiempo restante
- **HTTPS only**: Cookies `Secure`, `SameSite=Strict`
- **MFA**: TOTP obligatorio para role `admin` (configurable por rol)

## Testing Strategy

### Unit Tests
- [ ] `test_login_issues_token_pair()` — access 15min, refresh 7d
- [ ] `test_refresh_rotates_token()` — old revoked, new issued
- [ ] `test_revoked_refresh_rejected()` — 401 on revoked
- [ ] `test_reuse_detection_revokes_all()` — concurrent refresh → revoke all
- [ ] `test_granular_rbac_enforced()` — matrix permissions

### Integration Tests
- [ ] Full flow: login → access resource → refresh → logout → verify revoked
- [ ] Concurrent refresh attack simulation
- [ ] MFA enforcement for admin

### Property-Based (proptest)
- [ ] Token expiry boundaries
- [ ] RBAC permission combinations

### Security Tests
- [ ] `cargo audit` → 0 advisories
- [ ] Token timing attacks (constant-time compare)

## Rollout Plan
- **Feature Flag**: `AUTH_HARDENING=true` (default OFF, ON en release)
- **Migración**: Deploy con flag OFF → activar → existing sessions migradas en siguiente login
- **Rollback**: Flag OFF → old tokens válidos hasta expiry natural

## Definition of Done
- [ ] Spec aprobada
- [ ] Endpoints `/auth/refresh`, `/auth/logout` implementados
- [ ] Refresh token rotation + reuse detection
- [ ] RBAC granular en 100% endpoints (middleware `require_permission`)
- [ ] Valkey blacklist para access tokens
- [ ] Tests unit + integration pasando
- [ ] `cargo audit` → 0 advisories
- [ ] CHANGELOG.md actualizado