# dMart - Sistema de Gestión de UCI

Sistema integral para la gestión de pacientes en Unidades de Cuidados Intensivos, con cálculo automático de scores de severidad (APACHE II, GCS) y estimación de riesgo de mortalidad hospitalaria.

## Características

- **Gestión de Pacientes**: Registro completo de datos demográficos, clínicos e historia de ingreso a UCI
- **Evaluación Diaria**: Registro de mediciones fisiológicas para cálculo de APACHE II y GCS
- **Scores Automáticos**: Cálculo instantáneo de severidad y riesgo de mortalidad
- **Dashboard**: Visualización de estadísticas y evolución de pacientes
- **Exportación**: Generación de reportes en CSV y PDF
- **Diseño Responsivo**: Interfaz moderna con soporte para diferentes tonos de piel

## Stack Tecnológico

| Componente | Tecnología |
|------------|------------|
| Backend | Rust + Axum |
| Frontend | Leptos (WASM) + TailwindCSS |
| Base de Datos | SurrealDB (RocksDB) |
| Cache | Redis/ValKey (opcional) |
| Scoring | APACHE II, GCS |

## Estructura del Proyecto

```
dmart/
├── dmart-shared/       # Modelos y lógica compartida
│   └── src/
│       ├── models.rs   # Estructuras de datos
│       └── scales.rs   # Algoritmos de scoring
├── dmart-server/       # Servidor API
│   └── src/
│       ├── api/        # Endpoints REST
│       ├── db.rs       # Conexión a base de datos
│       └── main.rs     # Punto de entrada
├── dmart-app/         # Aplicación frontend (WASM)
│   └── src/
│       ├── pages/      # Páginas de la UI
│       └── components/ # Componentes reutilizables
├── dist/              # Frontend compilado
└── data/              # Base de datos
```

## Requisitos

- Rust 1.70+
- Node.js 18+ (para TailwindCSS)
- SurrealDB o simplemente RocksDB (embebido)

## Instalación

```bash
# Instalar dependencias Rust
cargo build

# Instalar dependencias frontend
cd dmart-app && npm install
```

## Ejecución

### Desarrollo

```bash
# Compilar frontend (desde dmart-app)
trunk serve

# Ejecutar servidor (desde raíz)
cd dmart-server && cargo run
```

### Producción

```bash
# Compilar release
cargo build --release

# Compilar frontend
cd dmart-app && trunk build

# Ejecutar servidor
DMART_PORT=3000 DMART_DB_PATH=./data/dmart.db cargo run --release
```

## Variables de Entorno

| Variable | Descripción | Default |
|----------|-------------|---------|
| `DMART_PORT` | Puerto del servidor | `3000` |
| `DMART_DB_PATH` | Ruta a la base de datos | `./data/dmart.db` |
| `DMART_DIST_PATH` | Ruta al frontend compilado | `./dist` |
| `DMART_VALKEY_URL` | URL de Redis/ValKey | `redis://127.0.0.1:6379` |
| `RUST_LOG` | Nivel de logging | `info` |

## API Endpoints

### Pacientes

| Método | Endpoint | Descripción |
|--------|----------|-------------|
| GET | `/api/patients` | Listar pacientes (con búsqueda) |
| POST | `/api/patients` | Crear paciente |
| GET | `/api/patients/:id` | Obtener paciente |
| PUT | `/api/patients/:id` | Actualizar paciente |
| DELETE | `/api/patients/:id` | Eliminar paciente |

### Mediciones

| Método | Endpoint | Descripción |
|--------|----------|-------------|
| GET | `/api/patients/:id/measurements` | Listar mediciones |
| POST | `/api/patients/:id/measurements` | Crear medición |
| GET | `/api/patients/:id/measurements/last` | Última medición |

### Exportación

| Método | Endpoint | Descripción |
|--------|----------|-------------|
| GET | `/api/patients/:id/export/csv` | Exportar a CSV |
| GET | `/api/patients/:id/export/pdf` | Exportar a PDF |

## Scores Clínicos

### APACHE II

Sistema de puntuación de gravedad basado en 12 variables fisiológicas, edad y enfermedades crónicas. Rango: 0-71 puntos.

**Clasificación de Severidad:**
- 0-9: Bajo (< 10% mortalidad)
- 10-19: Moderado (10-25% mortalidad)
- 20-29: Severo (25-50% mortalidad)
- ≥30: Crítico (> 50% mortalidad)

### Glasgow Coma Scale (GCS)

Escala de 3-15 puntos que evalúa el nivel de conciencia:
- 15: Consciente
- 13-14: Lesión leve
- 9-12: Lesión moderada
- 3-8: Lesión grave / Coma

## Licencia

MIT License
