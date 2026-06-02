# Arquitectura de dMart - Sistema de Gestión UCI

## Visión General

```
┌─────────────────────────────────────────────────────────────────┐
│                        dMart UCI System                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────┐         ┌──────────────────────────────────┐ │
│  │   Frontend   │         │           Backend Server          │ │
│  │   (WASM)     │◄──────►│           (Rust/Axum)             │ │
│  │              │  HTTP   │                                   │ │
│  │  - Leptos    │         │  ┌─────────┐  ┌──────────────┐    │ │
│  │  - Tailwind  │         │  │  API   │  │   Metrics    │    │ │
│  │  - Chart.js  │         │  │Routes  │  │  /api/health │    │ │
│  └──────────────┘         │  └───┬───┘  └──────────────┘    │ │
│                           │      │                            │ │
│                           │  ┌───┴────────────────────────┐   │ │
│                           │  │     Business Logic        │   │ │
│                           │  │  - Scales Calculation    │   │ │
│                           │  │  - Validation            │   │ │
│                           │  │  - Export (CSV/PDF)      │   │ │
│                           │  └───────────────────────────┘   │ │
│                           └──────────────────────────────────┘ │
│                                      │                          │
│                           ┌──────────┴──────────┐              │
│                           │                     │              │
│                    ┌──────▼──────┐    ┌────────▼───────┐       │
│                    │  SurrealDB  │    │ Valkey/Redis  │       │
│                    │ (SurrealKV) │    │   (Cache)     │       │
│                    └─────────────┘    └───────────────┘       │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## Stack Tecnológico

| Componente | Tecnología | Función |
|------------|------------|---------|
| Frontend | Leptos (WASM) | Interfaz de usuario reactiva |
| Backend | Axum | Servidor HTTP y API REST |
| Base de Datos | SurrealDB (SurrealKV) | Almacenamiento embebido |
| Cache | Valkey/Redis | Cache de sesiones y mediciones |
| Styling | TailwindCSS | Diseño responsive |
| Lenguaje | Rust | Sistema completo |

---

## Estructura del Proyecto

```
dmart/
├── Cargo.toml              # Workspace configuration
├── dmart-shared/           # Código compartido
│   ├── src/
│   │   ├── models.rs       # Estructuras de datos
│   │   ├── scales.rs       # APACHE II, GCS, NEWS2, SAPS III, SOFA
│   │   ├── validation.rs   # Validación de rangos clínicos
│   │   └── lib.rs         # Exports públicos
│   └── tests/
│       └── scale_tests.rs  # Tests de validación (66+ tests)
│
├── dmart-server/           # Servidor backend
│   └── src/
│       ├── main.rs         # Punto de entrada, router, CORS, middleware
│       ├── auth.rs         # JWT, AuthService, Claims extractor
│       ├── db.rs           # Conexión SurrealDB (SurrealKV), queries paginadas
│       ├── cache.rs        # Cache Valkey/Redis global
│       ├── security.rs     # Rate limiter, login throttle, security headers
│       ├── audit.rs        # Auditoría HIPAA (login/logout/PHI access)
│       ├── middleware/
│       │   ├── mod.rs
│       │   └── auth_mod.rs # Auth middleware para Axum
│       ├── crypto.rs       # Cifrado ChaCha20Poly1305 para datos sensibles
│       ├── rbac.rs         # Control de acceso basado en roles
│       └── api/            # Endpoints REST
│           ├── mod.rs
│           ├── patients.rs
│           ├── measurements.rs
│           ├── scales.rs   # Endpoints de cálculo de escalas
│           ├── admin.rs    # CRUD camas, equipos, staff
│           ├── auth.rs     # Login, register, refresh token
│           ├── stats.rs    # Estadísticas del dashboard
│           ├── export.rs   # Export CSV/PDF
│           ├── sandbox.rs  # Generación de datos sintéticos
│           ├── fhir.rs     # API FHIR R4 compatible
│           ├── institucion.rs # Configuración de institución
│           └── diagnosticos.rs # Búsqueda CIE-10
│
├── dmart-app/             # Frontend WASM (Leptos)
│   ├── src/
│   │   ├── main.rs        # Entry point
│   │   ├── app.rs         # Router y navegación
│   │   ├── api.rs         # Cliente HTTP
│   │   ├── pages/         # Páginas
│   │   │   ├── login.rs
│   │   │   ├── dashboard.rs
│   │   │   ├── patients.rs
│   │   │   ├── admin.rs   # Admin: camas, equipos, staff, institución
│   │   │   ├── patient_detail.rs
│   │   │   ├── patient_edit.rs
│   │   │   └── measurement.rs
│   │   └── components/    # Componentes UI
│   ├── index.html
│   └── Trunk.toml
│
├── dist/                  # Frontend compilado
├── data/                  # Base de datos (SurrealKV)
├── docs/                  # Documentación
├── docker-compose.yml     # Desarrollo
├── docker-compose.prod.yml # Producción
└── .github/workflows/    # CI/CD
```

---

## Flujo de Datos

### 1. Registro de Paciente

```
Frontend (form) 
    → POST /api/patients 
    → Validación 
    → SurrealDB 
    → Respuesta JSON 
    → UI actualizada
```

### 2. Nueva Medición (APACHE II)

```
Frontend (forma clínica)
    → POST /api/patients/:id/measurements
    → API → dmart-shared::scales::calculate_apache_ii_score()
    → Cálculo de 12 variables + edad + crónicas
    → Cálculo de riesgo de mortalidad
    → Guardar en SurrealDB
    → Cache en Valkey
    → Respuesta → UI con score calculado
```

### 3. Visualización de Evolución

```
Dashboard
    → GET /api/patients/:id/measurements
    → Cache Valkey (si existe)
    → o SurrealDB
    → Gráfico de evolución (Chart.js)
```

---

## Módulos Principales

### dmart-shared::scales

```rust
// Cálculo de APACHE II
pub fn calculate_apache_ii_score(data: &ApacheIIData) -> u32

// Cálculo de GCS
pub fn calculate_gcs_score(gcs: &GcsData) -> u32

// Riesgo de mortalidad
pub fn mortality_risk(apache_score: u32) -> f64

// Desglose de puntos
pub fn apache_ii_breakdown(data: &ApacheIIData) -> ApacheIIBreakdown
```

### dmart-shared::validation

```rust
// Validar rangos clínicos
pub fn validate_apache_measurement(data: &ApacheIIData) -> ValidationResult
pub fn validate_gcs_measurement(gcs: &GcsData) -> ValidationResult
```

---

## Configuración

### Variables de Entorno

| Variable | Default | Descripción |
|----------|---------|-------------|
| `DMART_PORT` | 3000 | Puerto del servidor |
| `DMART_DB_PATH` | ./data/dmart.db | Ruta BD |
| `DMART_DIST_PATH` | ./dist | Ruta frontend |
| `DMART_VALKEY_URL` | redis://127.0.0.1:6379 | URL Valkey/Redis |
| `DMART_CORS_ORIGIN` | http://localhost:3000 | Orígenes CORS permitidos (coma-separados) |
| `DMART_ADMIN_PASSWORD` | admin123 | Password admin inicial (solo primer inicio) |
| `JWT_SECRET` | (auto-generado) | Secreto JWT de 32 bytes |
| `JWT_EXPIRY_HOURS` | 1 | Horas de validez del token JWT (1-24) |
| `RUST_LOG` | dmart_server=info | Nivel de logging |

---

## API Endpoints

| Método | Endpoint | Descripción |
|--------|----------|-------------|
| GET | /api/health | Health check (versión, uptime, DB, caché) |
| GET | /api/stats | Estadísticas del dashboard |
| | **Auth** | |
| POST | /api/auth/login | Iniciar sesión |
| POST | /api/auth/register | Registrar usuario |
| POST | /api/auth/refresh | Refrescar token JWT |
| | **Patients** | |
| GET | /api/patients?q=&limit=&offset= | Listar pacientes (paginado, búsqueda) |
| POST | /api/patients | Crear paciente |
| GET | /api/patients/:id | Obtener paciente |
| PUT | /api/patients/:id | Actualizar paciente |
| DELETE | /api/patients/:id | Eliminar paciente |
| POST | /api/patients/:id/egreso | Egresar paciente (libera cama) |
| | **Measurements** | |
| GET | /api/patients/:id/measurements | Listar mediciones |
| POST | /api/patients/:id/measurements | Crear medición |
| GET | /api/patients/:id/measurements/last | Última medición |
| | **Escalas Clínicas** | |
| POST | /api/patients/:id/scales/apache | Calcular APACHE II |
| POST | /api/patients/:id/scales/gcs | Calcular GCS |
| POST | /api/patients/:id/scales/news2 | Calcular NEWS2 |
| POST | /api/patients/:id/scales/sofa | Calcular SOFA |
| POST | /api/patients/:id/scales/saps3 | Calcular SAPS III |
| GET | /api/patients/:id/scales/history | Historial de escalas |
| | **Admin** | |
| GET | /api/admin/stats | Estadísticas admin |
| POST | /api/admin/camas/init | Inicializar camas |
| CRUD | /api/admin/camas[/:id] | CRUD camas |
| GET | /api/admin/camas/disponibles | Camas disponibles |
| CRUD | /api/admin/equipos[/:id] | CRUD equipos |
| POST | /api/admin/equipos/asignar | Asignar equipo a cama |
| POST | /api/admin/equipos/:id/desvincular | Desvincular equipo |
| CRUD | /api/admin/staff[/:id] | CRUD staff |
| POST | /api/admin/staff/:id/toggle | Activar/desactivar usuario |
| GET/PUT | /api/admin/institucion | Configurar institución |
| | **Diagnósticos CIE-10** | |
| GET | /api/diagnosticos | Listar diagnósticos |
| GET | /api/diagnosticos/search | Buscar diagnósticos |
| | **Sandbox** | |
| POST | /api/sandbox/generate | Generar datos sintéticos |
| POST | /api/sandbox/clear | Limpiar datos de prueba |
| | **FHIR R4** | |
| GET | /api/fhir/Patient | Búsqueda FHIR Patient |
| GET | /api/fhir/Patient/:id | Obtener FHIR Patient |
| | **Export** | |
| GET | /api/patients/:id/export/csv | Exportar CSV |
| GET | /api/patients/:id/export/pdf | Exportar PDF |

---

## Testing

### Tests

```bash
# Tests compartidos (cálculos clínicos)
cargo test -p dmart-shared

# Tests del servidor (rate limiter, login throttle, sanitización)
cargo test -p dmart-server
```

**dmart-shared** (66+ tests):
- ✅ Tests de cálculo APACHE II (40+ tests)
- ✅ Tests de GCS (7 tests)
- ✅ Tests de mortalidad (5 tests)
- ✅ Tests de integración (2 tests)
- ✅ Tests de validación (9 tests)
- ✅ Tests NEWS2, SAPS III, SOFA

**dmart-server**:
- ✅ Rate limiter (sliding window)
- ✅ Login throttle (brute force protection)
- ✅ Sanitización de entrada
- ✅ Escape HTML

---

## Rendimiento

### Benchmarks Típicos

| Operación | Tiempo |
|-----------|--------|
| Cálculo APACHE II | <1ms |
| Crear paciente | ~10ms |
| Listar pacientes (paginado) | ~5ms |
| Get paciente | ~2ms |
| Búsqueda por query | ~10ms |
| Export CSV | ~50ms |

> **Nota:** Todos los endpoints de listado soportan paginación via `?limit=N&offset=0`.
> Las queries de búsqueda se ejecutan en SurrealDB con `WHERE ~=` (no filtrado en memoria).

---

## Seguridad

- ✅ JWT con expiración configurable (JWT_EXPIRY_HOURS)
- ✅ Argon2id para hashing de contraseñas (HIPAA compliant)
- ✅ CORS restringido a orígenes configurables (DMART_CORS_ORIGIN)
- ✅ Rate limiter (100 req/min por IP)
- ✅ Login throttle (5 intentos → bloqueo 5 min)
- ✅ Security headers (CSP, X-Frame-Options, XSS Protection, etc.)
- ✅ DefaultBodyLimit (1MB) anti-DOS
- ✅ Auth middleware en todas las rutas API (excepto /auth/login)
- ✅ RBAC (Admin, Medico, Enfermero, Viewer)
- ✅ Auditoría HIPAA (login/logout/acceso PHI)
- ✅ Cifrado ChaCha20Poly1305 para datos sensibles
- ✅ Sanitización de entrada (anti XSS, anti injection)
- ✅ Zeroize para datos sensibles en memoria

---

## Escalabilidad

### Actual (Monolítico)
- Servidor único con BD embebida
- Ideal para Hospitales pequeños/medianos

### Futuro (Distribuido)
- SurrealDB en modo cluster
- Valkey/Redis para cache distribuido
- Balanceador de carga

---

## Métricas

### Health Check

```bash
GET /api/health
```

```json
{
  "status": "healthy",
  "version": "0.1.0",
  "timestamp": "2026-06-02T12:00:00Z",
  "uptime_seconds": 3600,
  "database": "connected",
  "cache": "connected"
}
```
