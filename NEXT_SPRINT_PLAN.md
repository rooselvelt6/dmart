# Next Sprint Plan — dMart UCI
*Deuda técnica + hardening producción*

---

## Sprint Goal
**Sistema listo para producción real: CI/CD automatizado, tests unitarios críticos, observabilidad completa, backup/restore, release binaries automatizados.**

---

## 1. CI/CD Pipeline (Semana 1)
**Archivo:** `.github/workflows/ci.yml`

```yaml
name: CI
on: [push, pull_request]
jobs:
  test-server:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Cache cargo
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/bin
            ~/.cargo/registry/index
            ~/.cargo/registry/cache
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
      - run: cargo test -p dmart-server --lib --test api_tests --test hl7_integration -- --nocapture
  
  build-frontend:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: jetli/wasm-pack-action@v0.4.0
      - name: Install trunk
        run: cargo install trunk --locked
      - working-directory: dmart-app
        run: trunk build --release
      - name: Upload dist
        uses: actions/upload-artifact@v4
        with:
          name: dist
          path: dist/
  
  build-server-release:
    runs-on: ubuntu-latest
    needs: test-server
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Cache cargo
        uses: actions/cache@v4
        with:
          path: target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
      - run: cargo build --release -p dmart-server
      - name: Upload binary
        uses: actions/upload-artifact@v4
        with:
          name: dmart-server
          path: target/release/dmart-server
```

**Entregable:** `main` protegido — merge solo si CI verde.

---

## 2. Tests Unitarios Críticos (Semana 1-2)
**Archivo:** `dmart-server/src/db.rs` — añadir `#[cfg(test)] mod tests`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use dmart_shared::models::*;
    
    async fn test_db() -> Surreal<Db> {
        let dir = tempfile::tempdir().unwrap();
        Surreal::new::<SerdeKv>(dir.path()).await.unwrap()
    }
    
    #[tokio::test]
    async fn create_patient_propagates_patient_id() {
        let db = test_db().await;
        let mut p = Patient::new();
        p.nombre = "Test".into();
        p.apellido = "User".into();
        p.patient_id = "".into(); // vacío → debe generarse
        let created = create_patient_with_assignments(&db, p, &[]).await.unwrap();
        assert!(!created.patient_id.is_empty(), "patient_id debe propagarse");
        assert_eq!(created.patient_id, created.id.as_deref().unwrap_or(""));
    }
    
    #[tokio::test]
    async fn init_camas_idempotent() {
        let db = test_db().await;
        let c1 = init_camas(&db, 5, TipoCama::General).await.unwrap();
        let c2 = init_camas(&db, 3, TipoCama::General).await.unwrap(); // suma 3 más
        assert_eq!(c1.len(), 5);
        assert_eq!(c2.len(), 3);
        let all = list_camas(&db).await.unwrap();
        assert_eq!(all.len(), 8);
        // números únicos
        let nums: Vec<u8> = all.iter().map(|c| c.numero).collect();
        assert_eq!(nums, (1..=8).collect::<Vec<u8>>());
    }
    
    #[tokio::test]
    async fn create_equipo_assigns_to_cama() {
        let db = test_db().await;
        let cama = init_camas(&db, 1, TipoCama::General).await.unwrap().pop().unwrap();
        let mut eq = Equipo::new("Ventilador".into(), TipoEquipo::VentiladorMecanico);
        eq.cama_id = Some(cama.cama_id.clone());
        let created = create_equipo(&db, eq).await.unwrap();
        assert_eq!(created.cama_id, Some(cama.cama_id));
        // verifica cama actualizada
        let cama_u = get_cama(&db, &cama.cama_id).await.unwrap().unwrap();
        assert_eq!(cama_u.estado, EstadoCama::Ocupada);
    }
    
    #[tokio::test]
    async fn scale_calculation_fingerprint_deterministic() {
        let data = ApacheIIData::default();
        let fp1 = score_fingerprint("apache_ii", ALGO_VERSION, &serde_json::to_value(&data).unwrap());
        let fp2 = score_fingerprint("apache_ii", ALGO_VERSION, &serde_json::to_value(&data).unwrap());
        assert_eq!(fp1, fp2);
    }
}
```

**Comando:** `cargo test -p dmart-server --lib` — debe pasar en <15s.

---

## 3. Observabilidad & Operación (Semana 2)
### 3.1 Health checks extendidos
**Archivo:** `dmart-server/src/api/health.rs`

```rust
pub async fn deep_health(State(db): State<Database>) -> impl IntoResponse {
    let mut checks = HashMap::new();
    
    // DB
    checks.insert("database", match db.query("SELECT 1").await { Ok(_) => "healthy", Err(_) => "unhealthy" });
    
    // Cache
    checks.insert("cache", if cache::cache_available() { "healthy" } else { "degraded" });
    
    // Disk space
    let disk = fs2::available_space(".").unwrap_or(0);
    checks.insert("disk", if disk > 1_000_000_000 { "healthy" } else { "warning" });
    
    // Migration status (si usas SurrealDB server)
    // checks.insert("migrations", ...);
    
    let status = if checks.values().all(|v| *v == "healthy") { "healthy" } else { "degraded" };
    (StatusCode::OK, Json(serde_json::json!({ "status": status, "checks": checks }))).into_response()
}
```

### 3.2 Backup/Restore CLI
**Archivo:** `dmart-server/src/bin/backup.rs` (nuevo binary)

```rust
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
enum Cmd {
    Backup { output: PathBuf },
    Restore { input: PathBuf },
}

fn main() -> anyhow::Result<()> {
    let cmd = Cmd::parse();
    let db = Surreal::new::<SerdeKv>("./data/dmart.db").await?;
    
    match cmd {
        Cmd::Backup { output } => {
            let mut file = std::fs::File::create(output)?;
            // Export all tables as JSON
            for table in ["patients", "camas", "equipos", "users", "audit_logs", "institucion"] {
                let rows: Vec<serde_json::Value> = db.query(&format!("SELECT * FROM {}", table)).await?.take(0)?;
                serde_json::to_writer(&mut file, &rows)?;
                writeln!(&mut file)?;
            }
        }
        Cmd::Restore { input } => {
            // CUIDADO: solo para dev / disaster recovery
            let content = std::fs::read_to_string(input)?;
            for line in content.lines() {
                let rows: Vec<serde_json::Value> = serde_json::from_str(line)?;
                for row in rows {
                    db.create("auto", row).await?;
                }
            }
        }
    }
    Ok(())
}
```

**Uso:**
```bash
cargo run --bin backup -- backup --output backup-$(date +%F).json
cargo run --bin backup -- restore --input backup-2026-09-18.json
```

---

## 4. Release Binary Automation (Semana 2)
### 4.1 Dockerfile optimizado
**Archivo:** `Dockerfile`

```dockerfile
# Builder
FROM rust:1.80 AS builder
WORKDIR /app
COPY . .
RUN cargo build --release -p dmart-server

# Runtime (distroless)
FROM gcr.io/distroless/cc-debian12
COPY --from=builder /app/target/release/dmart-server /dmart-server
COPY --from=builder /app/dist /dist
COPY --from=builder /app/data ./data
ENV DMART_DB_PATH=/data/dmart.db
ENV DMART_DIST_PATH=/dist
ENV DMART_VALKEY_URL=redis://valkey:6379
EXPOSE 3000
ENTRYPOINT ["/dmart-server"]
```

### 4.2 Docker Compose prod
**Archivo:** `docker-compose.prod.yml`

```yaml
services:
  dmart-server:
    build: .
    image: dmart/dmart-server:${VERSION:-latest}
    restart: unless-stopped
    ports: ["3000:3000"]
    environment:
      - DMART_DB_PATH=/data/dmart.db
      - DMART_DIST_PATH=/dist
      - DMART_VALKEY_URL=redis://valkey:6379
      - RUST_LOG=info
    volumes:
      - ./data:/data
      - ./dist:/dist:ro
    depends_on:
      valkey:
        condition: service_healthy
  
  valkey:
    image: valkey/valkey:8-alpine
    command: valkey-server --appendonly yes
    volumes: [valkey-data:/data]
    healthcheck:
      test: ["CMD", "valkey-cli", "ping"]
      interval: 10s
      timeout: 3s
      retries: 3

volumes:
  valkey-data:
```

**Deploy:** `docker compose -f docker-compose.prod.yml up -d`

---

## 5. API Consistency (Semana 3)
**Archivo:** `dmart-server/src/api/versioning.rs` — normalizar todas las respuestas a `PaginatedResponse`

```rust
// En build_v1_router, envolver routers admin/patients/equipos con layer que normalice
fn paginated_layer<T: Serialize>(router: Router) -> Router {
    router.layer(axum::middleware::from_fn(normalize_paginated_response))
}

async fn normalize_paginated_response(req: Request, next: Next) -> Response {
    let mut res = next.run(req).await;
    if res.status().is_success() {
        // Si body es Array → envolver en {items, total, limit, offset}
        // Si body es Object con "items" → ya OK
    }
    res
}
```

---

## 6. Métricas & Alertas (Semana 3)
**Archivo:** `dmart-server/src/observability/metrics.rs` — añadir:

```rust
// Business metrics
lazy_static::lazy_static! {
    static ref PATIENTS_ACTIVE: IntGauge = register_int_gauge!("dmart_patients_active", "Pacientes en UCI").unwrap();
    static ref CAMAS_OCUPADAS: IntGauge = register_int_gauge!("dmart_camas_ocupadas", "Camas ocupadas").unwrap();
    static ref EQUIPOS_DISPONIBLES: IntGauge = register_int_gauge!("dmart_equipos_disponibles", "Equipos libres").unwrap();
    static ref EXPORT_PDF_TOTAL: IntCounter = register_int_counter!("dmart_export_pdf_total", "PDFs generados").unwrap();
    static ref EXPORT_CSV_TOTAL: IntCounter = register_int_counter!("dmart_export_csv_total", "CSVs generados").unwrap();
    static ref SCALE_CALC_DURATION: Histogram = register_histogram!("dmart_scale_calc_seconds", "Tiempo cálculo escalas").unwrap();
}

// Actualizar en create_patient / egreso / export / scale endpoints
```

**Alertas sugeridas (PrometheusRule):**
```yaml
groups:
- name: dmart.rules
  rules:
  - alert: HighBedOccupancy
    expr: dmart_camas_ocupadas / dmart_camas_total > 0.9
    for: 5m
    labels: { severity: warning }
    annotations: { summary: "Ocupación UCI > 90%" }
  - alert: ScaleCalcSlow
    expr: histogram_quantile(0.95, dmart_scale_calc_seconds_bucket) > 2
    for: 2m
    labels: { severity: warning }
    annotations: { summary: "Cálculo escalas > 2s (p95)" }
```

---

## 7. Documentación (Semana 3)
- `docs/API.md` — OpenAPI generado + ejemplos curl
- `docs/DEPLOY.md` — Docker + backup/restore + migraciones
- `docs/ARCHITECTURE.md` — Diagramas (C4: Context, Container, Component)

---

## Definition of Done (Sprint)
- [ ] CI verde en `main` (tests + build frontend + build server release)
- [ ] Tests unitarios `db.rs` pasando (`cargo test -p dmart-server --lib`)
- [ ] `cargo run --bin backup` funciona (backup + restore en temp DB)
- [ ] Docker compose prod arranca con `docker compose -f docker-compose.prod.yml up -d`
- [ ] Health endpoint `/obs/health` muestra DB, cache, disco, migraciones
- [ ] Métricas Prometheus expuestas en `/metrics` con business metrics
- [ ] Release binary `dmart-server` subido como artifact en CI
- [ ] 0 warnings `cargo clippy -p dmart-server`

---

## Estimación de Esfuerzo
| Item | Días | Riesgo |
|------|------|--------|
| CI/CD | 2 | Bajo |
| Tests unitarios db.rs | 3 | Bajo |
| Observabilidad (health/metrics/backup) | 3 | Medio |
| Docker + release binary | 2 | Bajo |
| API consistency | 2 | Medio |
| Docs | 2 | Bajo |
| **Total** | **14 días** | |

---

## 8. Usabilidad, Accesibilidad e Internacionalización (Semana 1-2)
### 8.1 Internacionalización (i18n) — ES/EN/PT/FR
**Archivo:** `dmart-app/locales/*.ftl` + `dmart-app/src/i18n.rs`

```rust
// dmart-app/src/i18n.rs
use fluent_templates::Loader;
use fluent_bundle::{FluentBundle, FluentResource};
use std::sync::OnceLock;

static BUNDLES: OnceLock<Vec<FluentBundle<FluentResource>>> = OnceLock::new();

pub fn init_i18n() {
    let locales = ["es", "en", "pt", "fr"];
    let bundles: Vec<_> = locales.iter().map(|lang| {
        let ftl = include_str!(concat!("../../locales/", lang, ".ftl"));
        let res = FluentResource::try_new(ftl.to_string()).unwrap();
        let mut bundle = FluentBundle::new(vec![lang.parse().unwrap()]);
        bundle.add_resource(res).unwrap();
        bundle
    }).collect();
    BUNDLES.set(bundles).unwrap();
}

pub fn tr(key: &str, args: Option<&fluent_bundle::FluentArgs>) -> String {
    let bundles = BUNDLES.get().unwrap();
    // usa el primer bundle que tenga la key (orden de preferencia)
    for bundle in bundles {
        if let Some(msg) = bundle.get_message(key) {
            if let Some(pattern) = msg.value() {
                let mut errors = vec![];
                return bundle.format_pattern(pattern, args, &mut errors).unwrap().into_owned();
            }
        }
    }
    key.to_string()
}
```

**Archivos `.ftl` (ejemplo `locales/es.ftl`):**
```ftl
app-title = "dMart UCI"
login-title = "Iniciar sesión"
login-username = "Usuario"
login-password = "Contraseña"
login-submit = "Entrar"
dashboard-title = "Panel de control"
patients-title = "Pacientes"
patients-new = "Nuevo paciente"
patients-search = "Buscar paciente…"
patients-export-pdf = "Exportar PDF"
patients-export-csv = "Exportar CSV"
beds-title = "Camas"
beds-free = "Libres"
beds-occupied = "Ocupadas"
equipment-title = "Equipos"
equipment-new = "Nuevo equipo"
settings-language = "Idioma"
settings-theme = "Tema"
theme-light = "Claro"
theme-dark = "Oscuro"
theme-system = "Sistema"
toast-saved = "Guardado correctamente"
toast-exported = "Exportado"
toast-error = "Error: {error}"
shortcut-new-patient = "Nuevo paciente (Alt+N)"
shortcut-search = "Buscar (Alt+B)"
shortcut-export-pdf = "Exportar PDF (Alt+E)"
shortcut-measurement = "Nueva medición (Alt+M)"
handoff-title = "Resumen de turno"
handoff-generate = "Generar reporte handoff"
```

**Integración en Leptos:**
```rust
// app.rs
use leptos::prelude::*;
use crate::i18n::{init_i18n, tr};

#[component]
pub fn App() -> impl IntoView {
    init_i18n(); // llama una vez
    view! {
        <html lang=move || get_lang()>
            <head>
                <meta charset="utf-8"/>
                <title>{move || tr("app-title", None)}</title>
            </head>
            <body>
                <LanguageSelector/>
                <Router>...</Router>
            </body>
        </html>
    }
}
```

---

### 8.2 Tema Claro/Oscuro + Alto Contraste
**Archivo:** `dmart-app/src/theme.rs` + CSS variables

```css
/* dmart-app/styles/theme.css */
:root {
  --bg: #0f172a; --bg-card: #1e293b; --text: #f1f5f9; --muted: #94a3b8;
  --primary: #22d3ee; --primary-hover: #06b6d4;
  --danger: #f87171; --success: #4ade80; --warning: #fbbf24;
  --border: #334155; --focus: #22d3ee;
}
@media (prefers-color-scheme: light) {
  :root { --bg: #f8fafc; --bg-card: #fff; --text: #0f172a; --muted: #64748b; --border: #e2e8f0; --focus: #0891b2; }
}
@media (prefers-contrast: more) {
  :root { --bg: #000; --bg-card: #111; --text: #fff; --primary: #0ff; --danger: #f00; --success: #0f0; --border: #fff; }
}
[data-theme="light"] { /* override para selector manual */ }
[data-theme="dark"] { /* override */ }
```

```rust
// dmart-app/src/theme.rs
use leptos::prelude::*;
use gloo_storage::{LocalStorage, Storage};

const THEME_KEY: &str = "dmart_theme";

#[derive(Clone, Copy, PartialEq)]
pub enum Theme { System, Light, Dark }

pub fn use_theme() -> (ReadSignal<Theme>, WriteSignal<Theme>) {
    let (theme, set_theme) = signal(LocalStorage::get(THEME_KEY).unwrap_or(Theme::System));
    Effect::new(move |_| {
        let t = theme.get();
        LocalStorage::set(THEME_KEY, t).ok();
        let doc = web_sys::window().unwrap().document().unwrap();
        let html = doc.document_element().unwrap();
        match t {
            Theme::Light => html.set_attribute("data-theme", "light").ok(),
            Theme::Dark => html.set_attribute("data-theme", "dark").ok(),
            Theme::System => html.remove_attribute("data-theme").ok(),
        }
    });
    (theme, set_theme)
}
```

**Selector en header:**
```rust
#[component]
pub fn ThemeSelector() -> impl IntoView {
    let (theme, set_theme) = use_theme();
    view! {
        <select class="theme-select" prop:value=move || theme.get() as u8 on:change=move |ev| {
            let v = event_target_value(&ev).parse::<u8>().unwrap();
            set_theme.set(unsafe { std::mem::transmute(v) });
        }>
            <option value=0>{tr("theme-system", None)}</option>
            <option value=1>{tr("theme-light", None)}</option>
            <option value=2>{tr("theme-dark", None)}</option>
        </select>
    }
}
```

---

### 8.3 Toast / Snackbar + Alertas Críticas
**Archivo:** `dmart-app/src/components/toast.rs`

```rust
use leptos::prelude::*;
use std::time::Duration;

#[derive(Clone)]
pub struct Toast { pub id: u32, pub kind: ToastKind, pub message: String }
#[derive(Clone, Copy, PartialEq)] pub enum ToastKind { Success, Error, Warning, Info }

#[component]
pub fn ToastContainer() -> impl IntoView {
    let (toasts, set_toasts) = signal(Vec::<Toast>::new());
    let add_toast = move |kind, msg| {
        let id = rand::random::<u32>();
        set_toasts.update(|t| t.push(Toast { id, kind, message: msg }));
        // auto-dismiss 3s
        set_timeout(move || set_toasts.update(|t| t.retain(|x| x.id != id)), Duration::from_secs(3));
    };
    provide_context(ToastContext { add: add_toast });
    view! {
        <div class="toast-container" role="status" aria-live="polite">
            {move || toasts.get().into_iter().map(|t| view! {
                <div class=format!("toast {}", match t.kind { ToastKind::Success => "success", ToastKind::Error => "error", ToastKind::Warning => "warning", _ => "info" })>
                    {t.message}
                </div>
            }).collect_view()}
        </div>
    }
}
```

**Uso:** `use_context::<ToastContext>().add(ToastKind::Success, tr("toast-saved", None))`

---

### 8.4 Atajos de Teclado (Power Users)
**Archivo:** `dmart-app/src/shortcuts.rs`

```rust
use leptos::prelude::*;
use web_sys::KeyboardEvent;

pub fn use_shortcuts() {
    let on_keydown = move |ev: KeyboardEvent| {
        if ev.alt_key() {
            match ev.key().as_str() {
                "n" => { ev.prevent_default(); navigate("/patients/new"); }
                "b" => { ev.prevent_default(); document().get_element_by_id("search-input").unwrap().focus(); }
                "e" => { ev.prevent_default(); export_current_pdf(); }
                "m" => { ev.prevent_default(); open_measurement_modal(); }
                _ => {}
            }
        }
        if ev.key() == "Escape" { close_modals(); }
        if ev.key() == "?" && ev.shift_key() { show_shortcuts_help(); }
    };
    window().add_event_listener_with_callback("keydown", on_keydown).ok();
    on_cleanup(|| window().remove_event_listener_with_callback("keydown", on_keydown).ok());
}
```

---

### 8.5 Accesibilidad (WCAG 2.1 AA) — Checklist
- [ ] Contraste ≥ 4.5:1 (audit con `axe-core` en CI)
- [ ] `:focus-visible` rings en todos los interactivos
- [ ] `aria-label` en botones solo-icono (PDF, CSV, editar, eliminar)
- [ ] Tablas virtualizadas: `role="row"`, `aria-rowindex`, `aria-rowcount`
- [ ] Formularios: `aria-invalid`, `aria-describedby` para errores
- [ ] `lang` attribute en `<html>` según i18n
- [ ] Skip link: `<a href="#main" class="skip-link">Saltar al contenido</a>`

---

### 8.6 Responsive Tablet-First (UCI usa tablets en cama)
```css
/* dmart-app/styles/responsive.css */
@media (max-width: 640px) {
  .sidebar { transform: translateX(-100%); }
  .sidebar.open { transform: translateX(0); }
  .patients-table { display: none; }
  .patients-cards { display: grid; grid-template-columns: 1fr; }
  .chart { width: 100%; height: 300px; }
}
@media (min-width: 641px) and (max-width: 1024px) {
  .sidebar { width: 64px; } /* solo iconos */
  .sidebar.expanded { width: 240px; }
  .patients-table { display: table; }
  .patients-cards { display: none; }
}
@media (min-width: 1025px) {
  .sidebar { width: 240px; }
}
```

---

### 8.7 Vista "Resumen de Turno" (Handoff)
**Ruta:** `/handoff` — **Archivo:** `dmart-app/src/pages/handoff.rs`

```rust
#[component]
pub fn HandoffPage() -> impl IntoView {
    let patients = api::list_patients(None, Some("activos")).await.unwrap_or_default();
    let sorted = patients.into_iter().sorted_by_key(|p| std::cmp::Reverse(p.ultimo_apache_score.unwrap_or(0))).collect_vec();
    view! {
        <div class="handoff">
            <header><h1>{tr("handoff-title", None)}</h1></header>
            <div class="handoff-meta">
                <span>{chrono::Local::now().format("%Y-%m-%d %H:%M").to_string()}</span>
                <span>Turno: {current_shift()}</span>
            </div>
            <table class="handoff-table">
                <thead><tr><th>Cama</th><th>Paciente</th><th>APACHE</th><th>GCS</th><th>Severidad</th><th>Últ. medición</th><th>Pendientes</th></tr></thead>
                <tbody>{sorted.into_iter().map(|p| view! {
                    <tr>
                        <td>{p.cama_numero.unwrap_or(0)}</td>
                        <td>{p.nombre_completo}</td>
                        <td>{p.ultimo_apache_score.unwrap_or(0)}</td>
                        <td>{p.ultimo_gcs_score.unwrap_or(0)}</td>
                        <td><SeverityBadge level=p.estado_gravedad/></td>
                        <td>{p.updated_at[..16].to_string()}</td>
                        <td>{pending_for(&p)}</td>
                    </tr>
                }).collect_view()}</tbody>
            </table>
            <button class="btn-primary" on:click=generate_handoff_pdf>{tr("handoff-generate", None)}</button>
        </div>
    }
}
```

---

### 8.8 Command Palette (Cmd/Ctrl+K)
**Archivo:** `dmart-app/src/components/command_palette.rs`

```rust
use leptos::prelude::*;

#[component]
pub fn CommandPalette() -> impl IntoView {
    let (open, set_open) = signal(false);
    let (query, set_query) = signal(String::new());
    let results = move || {
        let q = query.get().to_lowercase();
        if q.is_empty() { return vec![]; }
        // busca en pacientes, equipos, acciones
        vec![
            ("Nuevo paciente", "/patients/new"),
            ("Exportar CSV UCI", "/export/csv"),
            ("Configurar alarma", "/admin/alerts"),
        ].into_iter().filter(|(n,_)| n.to_lowercase().contains(&q)).collect()
    };
    view! {
        <div class=format!("command-palette {}", if open.get() { "open" } else { "" }) role="dialog">
            <input type="text" placeholder="Buscar… (Cmd+K)" prop:value=query on:input=move |ev| set_query.set(event_target_value(&ev))/>
            <ul>{move || results().into_iter().map(|(n, href)| view! {
                <li><a href=href>{n}</a></li>
            }).collect_view()}</ul>
        </div>
    }
}
// Global listener: document.addEventListener("keydown", e => { if (e.metaKey && e.key === "k") openPalette() })
```

---

### 8.8 PWA Enhancements
**Archivo:** `dmart-app/public/manifest.json` (actualizar)

```json
{
  "name": "dMart UCI",
  "short_name": "dMart",
  "start_url": "/",
  "display": "standalone",
  "background_color": "#0f172a",
  "theme_color": "#22d3ee",
  "icons": [...],
  "shortcuts": [
    { "name": "Nuevo paciente", "url": "/patients/new", "icons": [{ "src": "/icons/plus.png", "sizes": "192x192" }] },
    { "name": "Censo UCI", "url": "/patients?estado=activos", "icons": [{ "src": "/icons/bed.png", "sizes": "192x192" }] },
    { "name": "Alertas", "url": "/alerts", "icons": [{ "src": "/icons/bell.png", "sizes": "192x192" }] }
  ],
  "categories": ["medical", "health"]
}
```

**Background Sync** (mediciones offline):
```rust
// sw.js (service worker)
self.addEventListener('sync', event => {
  if (event.tag === 'sync-measurements') {
    event.waitUntil(syncPendingMeasurements());
  }
});
```

---

## 9. Reportes Automáticos Programados (Semana 2-3)
**Archivo:** `dmart-server/src/bin/scheduler.rs` (nuevo binary con `tokio-cron-scheduler`)

```rust
use tokio_cron_scheduler::{Job, JobScheduler};

pub async fn start_scheduler(db: Database) {
    let sched = JobScheduler::new().await.unwrap();
    
    // Censo diario 07:00
    sched.add(Job::new_async("0 0 7 * * *", move |_uuid, _l| {
        let db = db.clone();
        Box::pin(async move {
            let census = generate_census(&db).await;
            email::send_admin("Censo UCI diario", &census).await;
            save_report(&db, "censo-diario", &census).await;
        })
    }).unwrap()).await.unwrap();
    
    // Mortalidad semanal Lunes 08:00
    sched.add(Job::new_async("0 0 8 * * 1", move |_| {
        let db = db.clone();
        Box::pin(async move {
            let report = mortality_weekly(&db).await;
            email::send_medical_chief("Mortalidad semanal", &report).await;
        })
    }).unwrap()).await.unwrap();
    
    // Mantenimiento equipos diario 06:00
    sched.add(Job::new_async("0 0 6 * * *", move |_| {
        let db = db.clone();
        Box::pin(async move {
            let due = maintenance_due(&db).await;
            if !due.is_empty() { email::send_biomed("Mantenimiento vencido", &due).await; }
        })
    }).unwrap()).await.unwrap();
    
    sched.start().await.unwrap();
}
```

---

## Actualización de Esfuerzo (con UX/i18n)

| Item | Días | Riesgo |
|------|------|--------|
| CI/CD | 2 | Bajo |
| Tests unitarios db.rs | 3 | Bajo |
| Observabilidad (health/metrics/backup) | 3 | Medio |
| Docker + release binary | 2 | Bajo |
| API consistency | 2 | Medio |
| **i18n base (ES/EN) + tema + toast** | **3** | **Bajo** |
| **Responsive + shortcuts + a11y audit** | **3** | **Medio** |
| **Handoff + Command Palette + PWA** | **3** | **Medio** |
| Scheduler reportes automáticos | 2 | Medio |
| API consistency | 2 | Medio |
| Docs | 2 | Bajo |
| **Total** | **27 días** | |

---

## Priorización Recomendada (MVP UX en 1 semana)

| Día | Entregable |
|-----|------------|
| 1 | i18n base (ES/EN) + `fluent-templates` setup |
| 2 | Tema claro/oscuro + selector + CSS variables |
| 3 | Toast/Snackbar + alertas críticas |
| 4 | Atajos teclado (Alt+N/B/E/M/?) |
| 5 | Responsive tablet-first + a11y basics |
| 6 | Command Palette (Cmd+K) |
| 7 | Handoff view + PWA manifest + shortcuts |

---

## Notas
- i18n: empieza con ES/EN; PT/FR se añaden después (solo archivos `.ftl`).
- Tema: respeta `prefers-color-scheme` y `prefers-contrast` nativo.
- Toast: reutiliza para validaciones de formularios, exports, errores de red.
- Handoff: PDF usa el mismo `generate_pdf` del backend.
- Scheduler: binary separado (`cargo run --bin scheduler`), se ejecuta en contenedor aparte.