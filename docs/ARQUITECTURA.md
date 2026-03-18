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
│                    │  (RocksDB)  │    │   (Cache)     │       │
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
| Base de Datos | SurrealDB (RocksDB) | Almacenamiento embebido |
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
│   │   ├── scales.rs       # Algoritmos APACHE II, GCS
│   │   ├── validation.rs   # Validación de rangos clínicos
│   │   └── lib.rs         # Exports públicos
│   └── tests/
│       └── scale_tests.rs  # Tests de validación (54 tests)
│
├── dmart-server/           # Servidor backend
│   └── src/
│       ├── main.rs         # Punto de entrada
│       ├── api/           # Endpoints REST
│       │   ├── patients.rs
│       │   ├── measurements.rs
│       │   └── export.rs
│       ├── db.rs          # Conexión SurrealDB
│       └── cache.rs       # Cache Valkey/Redis
│
├── dmart-app/             # Frontend WASM
│   ├── src/
│   │   ├── main.rs        # Entry point
│   │   ├── app.rs         # Router y navegación
│   │   ├── api.rs         # Cliente HTTP
│   │   ├── pages/         # Páginas
│   │   │   ├── login.rs
│   │   │   ├── dashboard.rs
│   │   │   ├── patients.rs
│   │   │   ├── patient_detail.rs
│   │   │   ├── patient_edit.rs
│   │   │   └── measurement.rs
│   │   └── components/    # Componentes UI
│   ├── index.html
│   └── Trunk.toml
│
├── dist/                  # Frontend compilado
├── data/                  # Base de datos
└── docs/                  # Documentación
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
| `DMART_VALKEY_URL` | redis://127.0.0.1:6379 | URL Redis |
| `RUST_LOG` | info | Nivel de logging |

---

## API Endpoints

| Método | Endpoint | Descripción |
|--------|----------|-------------|
| GET | /api/health | Health check |
| GET | /api/patients | Listar pacientes |
| POST | /api/patients | Crear paciente |
| GET | /api/patients/:id | Obtener paciente |
| PUT | /api/patients/:id | Actualizar paciente |
| DELETE | /api/patients/:id | Eliminar paciente |
| GET | /api/patients/:id/measurements | Listar mediciones |
| POST | /api/patients/:id/measurements | Crear medición |
| GET | /api/patients/:id/measurements/last | Última medición |
| GET | /api/patients/:id/export/csv | Exportar CSV |
| GET | /api/patients/:id/export/pdf | Exportar PDF |

---

## Testing

### Tests de Integración (54 tests)

```bash
cargo test -p dmart-shared
```

- ✅ Tests de cálculo APACHE II (40+ tests)
- ✅ Tests de GCS (7 tests)
- ✅ Tests de mortalidad (5 tests)
- ✅ Tests de integración (2 tests)
- ✅ Tests de validación (9 tests)

---

## Rendimiento

### Benchmarks Típicos

| Operación | Tiempo |
|-----------|--------|
| Cálculo APACHE II | <1ms |
| Crear paciente | ~10ms |
| Listar pacientes | ~5ms |
| Get paciente | ~2ms |
| Export CSV | ~50ms |

---

## Seguridad

- ✅ CORS configurado
- ✅ Validación de entrada
- ✅ Sanitización de datos
- ✅ Prepared statements (SurrealDB)

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
  "timestamp": "2026-03-18T12:00:00Z"
}
```
