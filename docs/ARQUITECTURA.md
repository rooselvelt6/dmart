# ADR — Modelo de datos dMart UCI

**Estado**: Aceptado  
**Fecha**: 2026-09-11  
**Autor**: Equipo dMart

## Contexto

dMart UCI es un sistema de gestión de pacientes de UCI que requiere persistencia transaccional, auditoría HIPAA, interoperabilidad FHIR R4 y autenticación robusta con MFA. La base de datos elegida es **SurrealDB** (modo embedded `kv-surrealkv`) por su flexibilidad multi-modelo (documentos + grafo + relacional), soporte nativo de SQL-like (SurrealQL), y capacidad de ejecutarse embebido sin dependencias externas.

## Decisiones

### 1. Identificadores de entidad
- **Pacientes, camas, equipos, diagnósticos, auditoría**: UUID v4 como `record id` (SurrealDB `thing`). Clave primaria estable, sin colisiones en merge.
- **Usuarios (staff)**: UUID v4 como `record id`. El registro público (self-registration) usa `username` como record id (`user:username`) por simplicidad de login; los creados por admin usan UUID.
- **MFA settings**: record id = `user_id` (UUID) → tabla `mfa_settings` 1:1 con users.
- **Migraciones de esquema**: record id = número de versión (`schema_migrations:1`, `schema_migrations:2`…).

### 2. Modelo de tablas principales

| Tabla | Record ID | Índices clave | Descripción |
|-------|-----------|---------------|-------------|
| `patients` | UUID | `idx_patients_created_at`, `idx_patients_estado_gravedad`, `idx_patients_historia_clinica` | Datos clínicos + scores calculados |
| `camas` | UUID | `idx_camas_estado`, `idx_camas_numero`, `idx_camas_tipo` | Camas UCI con estado y asignación |
| `equipos` | UUID | `idx_equipos_cama_id`, `idx_equipos_tipo`, `idx_equipos_estado` | Ventiladores, monitores, etc. |
| `diagnosticos` | UUID (CIE-10 code) | `idx_diagnosticos_codigo`, `idx_diagnosticos_descripcion` | Catálogo CIE-10 seed |
| `users` | UUID / username | `idx_users_user_id`, `idx_users_username`, `idx_users_rol` | Staff + registro público |
| `mfa_settings` | UUID (user_id) | `idx_mfa_settings_enabled` | TOTP secret, backup codes (sha256) |
| `audit_logs` | UUID | `idx_audit_timestamp`, `idx_audit_user_id`, `idx_audit_action`, `idx_audit_resource` | PHI access log (retención 6 años) |
| `schema_migrations` | versión (u64) | — | Control de migraciones embebidas |
| `institucion_config` | singleton (`institucion_config:singleton`) | — | Config global de la institución |

### 3. Relaciones (Grafo)
- `paciente` → `asignado_a` → `cama` (campo `cama_id` en patients + `paciente_id` en camas)
- `equipo` → `conectado_a` → `cama` (campo `cama_id` en equipos, `NULL` = disponible)
- `audit_log` → `usuario` (campo `user_id` opcional) + `recurso` + `recurso_id`

No se usan `RELATE` edges explícitos; las FK lógicas son campos en los documentos (más simple, consultas WHERE directas).

### 4. Transacciones atómicas
SurrealDB 2.6.5 **no expone `db.begin()`** en el cliente Rust (`Surreal<Db>`). Las transacciones reales se logran con **scripts SurrealQL** `BEGIN TRANSACTION; … COMMIT TRANSACTION;` en una sola llamada `db.query()`.

Reglas validadas empíricamente:
- `THROW` dentro de `BEGIN/COMMIT` revierte todo (probado).
- `RETURN` **aborta el script** → el `COMMIT` debe ir **ANTES** del `RETURN` final.
- `not is::empty($x)` falla parse → usar `string::len($x) > 0`.
- `type::len` no existe → usar `array::len`.
- `type::table('tbl')` dentro de IF en transacción rompe → usar `FROM tbl` sin cualificar.
- Mensaje de `THROW` se pierde (genérico) → prevalidar en Rust para errores claros.
- Binds exigen valores owned (`'static`): `.bind(("equipos", equipos_ids.to_vec()))`.

Operaciones transaccionales implementadas:
- `create_patient_with_assignments`: crea paciente + ocupa cama + asigna equipos (rollback si cama no libre).
- `egresar_paciente`: libera cama + desvincula equipos.

### 5. Consultas con WHERE (no full-scan)
Todas las consultas de lectura usan `WHERE` con binds tipados y índices:
- `get_cama_libre_por_tipo`, `get_cama_by_numero`, `list_equipos_por_cama`, `list_equipos_disponibles` (`cama_id = NONE`)
- Conteo con `GROUP BY`: `count_camas_por_tipo`, `count_equipos_por_tipo` (fusionados en Rust)
- `get_user_by_username`, `list_staff` (rol IN)
- Auditoría: builder dinámico de WHERE (`action IN $critical`, rangos de fecha)
- Búsqueda diagnósticos: `string::contains(string::lowercase(descripcion), $q)` (no `~=`)

### 6. Agregaciones server-side (Stats)
`math::sum` es función de array, no agregado por fila. Solución:
1. `SELECT estado_gravedad, count(), array::sum(ultimo_apache_score)...` → no soportado directo.
2. Query que trae solo columnas numéricas + `estado_gravedad` → sumar en Rust (`PatientAggregates`).
3. Promedios ponderados por n-no-nulo.

### 7. Migraciones embebidas + Idempotencia
- Cada migración vive en `migrations/00N_nombre.surql` y se compila con `include_str!`.
- Tabla `schema_migrations` registra versiones aplicadas.
- `run_migrations` salta las ya aplicadas → idempotente.
- 001: 11 índices baseline; 002: índices users/MFA.

### 8. Autenticación y MFA TOTP (RFC 6238)
- JWT HS256 con `scope`: `"session"` (completo) o `"mfa"` (reto de 5 min).
- Login: si MFA habilitado → emite token `scope="mfa"` + `mfa_required=true`.
- Endpoints MFA: `/setup` (genera secreto base32 + 10 backup codes sha256), `/confirm` (valida 1er TOTP → habilita), `/verify` (reto token + TOTP/backup → sesión completa), `/disable` (valida TOTP → borra settings).
- Middleware rechaza `scope="mfa"` en rutas protegidas (salvo `/auth/mfa/verify` abierto).
- `totp-rs` v5 con features `["otpauth", "gen_secret"]` (sin QR; `get_url()` da URI otpauth).

### 9. FHIR R4
- `Patient` bundle search/get (`/fhir/Patient`, `/{id}`).
- `Observation` bundle para escalas (`/fhir/Patient/{id}/Observation`): GCS (LOINC 9269-2), APACHE II, SOFA, etc. con codesystem local `http://dmart.local/fhir/CodeSystem/scores`.
- Sin wrapper `ApiResponse` (JSON FHIR puro).

### 10. Auditoría HIPAA + Retención 6 años
- `AuditLog` con `uid` (no `id` para evitar colisión con record id de SurrealDB).
- Acciones críticas: `LoginFailed`, `Delete`, `ConfigChange`, `AuthChange`, `AccessDenied`.
- Endpoints admin: `GET /admin/audit` (filtros), `GET /admin/audit/critical`, `POST /admin/audit/cleanup` (DELETE `timestamp < now-6y` + `RETURN AFTER` → cuenta borrados).
- Limpieza manual o job programado externo.

### 11. Configuración por entorno
Variables críticas (`.env` / entorno):
- `DMART_MASTER_KEY` (obligatoria, ≥32 chars) → deriva claves JWT/encriptación.
- `DMART_DB_PATH` (default `./data/dmart.db`).
- `DMART_CORS_ORIGIN` (lista orígenes permitidos).
- `DMART_APP_NAME` (issuer TOTP, default "dMart UCI").
- `JWT_EXPIRY_HOURS` (1–24, default 1).

## Consecuencias

**Positivas**:
- Una sola base de datos embebida, cero dependencias externas en producción.
- Esquema flexible: añadir campos no requiere migración (schemaless), pero migraciones versionadas para índices/constraints.
- SQL-like familiar para consultas complejas (JOIN implícito via SELECT, GROUP BY, transacciones).
- Auditoría completa y retención legal automática.

**Riesgos / Mitigaciones**:
- SurrealDB embedded no tiene réplica/HA → backups periódicos del archivo `dmart.db` (cron + `rsync`/`restic`).
- Transacciones en script SQL crudo → tests de integración cubren rollback y commit.
- MFA sin códigos QR → usuario copia secreto base32 manualmente; aceptable para entorno hospitalario controlado.
- FHIR limitado a Patient/Observation → extensible añadiendo handlers.

## Pruebas de validación
- 22 tests E2E API (`api_tests.rs`): CRUD, transacciones, stats GROUP BY, MFA setup/verify/disable/backup, FHIR Observation, auditoría retention cleanup.
- 14 tests lib (migraciones + índices).
- 66 tests shared (escalas clínicas, mortalidad, integración).
- `cargo fmt --all -- --check`, `cargo clippy --workspace -- -D warnings`, `cargo build --release` pasan en CI.

## Referencias
- SurrealDB 2.6.5 docs: Transactions, DEFINE INDEX, RETURN AFTER.
- RFC 6238 (TOTP), RFC 7519 (JWT).
- HL7 FHIR R4 spec: Patient, Observation, Bundle.
- HIPAA §164.312(b) Audit controls, §164.530(j) Retention 6 years.