# updateIO.md — Plan de actualización de Interfaz y Operación (dMart UCI)

> Objetivo: dejar el sistema **listo para hoy**, corrigiendo el bug de registro de
> personal y las mejoras de UX/operación imprescindibles, sin romper nada.
> Alcance aprobado: **"Solo imprescindible hoy"**. Single-tenant (sin UI de tenants).
> Arranque persistente: **servicio systemd de usuario** (sin sudo).
> Commits: **automáticos por fase** (conventional commits).

---

## 1) Bug de personal (diagnóstico confirmado)

**Causa raíz (backend):** `list_staff`, `list_staff_paginated` y `count_staff` en
`dmart-server/src/db.rs` filtran:

```sql
WHERE rol = 'Medico' OR rol = 'Enfermero'
```

Por eso **Admin y Viewer nunca aparecen** aunque sí se creen.
Evidencia: `/api/admin/stats` devuelve `total_staff: 3` pero el listado solo
muestra 1 (`FIGUERA`, Medico) → hay 2 usuarios ocultos.

**"Viewer no deja registrar":** es RBAC correcto, no bug. `POST /admin/staff`
exige `users:create` (`dmart-server/src/rbac.rs:257`) y Viewer no lo tiene → 403.
Falta que la UI sea consciente del rol (ocultar/deshabilitar y explicar).

**"No puedo registrar Enfermero":** a reproducir. El alta no restringe rol;
sospecha de validación de contraseña (≥8) o de que el usuario sí se creó pero no
se ve (mismo bug del filtro). Se valida con test en `api_tests`.

---

## 2) Plan de ejecución — "Imprescindible hoy"

### Paso 0 — Preparación
- Backup de `data/dmart.db` → `/tmp/opencode/`.
- Verificar instancia única de `dmart-server` (evitar `Invalid revision`).
- Baseline: `cargo test -p dmart-server --test api_tests`.

### Paso 1 — Backend
- `dmart-server/src/db.rs`: `list_staff`, `list_staff_paginated`, `count_staff`
  → devolver **todos los roles** + filtro opcional `rol`.
- `dmart-server/src/api/admin.rs`: `GET /admin/staff?rol=`.
- `dmart-server/src/api/auth.rs` + `auth.rs`: nuevo `POST /auth/change-password`
  (verifica actual, ≥8, hashea, revoca sesiones).
- Test en `api_tests`: listado por rol + alta de Enfermero + cambio de contraseña.

### Paso 2 — Frontend (`dmart-app`)
- Login: `GET /auth/me` → guardar rol/usuario + **gating por permisos**
  (Viewer sin acciones de Admin).
- `pages/admin.rs`: columna + filtro de rol, validaciones y errores claros.
- Nueva `pages/perfil.rs` (`/perfil`): cambio de contraseña + MFA; enlace en
  sidebar (`app.rs`).
- `api.rs`: `change_password`, `logout`; auto-refresh con `/auth/refresh` y
  redirección a `/login` al expirar (el token dura 1 h).

### Paso 3 — Datos de demo
- `POST /sandbox/generate` + camas/equipos por API.
- **Nunca** usar `/sandbox/clear` (destructivo). Conservar datos reales.

### Paso 4 — Arranque persistente
- Unit systemd de usuario para `target/release/dmart-server` (+ backup),
  `enable`/`start`, sin sudo.

### Paso 5 — Verificación y cierre
- Tests `api_tests`, clippy (server + wasm), build release + `trunk build --release`, restart.
- Smoke test: login, lista activos/egresados/todos, egreso, staff
  (Admin/Enfermero/Viewer visibles), Mi Perfil, logout, expiración.
- Commits conventional por bloque.

---

## 3) Faltantes críticos incorporados al plan

1. **Sesión que no se caiga.** El token dura 1 h y la app no refresca: guarda
   solo `dmart_auth` y no usa `/auth/refresh` (hay cookie `refresh_token`).
   → auto-refresh silencioso + redirección a `/login` si expira.
2. **Logout real.** Hoy solo borra `localStorage`; no llama a `POST /auth/logout`
   ni revoca el token.
3. **Arranque persistente + backup/rollback.** No hay servicio systemd para
   `dmart-server` (el timer existente es de backup vía Docker y no aplica a la
   ejecución local); `keep-alive.sh` existe pero no está instalado.
4. **Datos de demo.** Solo hay 1 paciente (Roberto, egresado) y 3 usuarios.
   → `/sandbox/generate` + camas/equipos para que la demo no se vea vacía.
5. Menores rápidos: validación en formularios (contraseña ≥8, usuario duplicado),
   mensajes claros de 400/401/403, y mostrar estado de salud (`/obs/health`).

---

## 4) Fuera de alcance hoy (backlog documentado)

- **MFA en "Mi Perfil":** el backend (`/auth/mfa/*`) existe, pero el login web no
  implementa el reto TOTP. Activar MFA desde la UI dejaría al usuario sin poder
  iniciar sesión por el navegador, así que se posterga hasta cablear el reto en
  `login.rs`. El cambio de contraseña sí quedó operativo.
- Dispositivos / Monitores (`/devices`).
- Calidad de datos (`/data-quality`).
- Alertas / Escalamiento (`/escalation`).
- Tele-ICU (`/teleicu`) y ML / similaridad.
- Dashboard 2.0, command palette (Ctrl+K) y rediseño visual.
- Auditoría HIPAA (pestaña `/admin/audit`) y cableado de `uci_stats`:
  **solo si sobra tiempo**.

---

## 5) Guardarraíles (no dañar nada)

- Cambios **aditivos**; no tocar el esquema de `data/dmart.db`.
- Una sola instancia de `dmart-server` a la vez.
- No romper contratos (`PatientListItem`, etc.).
- Verificación por fase: `cargo test -p dmart-server --test api_tests`,
  clippy (server + wasm), `trunk build --release`, restart y prueba manual.

---

## 6) Estado / decisiones

- [x] Backend: fix vista de personal (Admin/Viewer) y alta de Enfermero verificado.
- [x] Backend: `POST /auth/change-password`.
- [x] Frontend: gating por rol + "Mi Perfil" (`perfil.rs` solo cambio de contraseña; MFA a backlog).
- [x] Frontend: auto-refresh de sesión + logout real.
- [x] Datos de demo generados.
- [x] Servicio systemd de usuario habilitado.
- [x] Smoke test + commits por fase.

### Reanudación tras corte de luz (2026-09-17)

El corte dejó el Paso 2 a medias: `app.rs`/`api.rs`/`stores` ya referenciaban
`pages/perfil.rs` (inexistente) y `admin.rs` con llaves desbalanceadas, por lo
que el frontend no compilaba. Se completó y verificó:

- `dmart-app/src/pages/perfil.rs` creado y registrado en `pages/mod.rs`.
- `admin.rs`: corregido `StaffPanel` (llave de cierre del bloque de filtro) y
  restaurado `CamasPanel`.
- `gloo-net` 0.7 no reexporta `RequestCredentials` → se usa `web_sys`
  (feature `RequestCredentials` añadida a `dmart-app/Cargo.toml`).
- Verificación: `trunk build --release` OK; `cargo test -p dmart-server --test
  api_tests` 35/35; `--test hl7_integration` 32/32; `--lib` 85/85; clippy server
  y wasm sin errores.
- Arranque persistente: unidades de usuario
  `~/.config/systemd/user/dmart-server.service` (Restart=always) y
  `dmart-backup.{service,timer}` (diario 02:00, salida en `data/backups/`).
  `loginctl enable-linger tdy` → `Linger=yes` (arranca sin login, sin sudo).
- Smoke: `/obs/health` 200, `/` 200, asset wasm 200, `/api/auth/me` sin token
  401. (Login interactivo no automatizado para no bloquear cuentas por el
  throttle; la lógica auth/RBAC está cubierta por `api_tests`.)
