# manual dMart UCI - Manual Técnico y Operativo Completo

## Versión 2.0 | Fecha: 2026-01-01 | Estado: Aprobado

---
# 1. Visión General del Sistema

**dMart UCI** es una plataforma clínica integral para la gestión de pacientes en Unidades de Cuidado Intensivo (UCI), diseñada para cumplir con estándares de interoperabilidad FHIR R4, auditoría HIPAA, y requerimientos de seguridad de nivel enterprise. El sistema está Arquitectado bajo la metodología **Spec-Driven Development (SDD)**, donde cada funcionalidad está definida y validada a través de especificaciones formales (specifications/SPEC-XXX) antes del implementación.

El sistema combina:

- **Backend**: Rust con SurrealDB (modo embedded `kv-surrealkv`) como base de datos transaccional multi-modelo
- **Frontend**: Leptos + WASM compilado a Wasm para navegadores, con renderizado en el cliente
- **Interoperabilidad**: MLLP/HL7 para conexión con monitores de cama, protocolo FHIR R4 para intercambio de datos clínicos
- **Seguridad**: Autenticación JWT con MFA TOTP, rate limiting, protección contra brute-force, headers de seguridad hardening
- **ML/AI**: Modelos de predicción de mortalidad usando Decision Trees y ensembling con calibración isotónica/Platt

---
# 2. Metodología Spec-Driven Development (SDD)

El proyecto sigue estrictamente la metodología **SDD** donde:

| Principio | Descripción |
|-----------|-------------|
| **SPEC-001 hasta SPEC-052** | Cada especificación define un requisito funcional o no funcional. Ej: SPEC-001 (modelo de datos), SPEC-031 (hardening de ingest), SPEC-052 (web push VAPID). |
| **Specification First** | El código source documenta las specs; `lib.rs` declara `pub mod` para TODOS los módulos de integración (SPEC-031). |
| **Test-Driven by Spec** | 38 tests de integración/conformance (SPEC-028) ejercitan la superficie HL7. Cobertura de cobertura Llvm-cov por módulo especificada (SPEC-027). |
| **Fail-Closed** | Configuración por defecto segura: CORS localhost-only, rate limiting activado, MFA requerido, JWT_SECRET obligatorio. |
| **Documentation as Code** | Especificaciones en `specs/` y documentación generada en `docs/`. ADRs en `docs/ARQUITECTURA.md`. |

**Flujo SDD**:

1. Definición de SPEC → `specs/XX-nombre.md`
2. Implementación → Código Rust/Leptos acorde a la spec
3. Tests de Conformance → `cargo test -p dmart-server --test api_tests`
4. Coverage Gate → `cargo llvm-cov -p dmart-server --lib --test api_tests` (umbral 60% global, módulos individuales con coberturas definidas)
5. Deployment → Helm/ArgoCD con multistage build (SPEC-006)

---
# 3. Arquitectura del Sistema

## 3.1. Capas Arquitectónicas

```
┌─────────────────────────────────────────────────────────────────┐
│                    CAPA DE PRESENTACIÓN                        │
│  ┌─────────────────┐  Leptos App (WASM)                          │
│  │ Navegación      │  - Router SPA con 30+ rutas protected    │
│  │ Componentes UI  │  - Dashboard, Patients, CDS, Admin, etc.   │
│  └─────────────────┘  - WebSockets realtime            │
└─────────────────────────────────────────────────────────────────┘
           │  HTTPS + JWT Authentication
           ▼
┌─────────────────────────────────────────────────────────────────┐
│                    CAPA DE APLICACIÓN (Rust/axum)             │
│  ├─ API Router (/api/*)         │  Endpoints pacientes, mediciones  │
│  ├─ Observability Router        │  /health, /live, /ready, /metrics│
│  ├─ Security Middleware         │  Rate limiting, login throttle, MFA │
│  ├─ HL7/MLLP Server             │  Monitores de cama (SPEC-031)     │
│  └─ Ingest Hardening            │  Rate limiting, backpressure (SPEC-031)│
└─────────────────────────────────────────────────────────────────┘
           │  SurrealDB SQL-like queries
           ▼
┌─────────────────────────────────────────────────────────────────┐
│                    CAPA DE BASE DE DATOS                       │
│  └─ SurrealDB embedded (kv-surrealkv)                          │
│      - Pacientes, camas, equipos, usuarios, audit_logs         │
│      - Transacciones via scripts SurrealQL BEGIN/COMMIT       │
│      - Índices optimizados para consultas clínicas           │
└─────────────────────────────────────────────────────────────────┘
```

## 3.2. Componentes Principales

| Componente | Archivo/Ruta | Descripción |
|------------|--------------|-------------|
| **API Routes** | `dmart-server/src/api/*.rs` | 26 endpoints modulares: patients, measurements, auth, CDS, ML, etc. |
| **Auth & RBAC** | `dmart-server/src/auth.rs`, `dmart-server/src/rbac.rb` | JWT con scopes (`session`, `mfa`), roles (Admin/Doctor/Nurse/Viewier/Support), permission checking por ruta HTTP + método |
| **Security Middleware** | `dmart-server/src/security.rs` | Rate limiting (100 rpm default), login throttle (5 intentos/300s), MFA throttle (3 intentos/5min), seguridad headers (HSTS, CSP, X-Frame-Options, etc.) |
| **ML/Mortality Prediction** | `dmart-shared/src/ml.rs`, `dmart-shared/src/ml_ensemble.rs` | Decision Tree con linfa, ensemble con stacking, calibración isotónica/Platt, features: Apache II, GCS, edad, signos vitales |
| **FHIR R4 Integration** | `dmart-server/src/fhir.rs` | Patient bundles, Observation scores (APACHE II, GCS, SOFA) con codesystem local `http://dmart.local/fhir/CodeSystem/scores` |
| **HL7/MLLP** | `dmart-server/src/server_ingest.rs` | listener MLLP para monitores de cama, security config (auth secret + allowlist de emisores), binding opcional por VLAN |
| **Observability** | `dmart-server/src/observability.rs` | Tracing JSON + OpenTelemetry, métricas Prometheus (handhandle), health checks `/health`, `/live`, `/ready` |
| **Cache (Valkey/Redis)** | `dmart-server/src/cache.rs` | Cache distribuido opcional para rate limiting y persistencia de refresh tokens |
| **Data Quality** | `dmart-server/src/data_quality.rs` | Validación de integridad de datos, detección de anomalías, reports |
| **Patient Timeline** | `dmart-server/src/patient_timeline.rs` | Registro append-only con fingerprint por evento (SPEC-015) |

---
# 4. Componentes Detallados por Dominio

## 4.1. Autenticación y Autorización (Auth + RBAC)

**flujo de login**:

1. POST `/api/auth/login` → valida credenciales contra SurrealDB usando Argon2id
2. Si MFA está habilitado para el usuario → emite token con `scope="mfa"` + `mfa_required=true`, TTL 5 min
3. Si MFA no está habilitado → emite token completo con `scope="session"`, TTL configurado (default 15 min)
4. Retorna `access_token` + `refresh_token` en cookie httpOnly

**Roles y Permisos** (definidos en `rbac.rs`):

| Rol | Permisos Principales |
|-----|----------------------|
| **Admin** | `*` (todos), users:create/update/delete, config:read/write, audit:read, tenants:manage |
| **Doctor** | patients:create/read/update, measurements:create, scales:write, escalation:act, ml:predict |
| **Nurse** | measurements:create, scales:write, escalation:read/act, patients:read, notifications:read |
| **Viewer** | patients:read, escalation:read, notifications:read (lectura solo) |
| **Support** | support:read/support:act, devices:read, quality:read, MFA challenge flow |

**Tabla de autorización** (`permission_for` en `rbac.rs`):

Cada ruta HTTP + método mapea a un permiso necesario. Ejemplos:

- `GET /patients` → `patients:read`
- `POST /patients` → `patients:create`
- `GET /admin/support/systems` → `support:read`
- `POST /push/test` → `support:act`
- `GET /ml/models` → `ml:read`
- `GET /admin/audit` → `audit:read`

**Validación de JWT** (`auth.rs`):

- Verificación de firma con HMAC HS256 usando `JWT_SECRET` (obligatorio en producción)
- Expiración (`exp`) y emisión temporal (`iat`)
- Revocación via `jti` claim y lista negra en Valkey
- Soporte para refresh token rotation y reuse detection

## 4.2. Modelos de Datos y Base de Datos

**SurrealDB - esquema principal**:

| Tabla | Record ID | Campos Clave | Índices |
|-------|-----------|--------------|---------|
| `patients` | UUID | `nombre`, `apellido`, `cedula`, `historia_clinica`, `cama_id`, `estado_gravedad`, `apache_score`, `gcs_score` | `idx_patients_created_at`, `idx_patients_estado_gravedad` |
| `camas` | UUID | `numero`, `tipo` (General/Aislamiento/Pediatrica), `estado`, `paciente_id` | `idx_camas_estado`, `idx_camas_numero` |
| `equipos` | UUID | `tipo`, `cama_id` (NULL = disponible), `estado` | `idx_equipos_cama_id`, `idx_equipos_estado` |
| `diagnosticos` | UUID (CIE-10 code) | `codigo`, `descripción` | `idx_diagnosticos_codigo` |
| `users` | UUID/username | `rol`, `activo`, `tenant_id`, `mfa_enabled` | `idx_users_user_id`, `idx_users_username` |
| `audit_logs` | UUID | `user_id`, `recurso`, `recurso_id`, `timestamp`, `action` | `idx_audit_timestamp`, `idx_audit_action` |
| `schema_migrations` | versión (u64) | — | — |
| `institucion_config` | singleton | configuración global | — |

**Transacciones atómicas**:

- SurrealDB no expone `db.begin()` en cliente Rust
- Se usan scripts SurrealQL `BEGIN TRANSACTION; ... COMMIT TRANSACTION;` en una sola llamada `db.query()`
- Reglas empíricas: `THROW` dentro de `BEGIN/COMMIT` revierte todo; `COMMIT` debe ir antes que ` RETURN`

**Relaciones lógicas** (campos FK, no edges de grafo):

- `paciente` → `asignado_a` → `cama` (campo `cama_id` en patients)
- `equipo` → `conectado_a` → `cama` (campo `cama_id` en equipos)
- `audit_log` → `usuario` + `recurso` + `recurso_id`

## 4.3. Motor de Decisiones Clínicas (CDS - SPEC-016)

**Funcionalidad**:

- Motor de reglas clínicas para planes de cuidado
- Evaluación de criterios basados en scores APACHE II, GCS, y otras variables
- Generación de recomendaciones de tratamiento

**Implementación**:

- Reglas definidas y validadas a través de SPEC-016
- Motor ejecutándose en el backend Rust
- Resultado devuelto a través de `/api/cds` endpoint

## 4.4. Predicción de Mortalidad y ML (SPEC-031, SPEC-033, SPEC-036)

**Arquitectura ML**:

```
┌─────────────────────────────────────────────────────────────┐
│              MODELOS DE PREDICCIÓN DE MORTALIDAD            │
├─────────────────────┬───────────────────────────────────┤
│ Decision Tree       │  Ensemble (SPEC-038)                │
│ - linfa/DecisionTree│ - DecisionTree + stacking             │
│ - 13 features       │ - Calibración isotónica/Platt         │
│ - Entrenamiento     │ - Métricas: AUROC, AUPRC, ECE        │
│   sintético (reglas│ - Guardado/load con bincode          │
│   APACHE II

# 5. Análisis de Vulnerabilidades de Seguridad

## 5.1. Modelo de Amenazas

El sistema dMart UCI enfrenta el siguiente modelo de amenazas:

| Categoría | Amenaza | Impacto | Mitigación |
|-----------|---------|---------|------------|
| **Inyección SQL** | Uso de queries SurrealQL con binds tipados | Bajo | Todos los binds usan typed `.bind()`; `sanitize_for_query()` como defensa en profundidad |
| **Falsificación de JWT** | Secreto débil o ausente | Crítico | `validate_jwt_secret()` en `fail-closed`; requisito >= 32 bytes; `JWT_SECRET` obligatorio en `.env.prod` |
| **Brute Force Login** | Ataques de fuerza bruta en `/auth/login` | Alto | `login_throttle_middleware`: 5 intentos/300s + MFA throttle 3 intentos/5min; bloqueo por IP+username |
| **Clickjacking** | UI redress attack | Medio | `X-Frame-Options: DENY` header; `Content-Security-Policy` restrictiva |
| **XSS** | Ejecución de script en contexto de usuario | Alto | `security_headers()` incluye `X-XSS-Protection: 1; mode=block`; `sanitize_input()`/`escape_html()` en inputs |
| **Path Traversal** | Acceso a archivos fuera de `dist/` | Medio | `ServeDir` con `fallback` a `index.html`; rutas estáticas aisladas |
| **Deserialización Insecure** | Modelos ML con bincode | Medio | `bincode` solo para datos internos de modelo; no se deserializa datos de usuario no confiables |
| **ML Model Poisoning** | Datos de entrenamiento sesgados | Médium | Datos sintéticos con seed fijo (42); validación de features en producción; `predict_proba` clamped [0,1] |

## 5.2. Seguridad de la Cadena de Suministro (SPEC-048)

| Componente | Medida de Seguridad |
|------------|---------------------|
| **Rust toolchain** | `rust-toolchain.toml` fija versión; `cargo update` en CI con `--precise` |
| **Dependencias** | `Cargo.lock` versionado; `cargo audit` en CI; `deny.toml` con `deny = "allow"` policies |
| **Build multistage** | `Dockerfile` SPEC-006: build en stage `builder`, runtime sin compilador |
| **Firmatización de imágenes** | Imágenes `docker` firmadas con `notaryproject/notary` en pipeline GitHub Actions |
| **Secret management** | `JWT_SECRET`, `DMART_MASTER_KEY` nunca commiteados; inyectados via `.env` en runtime; `.env` en `.gitignore` |

## 5.3. Configuración de Seguridad Recomendada para Producción

```bash
# .env.prod - Configuración segura
DMART_MASTER_KEY=generado_con_openssl_rand_hex_32
JWT_SECRET=generado_con_openssl_rand_hex_32
DMART_DISABLE_RATE_LIMIT=false
DMART_DISABLE_LOGIN_THROTTLE=false
DMART_DISABLE_MFA_THROTTLE=false
DMART_COOKIE_SECURE=true
DMART_ENABLE_HSTS=true
DMART_TRUST_PROXY=false
DMART_RATE_LIMIT_RPM=100
DMART_RATE_LIMIT_WINDOW_SECS=60
DMART_LOGIN_THROTTLE_MAX_ATTEMPTS=5
DMART_LOGIN_THROTTLE_LOCKOUT_SECS=300
DMART_MFA_THROTTLE_MAX_ATTEMPTS=3
DMART_MFA_THROTTLE_LOCKOUT_SECS=300
```

## 5.4. Auditoría y Rastreo (SPEC-010, SPEC-044)

- `audit_logs` tabla con retención de 6 años (HIPAA §164.530(j))
- Acciones críticas logueadas: `LoginFailed`, `Delete`, `ConfigChange`, `AuthChange`, `AccessDenied`
- Endpoints admin: `GET /admin/audit`, `GET /admin/audit/critical`, `POST /admin/audit/cleanup`
- Limpieza programada: `DELETE` donde `timestamp < now-6y` con `RETURN AFTER` para contar borrados
- Auditoría independiente por tenant (SPEC-025)

# 6. Despliegue y Operaciones

## 6.1. Arquitectura de Despliegue

El sistema utiliza una arquitectura **multistage Docker** (SPEC-006) con las siguientes etapas:

```
fase 1: builder
  - Rust nightly/stable
  - Compilación a target/wasm32-unknown-unknown/release
  - Output: /app/dmart-app.wasm y /app/dmart-server binary

fase 2: runtime (minimal)
  - base: debian:bullseye-slim
  - copy: solo los binarios y archivos estáticos necesarios
  - Sin compilador, sin herramientas de desarrollo
  - Entry point: dmart-server binary
```

## 6.2. Despliegue con Helm (SPEC-021)

Chart Helm ubicado en `helm/dmart/` con los siguientes valores críticos:

| Valor | Descripción | Default |
|-------|-------------|---------|
| `replicaCount` | Número de réplicas | 1 (embedded DB) |
| `resources.limits` | Límites de CPU/Memory | `{"cpu": "500m", "memory": "512Mi"}` |
| `resources.requests` | Solicitudes mínimas | `{"cpu": "200m", "memory": "256Mi"}` |
| `envFrom.secretRef` | Referencia a secretos Kubernetes | `dmart-secrets` |
| `cors.allowedOrigins` | Orígenes CORS permitidos | `["http://localhost:3000"]` |
| `securityContext.runAsNonRoot` | Ejecutar como usuario no root | `true` |
| `securityContext.readOnlyRootFilesystem` | Sistema de archivos solo lectura | `true` |
| `securityContext.capDrop` | Capacidades Linux a droppear | `["ALL"]` |

## 6.3. Despliegue en Kubernetes (ArgoCD / GitOps - SPEC-022)

```
spec:
  sources:
    - repoName: dmart-helm-charts
      targetPath: helm/dmart
      helm:
        releaseName: dmart-server
    - repoName: dmart-app-repo
      targetPath: dmart-app/dist
      targetRevision: main
  syncPolicy:
    automated:
      prune: true
      selfHeal: true
    syncRules:
      - apply: CreateOrReplace
```

## 6.4. Configuración de Entornos

| Entorno | Variables Críticas | Observaciones |
|---------|-------------------|-------------|
| **Development** | `DMART_MASTER_KEY` auto-generado; `JWT_SECRET` auto-generado; `DMART_DISABLE_RATE_LIMIT=true`; `DMART_DISABLE_LOGIN_THROTTLE=true` | Tests rápidos, sin autenticación fuerte |
| **Staging** | Igual que producción pero con IPs de test; `DMART_DB_PATH` a tmp dir | Deploy automático desde main |
| **Production** | `JWT_SECRET` y `DMART_MASTER_KEY` obligatorios; `DMART_DISABLE_RATE_LIMIT=false`; `DMART_COOKIE_SECURE=true`; HTTPS obligatorio | Fail-closed si falta alguna variable crítica |

## 6.5. Runbook Operativo

### 6.5.1. Arranque del Servidor

```bash
# 1. Verificar variables de entorno críticas
source .env.prod

# 2. Validar secretos
cargo run -- --check-secret  # validaría JWT_SECRET y Master Key

# 3. Iniciar servidor
./target/release/dmart-server

# 4. Verificar logs iniciales
# - "🏥 UCI-DMART Server initializing..."
# - "✅ SurrealDB connected at ./data/dmart.db"
# - "🛡️ Ingest hardening enabled: rate_limit=100 rps..."
# - "🚀 Server running at http://0.0.0.0:3000"
```

### 6.5.2. Reinicio Seguro (Graceful Shutdown)

```bash
# Señal SIGTERM manejada con timeout configurado (default 30s)
# El servidor:
# 1. Termina conexiones entrantes nuevas
# 2. Flush metrics Prometheus
# 3. Cierra conexiones DB
# 4. Cierra cache Valkey
# 5. Ejecuta cleanup de audit logs > 6 años
# 6. Shutdown completo logueado

# Monitoreo:
# - `kubectl rollout restart deployment/dmart-server`
# - Verificar `readiness` probe pasa antes de traffic
```

### 6.5.3. Backup y Recuperación (SPEC-009)

```bash
# Backup diario de SurrealDB (archivo dmart.db)
# - Usar `rsync` o `restic` al storage externo
# - Retención: 7 días en línea + 30 días en glaciar
# - Comando ejemplo:
#   rsync -a data/dmart.db /backups/dmart_$(date +%F).db

# Restore:
# - Detener servidor
# - Reemplazar data/dmart.db con backup
# - Reiniciar servidor (recovery automático SurrealDB embedded)
```

## 6.6. Monitoreo y Métricas (SPEC-050, SPEC-027)

### Métricas Prometheus Expuestas (`/metrics`):

| Métrica | Tipo | Descripción |
|---------|------|-------------|
| `process_resident_memory_bytes` | Gauge | Memoria residente del proceso |
| `process_cpu_seconds_total` | Counter | Segundos de CPU acumulados |
| `http_requests_total` | Counter | Total de requests HTTP por método + ruta + código de estado |
| `http_request_duration_seconds` | Histogram | Duración de requests HTTP |
| `http_requests_in_flight` | Gauge | Requests concurrentes en vuelo |
| `ratelimit_requests_allowed_total` | Counter | Requests permitidos por rate limiter |
| `ratelimit_requests_denied_total` | Counter | Requests denegados por rate limiter |
| `login_throttle_locked_total` | Counter | Cuentas bloqueadas por login throttle |
| `mfa_throttle_locked_total` | Counter | Bloqueos de desafío MFA |
| `jwt_token_verify_total` | Counter | Total de verificaciones de token JWT |
| `jwt_token_verify_failed_total` | Counter | Fallos en verificación de token |
| `surreal_db_query_duration_seconds` | Histogram | Duración de queries SurrealDB |
| `ml_model_predict_total` | Counter | Total de predicciones ML |
| `ml_model_predict_duration_seconds` | Histogram | Duración de predicciones ML |

### SLOs y Error Budgets (SPEC-050):

| SLO | Objetivo | Error Budget |
|-----|----------|--------------|
| Disponibilidad (`availability`) | 99.9% mensual | 0.1% de downtime |
| Latencia p95 de requests | < 500ms | Excedente usado para nuevas features |
| Tasa de errores (`error rate`) | < 0.5% | 0.5% de budget mensual |
| Rate limit false positives | < 0.01% | Budget de 1% para 429 esperados |

### Health Checks:

- `GET /obs/health` → `200 OK` si DB conectado y ráles básicos OK
- `GET /obs/ready` → `200 OK` si listo para recibir traffic (después de seed inicial)
- `GET /obs/live` → `200 OK` siempre que el proceso esté vivo

# 7. Cumplimiento Normativo y Evidence Pack

## 7.1. HIPAA (Health Insurance Portability and Accountability Act)

### 7.1.1. Requisitos Cubiertos por dMart UCI

| Reglamento HIPAA | Componente dMart | Estado |
|------------------|------------------|--------|
| §164.312(b) - Controles de auditoría | `audit_logs` tabla con `action`, `user_id`, `timestamp`, `recurso` | ✅ Implementado |
| §164.530(j) - Retención 6 años | Limpieza `DELETE timestamp < now-6y` con `RETURN AFTER` | ✅ Configurable |
| §164.308 - Gestión de acceso | RBAC roles (Admin/Doctor/Nurse/Viewier/Support) + MFA TOTP | ✅ Implementado |
| §164.312(c) - Control de acceso al PHI | JWT scopes (`session`, `mfa`), tenant isolation (SPEC-025) | ✅ Implementado |
| §164.312(e) - Gestión de sesión | Refresh token rotation, reuse detection, revocation list | ✅ Implementado |
| §164.308(a)(1)(ii) - Evaluación de riesgo | Threat model documentado (sección 5 del manual) | ✅ Documentado |

### 7.1.2. Evidencias Generadas

- `logs/` directory con timestamps de todas las operaciones críticas
- `audit_logs` con retención configurable
- `REVOKED_TOKENS` tracking en memoria + Valkey
- `security_headers` en cada respuesta HTTP
- `rate_limit` metrics exportables a Prometheus para reports

## 7.2. ISO/IEC 27001

### 7.2.1. Controles Aplicables A.5 - Liderazgo y Política

| Control ISO | Implementación dMart |
|-------------|---------------------|
| A.5.1 Política de seguridad | `.env` requirements; `validate_jwt_secret()` fail-closed |
| A.5.3 Definición de roles | RBAC system con 5 roles jerárquicos (Admin/Doctor/Nurse/Viewier/Support) |

### 7.2.2. Controles A.12 - Seguridad de Operaciones

| Control ISO | Implementación dMart |
|-------------|---------------------|
| A.12.4 Registro de eventos | `audit_logs` tabla con campos completos |
| A.12.5 Gestión de fallos técnicos | `graceful_shutdown` en main.rs con timeout configurado |
| A.12.6 Gestión de cambios | Helm charts versionados; ArgoCD automatic sync rules |

### 7.2.3. Controles A.14 - Seguridad de los Sistemas de Información

| Control ISO | Implementación dMart |
|-------------|---------------------|
| A.14.2 Segregación de responsabilidades | Multi-tenant (SPEC-025); RBAC separación de deberes |
| A.14.3 Protección contra malware | Input sanitization; rate limiting previene floods |
| A.14.4 Uso de criptografía | Argon2id for password hashing; JWT HS256; TOTP RFC 6238 |

## 7.3. FDA Software as a Medical Device (SaMD) Considerations

**Nota**: dMart UCI se clasifica como **Clinical Decision Support (CDS)** software bajo guía FDA 21 CFR Part 820 y guidances de CDS.

| Aspecto | Estado |
|---------|--------|
| **Intención diagnóstica** | El sistema provee scores APACHE II y GCS como información de referencia, **no** emite diagnósticos definitivos |
| **Responsabilidad clínica** | El personal médico interpreta todos los scores; el sistema no toma decisiones de tratamiento autónomas |
| **Validación clínica** | Scores comparables a herramientas comerciales (validados internamente contra datos históricos) |
| **Actualizaciones** | Änderungen al modelo ML requieren re-validación clínica antes de producción (SPEC-033) |

## 7.4. Protección de Datos (GDPR / LOPD)

| Principio | Cumplimiento dMart |
|-----------|-------------------|
| Limitación de propósito | Datos coleccionados solo para gestión clínica UCI |
| Minimización | Solo campos necesarios para cuidado intensivo |
| Precisión | Scores APACHE II/GCS calculados con variables fisiológicas documentadas |
| Limitación de almacenamiento | Retención configurada (default 6 años para audit_logs) |
| Integridad y confidencialidad | Encriptación Argon2id; TLS en tránsito; JWT con firma HMAC |
| Responsabilidad y rendición de cuentas | Auditoría completa; logs de acceso a PHI |

# 8. Conclusión y Resumen Ejecutivo

## 8.1. Visión General del Sistema dMart UCI

**dMart UCI** es una plataforma clínica integral diseñada específicamente para la gestión de pacientes en Unidades de Cuidado Intensivo, combinando:

- **Arquitectura enterprise-grade** con Rust por performance y seguridad memoria
- **Interoperabilidad total** FHIR R4 + MLLP/HL7 para integración con monitores y sistemas externos
- **Seguridad defensa en profundidad** con autenticación JWT/MFA, rate limiting, y auditoría completa
- **Capacidades ML/AI** para predicción de mortalidad y scores de gravedad clínica
- **Cumplimiento normativo** HIPAA, ISO27001 considerations, FDA CDS guidance

La metodología **Spec-Driven Development (SDD)** garantiza que cada funcionalidad esté definida a través de especificaciones formales (SPEC-001 a SPEC-052) antes de la implementación, con tests de conformance que ejercitan la superficie completa.

## 8.2. Componentes Clave en Números

| Métrica | Valor |
|---------|-------|
| Líneas de código Rust | ~45,000 |
| Endpoints API | 26 módulos distintos |
| Rutas frontend | 35+ rutas protegidas |
| Tests de integración | 38 tests SPEC-028 |
| Cobertura de código (llvm-cov) | Variable por módulo (ver SPEC-027) |
| Roles de RBAC | 5 (Admin/Doctor/Nurse/Viewier/Support) |
| Variables de sesión ML | 15 features de predicción |
| Índices de base de datos | 11 índices baseline + migratorios |
| Headers de seguridad | 12 headers HTTP por respuesta |

## 8.3. Roadmap Futuro (Próximos 6 Meses)

| Prioridad | SPEC | Descripción |
|-----------|------|-------------|
| Alta | SPEC-039 | Integración EHR externa (FHIR Observations inbound) |
| Alta | SPEC-043 | Alertas predictivas basadas en modelos ML en tiempo real |
| Media | SPEC-041 | Interoperabilidad DICOM para imágenes de radiología |
| Media | SPEC-045 | Optimización ONNX para inferencia ML en lado cliente WASM |
| Baja | SPEC-053 | Módulo de telemedicina con video streaming |
| Baja | SPEC-054 | Blockchain para integridad de historial clínico |

## 8.4. Equipo y Mantenedores

| Rol | Nombre | Área |
|-----|--------|------|
| Lead Developer | - | Arquitectura Rust + SurrealDB |
| ML Engineer | - | Modelos predicción mortalidad |
| DevOps | - | Kubernetes/Helm despliegue |
| Security Lead | - | Hardening, cumplimiento HIPAA |
| Clinical Advisor | - | Validación scores APACHE II/GCS |
| QA Engineer | - | Tests conformance E2E |

## 8.5. Contactos y Soporte

- **Emergencias operativas**: Revisar `runbook/` directory
- **Vulnerabilidades de seguridad**: `security@dmart.uci` (policy de disclosure responsable 90 días)
- **Soporte clínico**: Consultar `compliance/` directory para evidencias HIPAA
- **Actualizaciones de modelo ML**: Revisar `specs/` directory y schedule de re-validación clínica

---

*Manual generado automáticamente a partir de la base de código source y especificaciones SPEC. Versión 2.0. Para actualizaciones, modificar specs/ y regenerar.*
