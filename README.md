# dMart - Sistema de Gestión de Unidad de Cuidados Intensivos

<p align="center">
  <img src="https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white" alt="Rust">
  <img src="https://img.shields.io/badge/WebAssembly-654FF0?style=for-the-badge&logo=webassembly&logoColor=white" alt="WASM">
  <img src="https://img.shields.io/badge/Leptos-FF4B4B?style=for-the-badge&logo=leptos&logoColor=white" alt="Leptos">
  <img src="https://img.shields.io/badge/SurrealDB-FF00A0?style=for-the-badge&logo=surrealdb&logoColor=white" alt="SurrealDB">
</p>

---

## 📋 Descripción

**dMart** es un sistema integral para la gestión de pacientes en Unidades de Cuidados Intensivos (UCI), desarrollado completamente en **Rust** con tecnología WebAssembly. El sistema proporciona cálculo automático de scores de severidad **APACHE II** y **Glasgow Coma Scale (GCS)**, junto con estimación de riesgo de mortalidad hospitalaria.

Este proyecto fue diseñado siguiendo los estándares clínicos internacionales y cuenta con una suite completa de pruebas de validación que garantizan la precisión de los cálculos médicos.

### Destacados

- ✅ Cálculo automático de APACHE II (12 variables fisiológicas)
- ✅ Cálculo de Glasgow Coma Scale (GCS)
- ✅ Estimación de riesgo de mortalidad hospitalaria
- ✅ 63+ tests de validación pasando
- ✅ Documentación técnica completa
- ✅ Arquitectura moderna y escalable

---

## 🏗️ Stack Tecnológico

| Capa | Tecnología | Versión | Descripción |
|------|------------|---------|-------------|
| **Lenguaje** | Rust | 1.70+ | Sistema de tipos seguros, sin GC |
| **Backend** | Axum | 0.7 | Framework web async, alto rendimiento |
| **Frontend** | Leptos | 0.7 | Framework reactivo WASM |
| **WASM Build** | Trunk | 0.21 | Build tool para aplicaciones WASM |
| **Estilos** | TailwindCSS | 3.x | CSS utilitario moderno |
| **Base de Datos** | SurrealDB | 2.x | Base de datos embebida (RocksDB) |
| **Cache** | Valkey/Redis | 6+ | Cache de sesiones y datos |
| **Serialización** | Serde | 1.x | Serialización/deserialización JSON |

### Diagrama de Arquitectura

```
┌─────────────────────────────────────────────────────────────────────┐
│                          dMart UCI System                            │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌──────────────────┐           ┌────────────────────────────────┐  │
│  │    Navegador    │           │         Servidor Backend         │  │
│  │   (WASM/WASM)   │◄────────►│         (Rust + Axum)           │  │
│  │                  │   HTTP    │                                │  │
│  │  ┌────────────┐  │           │  ┌──────────┐ ┌────────────┐  │  │
│  │  │  Leptos   │  │           │  │   API    │ │  Metrics   │  │  │
│  │  │  Router   │  │           │  │  REST    │ │  /health   │  │  │
│  │  └────────────┘  │           │  └────┬─────┘ └────────────┘  │  │
│  │                  │           │       │                        │  │
│  │  ┌────────────┐  │           │  ┌────┴─────────────────────┐ │  │
│  │  │   UI       │  │           │  │   Business Logic         │ │  │
│  │  │ Components │  │           │  │ - Scales Calculation     │ │  │
│  │  └────────────┘  │           │  │ - Validation            │ │  │
│  └──────────────────┘           │  │ - Export (CSV/PDF)      │ │  │
│                                 │  └─────────────────────────┘ │  │
│                                 └────────────────────────────────┘  │
│                                              │                      │
│                                 ┌────────────┴────────────┐        │
│                                 │                         │        │
│                        ┌────────▼────────┐    ┌────────▼───────┐  │
│                        │    SurrealDB    │    │ Valkey/Redis  │  │
│                        │    (RocksDB)    │    │    (Cache)    │  │
│                        └─────────────────┘    └────────────────┘  │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

---

## 🎯 Características Principales

### Gestión de Pacientes
- Registro completo de datos demográficos
- Historial clínico completo
- Seguimiento de ingreso hospitalario y UCI
- Soporte para diversidad de tono de piel
- Datos de contacto de familiares responsables

### Evaluación Clínica
- **12 variables fisiológicas** para APACHE II:
  - Temperatura, Presión arterial media
  - Frecuencia cardíaca, Frecuencia respiratoria
  - Oxigenación (PaO2 / A-aDO2)
  - pH arterial, Sodio, Potasio
  - Creatinina, Hematocrito, Leucocitos
  - Glasgow Coma Scale (GCS)
- Puntuación por edad (0-6 puntos)
- Evaluación de enfermedades crónicas severas (5 puntos)
- **Score máximo: 71 puntos**

### Scores y Métricas
- Cálculo automático de APACHE II
- Cálculo automático de GCS (3-15 puntos)
- Estimación de riesgo de mortalidad hospitalaria
- Clasificación de severidad (Bajo/Moderado/Severo/Crítico)
- Evolución temporal del paciente con gráficos

### Exportación
- Reportes en formato CSV
- Reportes en formato PDF
- Historial completo de mediciones

---

## 📊 Pruebas y Validación

### Suite de Tests: 63+ Tests

El sistema cuenta con una suite completa de pruebas que validan:

```
cargo test -p dmart-shared
```

| Categoría | Tests | Descripción |
|----------|-------|-------------|
| **APACHE II** | 40+ | Validación de cada variable fisiológica |
| **GCS** | 7 | Cálculo de coma de Glasgow |
| **Mortalidad** | 5 | Fórmula de riesgo hospitalario |
| **Validación** | 9 | Rangos clínicos válidos |
| **Integración** | 2 | Casos de uso completos |

### Tests de Variables APACHE II

| Variable | Tests |
|----------|-------|
| Temperatura | Normal, Fiebre alta, Hipotermia |
| Presión Arterial | Normal, Alta, Baja |
| Frecuencia Cardíaca | Normal, Taquicardia, Bradicardia |
| Frecuencia Respiratoria | Normal, Alta |
| Oxigenación (PaO2) | Normal, Bajo, Crítico |
| Oxigenación (A-aDO2) | Normal, Alto |
| pH Arterial | Normal, Acidosis, Alcalosis |
| Sodio | Normal, Alto |
| Potasio | Normal, Alto, Bajo |
| Creatinina | Normal, Alta, Con falla aguda |
| Hematocrito | Normal, Bajo |
| Leucocitos | Normal, Alto |
| Edad | Joven, Mediana, Anciano, Muy anciano |
| GCS | Normal, Moderado, Coma |

### Validación Clínica

El módulo de validación (`validation.rs`) verifica:
- Rangos físicos posibles para cada variable
- Valores críticos (warnings)
- Valores inválidos (errors)
- Consistencia del GCS

```rust
// Ejemplo de validación
use dmart_shared::validation::{validate_apache_measurement, ValidationResult};

let result = validate_apache_measurement(&data);
if !result.valid {
    for error in result.errors {
        println!("Error: {} - {}", error.field, error.message);
    }
}
```

---

## 🔒 Seguridad

### Medidas Implementadas

| Seguridad | Descripción |
|-----------|-------------|
| **CORS** | Configuración de Cross-Origin Resource Sharing |
| **Validación de Entrada** | Sanitización de datos en backend |
| **Typesafe** | Rust previene bugs en tiempo de compilación |
| **WASM** | Frontend compilado, no código fuente expuesto |
| **Base de Datos Embebida** | Datos locales, no expuestos a internet |
| **Prepared Statements** | Consultas parametrizadas (SurrealDB) |

### Logging

Sistema de logging configurable:

```bash
RUST_LOG=debug cargo run    # Verboso
RUST_LOG=info cargo run    # Normal
RUST_LOG=warn cargo run    # Solo advertencias
```

---

## 🚀 Instalación y Ejecución

### Requisitos

- **Rust 1.70+**: https://rustup.rs/
- **Node.js 18+** (opcional, para desarrollo frontend)
- **Trunk**: `cargo install trunk`

### Compilación Rápida

```bash
# Compilar todo el proyecto
cargo build --release

# Compilar frontend
cd dmart-app && trunk build
```

### Ejecución

```bash
# Ejecutar servidor
./target/release/dmart-server
```

El servidor estará disponible en: **http://localhost:3000**

### Desarrollo

```bash
# Terminal 1: Frontend
cd dmart-app && trunk serve

# Terminal 2: Backend
cd dmart-server && cargo run
```

---

## ⚙️ Configuración

### Variables de Entorno

| Variable | Default | Descripción |
|----------|---------|-------------|
| `DMART_PORT` | `3000` | Puerto del servidor HTTP |
| `DMART_DB_PATH` | `./data/dmart.db` | Ruta de la base de datos |
| `DMART_DIST_PATH` | `./dist` | Ruta de archivos estáticos (WASM) |
| `DMART_VALKEY_URL` | `redis://127.0.0.1:6379` | URL de cache (opcional) |
| `RUST_LOG` | `info` | Nivel de logging |

### Ejemplo de Configuración

```bash
export DMART_PORT=3000
export DMART_DB_PATH=./data/dmart.db
export DMART_DIST_PATH=./dist
export RUST_LOG=info
./target/release/dmart-server
```

---

## 📡 API REST

### Endpoints Disponibles

#### Health Check
```http
GET /api/health
```

#### Pacientes
```http
GET    /api/patients              # Listar todos
POST   /api/patients              # Crear
GET    /api/patients/:id          # Obtener uno
PUT    /api/patients/:id          # Actualizar
DELETE /api/patients/:id          # Eliminar
```

#### Mediciones
```http
GET  /api/patients/:id/measurements         # Listar
POST /api/patients/:/measurements           # Crear
GET  /api/patients/:id/measurements/last    # Última medición
```

#### Exportación
```http
GET /api/patients/:id/export/csv   # Exportar CSV
GET /api/patients/:id/export/pdf  # Exportar PDF
```

### Formato de Respuesta

```json
{
  "success": true,
  "data": { ... },
  "error": null
}
```

---

## 📂 Estructura del Proyecto

```
dmart/
├── Cargo.toml                  # Workspace raíz
├── README.md                  # Este archivo
│
├── dmart-shared/             # Biblioteca compartida
│   ├── src/
│   │   ├── lib.rs            # Exports públicos
│   │   ├── models.rs        # Estructuras de datos
│   │   ├── scales.rs         # Algoritmos clínicos
│   │   └── validation.rs     # Validación de datos
│   └── tests/
│       └── scale_tests.rs    # Suite de pruebas (54 tests)
│
├── dmart-server/             # Servidor backend
│   ├── src/
│   │   ├── main.rs           # Punto de entrada
│   │   ├── api/              # Endpoints REST
│   │   ├── db.rs             # Conexión SurrealDB
│   │   └── cache.rs          # Cache Valkey/Redis
│   └── Cargo.toml
│
├── dmart-app/                # Frontend WASM
│   ├── src/
│   │   ├── main.rs           # Entry point
│   │   ├── app.rs            # Router
│   │   ├── api.rs            # Cliente HTTP
│   │   ├── pages/            # Páginas UI
│   │   └── components/       # Componentes
│   ├── index.html
│   ├── Trunk.toml
│   └── Cargo.toml
│
├── dist/                     # Frontend compilado (WASM)
├── data/                     # Base de datos
└── docs/                     # Documentación técnica
    ├── API.md
    ├── APACHE_II.md
    ├── GCS.md
    └── ARQUITECTURA.md
```

---

## 📚 Referencias Clínicas

### APACHE II
- **Knaus WA**, Draper EA, Wagner DP, Zimmerman JE (1985). APACHE II: a severity of disease classification system. Crit Care Med. 13(10):818-29.

### Glasgow Coma Scale
- **Teasdale GM**, Jennett B (1974). Assessment of coma and impaired consciousness. Lancet. 2(7872):81-4.

---

## 📈 Métricas de Rendimiento

| Operación | Tiempo Típico |
|-----------|---------------|
| Cálculo APACHE II | <1ms |
| Crear paciente | ~10ms |
| Listar pacientes | ~5ms |
| Obtener paciente | ~2ms |
| Export CSV | ~50ms |
| Export PDF | ~100ms |

---

## 🏆 Logros del Proyecto

| Logro | Descripción |
|-------|-------------|
| ✅ Sistema completo | Gestión total de UCI desde cero |
| ✅ Estándar clínico | Implementación fiel de APACHE II (Knaus 1985) |
| ✅ Tests rigurosos | 63+ tests validando cálculos médicos |
| ✅ Tipado seguro | Rust previene errores en tiempo de compilación |
| ✅ Documentación | 4 documentos técnicos para artículo |
| ✅ UI moderna | Interfaz glassmorphism responsiva |
| ✅ WASM | Frontend compilado, alto rendimiento |
| ✅ Empotrado | Base de datos local, sin infraestructura |

---

## 🤝 Contribución

Este proyecto está bajo licencia MIT. Siéntete libre de:

- Reportar bugs
- Sugerir nuevas características
- Enviar pull requests
- Utilizar para proyectos académicos

---

## 📄 Licencia

MIT License - Copyright (c) 2026

---

<p align="center">
  <strong>dMart UCI</strong> - Sistema de Gestión de Cuidados Intensivos<br>
  Desarrollado con ❤️ en Rust
</p>
