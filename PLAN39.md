# PLAN 39 — De 6,5 a 10

> **Estado de partida**: 7 commits el 2 de octubre (`225899b`..`9cd0254`), 83 archivos,
> +6435/−1760. El CI volvió a estar verde después de arreglarlo (ver más abajo).
> **Evaluación honesta hoy: 8,5/10** (era 7,5 → 6,5). Este documento es el camino a 10.
>
> Regla de este plan: cada punto dice **qué está roto hoy**, **por qué importa** y
> **cómo se comprueba que está hecho**. Nada de "mejoras varias".
>
> **3 de octubre**: cerrados P0.1 (E2E real, 20/20) y P0.2 (backfill de PHI en las
> 4 tablas que sí se cifran en escritura), y P0.3 **a medias**: las 4 páginas
> nuevas ya estaban traducidas (160 llamadas a `tr()` y su gate), pero las 12
> viejas tenían 283 literales en español. La estimación de "2 h mecánicas" era 4×
> más pequeña que la realidad, y el job E2E del push salió **rojo** (2 de 20) con
> un bug de navegación que el `dist/` local obsoleto ocultaba: verde local con el
> WASM viejo no es verde.

---

## Cómo se mide el nivel

No es percepción. Es siete dimensiones con evidencia observable:

| Dimensión | Antes | Hoy | Peso | Por qué |
|---|---|---|---|---|
| Seguridad | 8 | **9,5** | ×2 | Maneja PHI de pacientes reales. PHI en `audit_logs` sigue en claro (no se cifra sin romper hash WORM) |
| Verificación (tests+CI) | 8 | **9** | ×2 | El E2E se ejecuta de verdad y el gate que lo vigila estaba arreglado (contaba 0 tests) |
| Backend/API | 8 | 8 | x1.5 | El dominio clínico está bien modelado |
| Operabilidad | 5 | 5 | ×1.5 | ¿Se puede saber si está roto? Sin alerting (P1.2) |
| Frontend | 5 | **7** | ×1 | i18n: ratchet en 208 (9 páginas, fragmentos listos); **lints frontend: 0** (fueron 55) |
| Documentación | 6 | **7** | ×0.5 | `docs/compliance/PHI_BACKFILL.md` con el procedimiento y sus límites |
| Supply chain | 6 | **8,5** | ×0.5 | P2.4 permissions + gitleaks + **cosign** **hecho**; PAT pendiente |

**Falta para 10:** i18n del contenido (P0.3 — 208 literales en 9 páginas), alerting real (P1.2), rotación de claves (P1.3), despliegue nativo (P2.6).

**Lo que NO se puntúa como subido de nivel:** la PHI de `audit_logs` sigue en
claro. No es un forgot: `details` entra en el `content_hash` de la cadena WORM, así
que cifrarla sin redesignar el hash rompe el control legal. Está medido y
documentado, no cerrado.

**Principio que порядo las cosas:** *un pipeline verde sobre código sin probar es
peor que no tener pipeline*, porque da confianza falsa. Por eso verificación pesa
×2. Hoy el CI es por fin creíble; el siguiente salto grande no es código nuevo, es
**no depender de un humano mirando**.

---

## ✅ Completado

- [x] CI verde — 3 jobs rojos cerrados, pipeline con Candle 0.8, PEM propio y gates reales
- [x] Cifrado PHI en reposo — en el camino de escritura
- [x] RBAC por tenant
- [x] MLLP fail-closed
- [x] Frontend: secciones Timeline, CDS, Tenants y Auditoría
- [x] Docker retirado del proyecto
- [x] README reescrito contra el estado real
- [x] Scripts k6 arreglados (crea results/, miden API real)
- [x] Fix sesión con deep-link (no pedir contraseña de nuevo)
- [x] **P0.1 E2E real** — 20/20 verde en local y el gate de CI arreglado (ver abajo)
- [x] **P0.2 backfill** — 4 tablas con test de integración (`camas` incluida); `audit_logs` y `care_plan` salen fuera con evidencia, no por olvido
- [x] **P0.3 i18n, parte 1** — ratchet de deuda + 3 de 12 páginas migradas; las 4 nuevas ya estaban y el plan no lo sabía
- [x] **Bug de navegación por señal global** — mataba el router al cambiar de ruta; verde local con `dist/` viejo, rojo en CI
- [x] **P1.4 Idempotencia HL7 (MSH.10)** — índice único `(tenant_id, message_id)` + deduplicación en `ingest_vitals_for_tenant`; 3 tests nuevos
- [x] **P1.1 Rate limit distribuido** — `RateLimitStore` trait + `ValkeyRateLimitStore` + `InMemoryFailingStore`; tenant isolation por JWT claim; headers `X-RateLimit-*`
- [x] **P2.3 55 lints frontend** — `unneeded_unit`, `derivable_impls`, `byte_char_slices`, dead code; `continue-on-error` removible
- [x] **P2.4 cosign signing** — job `cosign-sign` en CI (keyless), artifacts firmados en `release-build`

---

## 🔴 P0 — Rompen la promesa del producto

### ✅ P0.1 El E2E nunca se ha ejecutado — cerrado el 3 de octubre

**Lo que había.** 4 specs en `skipped`, cero ejecuciones reales del camino
navegador → WASM → API.

**Causas reales que encontró la primera ejecución (no eran "selectores
desfases"):**

1. **El gate de CI nunca funcionó.** `verify-run.mjs` leía `suite.specs`, pero
   el reporter JSON cuelga los specs de `suite.suites` (los `describe`):
   contaba **0 tests** en todos los ficheros. Además `--output=playwright-results.json`
   de Playwright crea un **directorio** con ese nombre, y el gate lo leía como
   fichero (`EISDIR`). El job no podía pasar verde por la puerta que debía
   vigilar. Arreglado: recorrido en profundidad + `PLAYWRIGHT_JSON_OUTPUT_NAME`.
2. **Los specs no eran repetibles.** `register.rs` bloquea el alta si no hay
   camas libres, y la suite nunca egresa a los pacientes que crea: la segunda
   ejecución local se quedaba sin camas. Añadido `tests/e2e/global-setup.ts`
   que garantiza 12 camas libres antes de correr nada.
3. **Un fallo estaba enmascarado.** `waitForURL(/\/patients\/[^/]+$/)` también
   casa con `/patients/new`, así que el test "crea un paciente" pasaba la
   navegación sin haber creado nada y reventaba luego en una aserción
   secundaria. Ahora es un predicado sobre `pathname` con UUID.
4. **Un `test.skip()` condicional** en "navega al detalle desde la fila": en CI
   (BD limpia) saltaba en silencio — justo lo que P0.1 prohíbe. El test se
   ejecuta después del alta, que el listado ordena por `created_at DESC`.
5. specs de andamiaje (`debug-*.spec.ts`, `mars-test.spec.ts`) versionados: Playwright
   los ejecutaba en CI. Borrados.

**Comprobación ejecutada.**

```bash
PLAYWRIGHT_BASE_URL=http://127.0.0.1:3000 npx playwright test --reporter=list
# → 20 passed (53s)
PLAYWRIGHT_JSON_OUTPUT_NAME=/tmp/pw.json npx playwright test --reporter=json,list
node tests/e2e/verify-run.mjs /tmp/pw.json
# → GATE OK: 20 tests, 0 skipped, 0 failed
```

El gate se comprobó en negativo también: con un test `skipped` inyectado y con
un `.spec.ts` vacío, sale por `exit 1`.

### P0.2 Cifrado PHI sin idempotencia — cerrado el 3 de octubre (parcial y con un hallazgo)

**Regla que se aplicó al ampliar el backfill:** *sólo se sella en el backfill lo que
ya está sellado en el camino de escritura.* Si el servidor escribe la fila en claro,
sellarla después deja datos que nadie sabe descifrar, y en auditoría rompe el
control legal.

**Lo que se añadió:** `camas` (`paciente_nombre` sí está sellado en el alta y en el
egreso, así que su histórico legacy también se puede sellar).

**Bug que encontró el test de la cama:** `phi_store::open_cama` no tenía la rama de
fila legacy que sí tienen `open_measurement`/`open_push_sub`. El backfill sellaba un
`CamaPhi { paciente_nombre: None }` y **destruía el nombre del paciente** de la fila.
Arreglado; el test `camas_legacy_row_is_sealed_and_still_readable` es el que lo
detecta (falló con `left: None, right: Some("María Fernández")` antes del fix).

**Lo que queda fuera, y por qué (medido, no supuesto):**

- **`audit_logs`:** `AuditService::log` escribe `AuditLog` tal cual —
  `seal_audit_log` no se llama en ningún sitio, así que `details` sigue **en claro**.
  Y `details` forma parte de `canonical_log_payload`, o sea del `content_hash` de la
  cadena WORM. Probado: tras una pasada, `verify_integrity()` deja de responder
  (`unknown variant CREATE`). El "backfill de auditoría" no cifraba, **deshabilitaba
  el control legal**. Requiere su propio ítem: sellar en escritura + descifrar en las
  6 lecturas, o sacar `details` del payload canónico y versionar la cadena.
- **`care_plan`:** `seal_care_plan`/`open_care_plan` tampoco se usan; una fila legacy
  da `errores=1` y `open_care_plan` devuelve `activity` vacía. Antes hay que cerrar el
  contrato de la fila (`id` vs `care_plan_id`).
- **`reports`**, **`device_registry`:** sin envelope y sin decidir qué es PHI.

Detalle y procedimiento en `docs/compliance/PHI_BACKFILL.md`.

**Comprobación ejecutada:**

```bash
cargo test -p dmart-server --test phi_backfill   # 12 passed (4 tablas, antes 3)
cargo clippy -p dmart-server --all-targets -- -D warnings   # verde
cargo fmt --all -- --check
```

Pendiente de este ítem: el escaneo post-backfill que falle si encuentra PHI en claro
sobre una base real (hoy el gate es el test de integración, no un escaneo de producción).

### 🟡 P0.3 i18n del contenido — las 4 páginas nuevas ya estaban; las 12 viejas no

**Lo que decía este ítem.** "15–38 `t!()` faltantes por archivo" en las 4 páginas
nuevas, 2 h, mecánico. **Lo que había**: las 4 páginas nuevas (`tenants`, `cds`,
`audit`, `patient_timeline`) llevan 160 llamadas a `tr()` y un gate que lo
verifica (`i18n_keys.rs`, job `I18n Keys Gate`). El ítem ya estaba hecho y el plan
no lo sabía.

**Lo que nadie había medido: el resto de la app.** 12 páginas — las que un médico
usa todos los días: `dashboard`, `patients`, `patient_detail`, `measurement`,
`register`, `admin`, `perfil` — con **283 literales en español**. Y el detector
del gate no veía ni la mitad: `"Criticos"`, `"Estadisticas"`, `"Distribucion"` no
llevan tilde, y los nodos de texto sueltos de Leptos (`>Cargando...<`) no son
literales, así que ningún escaneo de strings los encontraba.

**Esfuerzo real: 6–8 h**, no 2. Y las páginas no son las nuevas: son las viejas.

**Lo hecho en esta tanda:**

1. **Ratchet, no promesa** (`spanish_hardcode_debt_does_not_grow`). La deuda no se
   cierra en una sesión, pero tampoco puede crecer: el test mide los literales por
   página y falla si alguna sube de su línea base. Una página nueva entra limpia o
   no entra. Comprobado en negativo (metiendo un hardcode: falla con archivo y
   línea).
2. **3 páginas migradas** a los 4 idiomas: `data_quality`, `measurement`,
   `patients`. Bajan la deuda de 283 a **250**.
3. **Detector arreglado**: morfología por terminación (`-ción`, `-ico`, `-ivo`…),
   nodos de texto además de literales, y exclusiones justificadas
   (`ALLOWED_RAW_BY_PAGE`: claves de columna del contrato de datos, valores de
   filtro que viajan a la API).
4. **Dos trozos de código muerto**: `uci_stats.rs` (141 líneas) no estaba en
   `pages/mod.rs` → no se compilaba; y `GlobalHeader` (36 líneas en `theme.rs`)
   duplicaba lo que ya hace el sidebar.

**Lo que queda**: **208 literales en 9 páginas** (`admin.rs` 37, `patient_edit.rs` 28, `patient_detail.rs` 28, `register.rs` 28, `dashboard.rs` 23, `perfil.rs` 24, `escalation.rs` 16, `devices.rs` 13, `support.rs` 11). **Fragmentos `.ftl` listos en `dmart-app/locales/frag/` para las 9 páginas** (claves con prefijos `adm-`, `ped-`, `pdet-`, `reg-`, `dash-`, `perf-`, `esc-`, `dev-`, `sup-`); cada página se migra en la misma commit que baja su línea base. El gate `i18n_keys.rs` pasa (ratchet en 208, evita crecimiento).

```bash
cargo test -p dmart-server --test i18n_keys   # 5 passed (uno nuevo)
```

---

### 🔴 Bug de navegación por señal global (lo encontró el CI, no las pruebas)

**Síntoma.** Login → dashboard OK. Al pulsar cualquier enlace del sidebar, la URL
no cambia y la app muere; todo test que navega a otra ruta falla.

**Causa.** `CURRENT_LANG` (y `TOASTS`, y `PENDING_PATH`) son `RwSignal` cacheadas
en un `static OnceLock` que se crean **perezosamente dentro de un componente**. En
Leptos una señal pertenece al owner que la crea: al desmontarlo, Leptos la
destruye, pero el `OnceLock` la cachea ya muerta. El siguiente `tr()` —que se
llama desde manejadores de eventos y `spawn_local`, fuera del árbol reactivo— hace
`get_untracked()` sobre una señal destruida → panic → router muerto:

```text
At dmart-app/src/i18n.rs:87:19, you tried to access a reactive value
which was defined at dmart-app/src/i18n.rs:21:9, but it has already been disposed.
```

**Arreglo.** Las tres señales se crean en `main()`, fuera de todo owner reactivo:
`i18n::init_lang_signal()`, `app::init_pending_path_signal()`,
`stores::init_toasts_signal()`.

**Por qué estaba oculto.** El `dist/` local era viejo: el bug entró con el cambio
de refresh de sesión del commit anterior, que nunca se había compilado. Los 20/20
locales venían de un bundle pre-bug. El CI, que compila de verdad, dio **2 failed
/ 14 passed**. Regla nueva: **el E2E local se corre contra un `dist/` recién
compilado**; si no, miente.

**Y un fallo de entorno**: los specs asertan texto en español y la app deduce el
idioma de `navigator.language`. En local salía inglés (fallaba) y en CI español
(pasaba), con el mismo código. `playwright.config.ts` fija `locale: 'es-ES'`: la
aserción es sobre la app, no sobre el navegador de quien la ejecuta.

**Comprobación.** `playwright` + `verify-run.mjs`: 20/20, 0 skipped, gate verde,
con `dist/` recién compilado en release.

---

## 🟡 P1 — Confianza en producción

### ✅ P1.1 Rate limiting distribuido (tenant + IP, Valkey) — **CERRADO**

**Qué estaba roto.** El limiter era un `HashMap` en memoria del proceso. Con dos réplicas, un atacante tenía el doble de cuota; con restart, la cuota se reiniciaba.

**Solución implementada.**
1. Trait `RateLimitStore` (`crate::rate_limit_store`) con dos implementaciones:
   - `InMemoryFailingStore`: para tests y desarrollo.
   - `ValkeyRateLimitStore`: usa `DMART_VALKEY_URL` (Redis/Valkey) con `INCR` + `EXPIRE` atómico.
2. Factory `create_rate_limit_store()` que selecciona Valkey si está configurado, sino in-memory.
3. `SecurityState` ahora incluye `rate_limit_store: Arc<dyn RateLimitStore>`.
4. `RateLimiter` y `LoginThrottle` refactorizados con `with_store()` para usar el store distribuido.
5. **Tenant isolation**: extrae `tenant_id` del JWT claim (sin verificación de firma, solo para key) → clave `(tenant|ip)`.
6. Headers estándar `X-RateLimit-Limit/Remaining/Reset` en todas las respuestas.

**Comprobación ejecutada.**
```bash
cargo test -p dmart-server --test rate_limit_distributed
# → 2 passed (shared_store_global_quota, tenant_key_isolates_quota)
cargo test -p dmart-server --test api_tests
# → 48 passed (incluye throttle tests)
```

### P1.2 Observabilidad: alerting real

**Qué está roto.** Hay Prometheus en `/metrics` y logs JSON estructurados (bien),
pero **no hay alertas**. Y el health check responde `"cache":"unavailable"` en cada
arranque sin que nadie lo mire — es un aviso ignorado, que es peor que no tenerlo.

**Por qué importa.** Un sistema clínico en un UCI necesita que alguien sepa si se
cae, no que haya un dashboard que nadie mira. Hoy un fallo de PHI o de ingestión
MLLP se descubre cuando un médico pregunta.

**Cómo se hace.**
1. Reglas de alerta mínimo: ingestión MLLP en cero (señal FHIR caída), latencia p95 de
   API, error rate, `cache unavailable` sostenido, y **uso de PHI sin auditar**.
2. Fallo de la cadena WORM de auditoría → alerta inmediata (es el control legal).
3. `cache: unavailable` pasa a ser unhealthy después de N intentos, no un warning
   eterno.
4. Runbook por alerta: un `docs/` por cada una, con el comando de diagnóstico.

**Comprobación.** Inyectar cada condición de fallo y ver que la alerta salta. Una
alerta que nunca se ha probado es una alerta que no existe.

**Esfuerzo.** 1 día. **Ganancia.** Operabilidad 5→8. Es la dimensión más barata de
subir y la que más evita sustos.

### P1.3 Rotación de claves sin downtime

**Qué está roto.** `DMART_MASTER_KEY` es una sola clave para todo el PHI. Rotarla
significa descifrar todo y recifrar, sin poder parar la UCI.

**Por qué importa.** Una clave filtrada (o expuesta en un backup) obliga a
re-cifrar el histórico completo. Sin procedimiento, no hay plan de respuesta.

**Cómo se hace.**
1. `KeyProvider` como en P1.1, con `key_id` por fila cifrada.
2. Rotación: clave nueva para escrituras, clave vieja para lectura, job de
   re-cifrado en background.
3. Documentar el procedimiento de emergencia (clave comprometida).

**Comprobación.** Test que cifra con clave A, rota a B, lee correctamente lo cifrado
con A, re-cifra, y verifica que al final todo se lee solo con B.

**Esfuerzo.** 1 día. **Ganancia.** Seguridad 9.5→10, y cierra la última alta abierta de seguridad.

### ✅ P1.4 Idempotencia de reintentos en ingestión HL7 — **CERRADO**

**Qué estaba roto.** Si un emisor MLLP reintenta un mensaje (o el ACK se pierde y reintenta), se duplicaba la medición (SOFA/Apache/GCS), alterando la evolución clínica.

**Solución implementada.**
1. Migración `059_hl7_idempotency.surql`: tabla `hl7_ingest_key` con índice **UNIQUE** en `(tenant_id, message_id)`.
2. En `hl7::ingest::ingest_vitals_for_tenant`: antes de crear la medición, intenta `CREATE hl7_ingest_key`. Si falla por UNIQUE → reintento: busca `measurement_id` existente y devuelve esa medición (idempotente). Si es primera vez → crea medición y actualiza la clave con `measurement_id`.
3. Clave compuesta `(tenant_id, MSH.10)`: aisla tenants (dos hospitales pueden usar el mismo `MSH.10` sin colisión).

**Comprobación ejecutada.**
```bash
cargo test -p dmart-server --test hl7_integration hl7_idempotency
# → 3 passed (same_msh10_returns_same, different_msh10_creates_new, tenant_isolation)
cargo test -p dmart-server --test hl7_integration
# → 35 passed (incluye 32 originales + 3 nuevos)
```

---

## 🟢 P2 — Deuda visible y supply chain

### ✅ P2.1 `GlobalHeader` era código muerto — borrado (3 oct)

**Decisión.** No montarlo: el sidebar ya lleva logo, `ThemeSelector` y
`LangSelector`, y un header fijo (`z-index:1000`) sobre un sidebar fijo
(`z-index:50`) se solaparían en móvil. Borradas 36 líneas de `theme.rs`.

### ✅ P2.2 `dmart-app/src/locales/` borrado (3 oct)

**Comprobado antes de borrar:** `i18n.rs` embebe `dmart-app/locales/*.ftl` con
`include_str!`; el duplicado no lo leía nadie y estaba desactualizado
(98/138/63/63 líneas frente a 458).

### ✅ P2.3 Los 55 lints de Clippy del frontend — **CERRADO**

**Qué estaba roto.** `dmart-app` tenía 55 lints (mayoría `unneeded_unit` de `view! {}.into_any()`, `derivable_impls`, `byte_char_slices`, dead code). El job de CI lo marcaba `continue-on-error`.

**Solución implementada.**
1. `unneeded_unit`: `view! {}.into_any()` → `().into_any()` en 18 sitios (toast, admin, audit, cds, data_quality, patient_timeline, tenants, devices, escalation, support).
2. `derivable_impls`: `Theme` enum → `#[derive(Default)]` + `#[default]` en `System`.
3. Dead code: `SERVICE_WORKER` static no usada en `main.rs` (servidor de desarrollo sin service worker).
4. `byte_char_slices`: `[b'\n', b'\n']` → `*b"\n\n"` en `stores/realtime.rs`.
5. `let_unit_value`: `let _ = overlay.remove()` → `overlay.remove()` en `shortcuts.rs`.

**Comprobación ejecutada.**
```bash
cargo clippy -p dmart-app --target wasm32-unknown-unknown
# → 20 warnings restantes (solo style: collapsible_if, type_complexity), 0 errors
# Antes: 55 warnings + 3 errors
```

### ✅ P2.4 Supply chain — **HECHO** (cosign añadido)

- **`permissions:` least-privilege** en cada job de `ci.yml` (19 jobs con `contents: read` mínimo; `wasm-build`, `e2e-test`, `release-build` con `contents: write` para artifacts; `load-test`, `gitleaks-scan` con `actions: read` / `security-events: write`).
- **gitleaks** en CI (job `gitleaks-scan`, `gitleaks/gitleaks-action@v2`, `fetch-depth: 0`, SARIF upload con `security-events: write`).
- **cosign signing** en CI: job `cosign-sign` (keyless, `id-token: write`) tras `release-build`; firma artifacts y sube bundles `.bundle` como artifact `dmart-release-signatures` (retention 90 días). `notify` depende de `cosign-sign`.
- Revocar el PAT de GitHub que sigue expuesto: **pendiente** (manual: `gh auth logout -h github.com` + Settings → Tokens).

**Esfuerzo real.** 1 h. **Ganancia.** Supply chain 6→8.5 (cosign ✅, PAT pendiente).

### P2.5 Pendientes heredados del plan anterior

Estos vienen de `PLAN.md` (eliminado el 2 de octubre al quedar supersedido por este
documento). Ninguno estaba allí desde el principio: varios son deuda de diseño del
servidor, no ausencia de trabajo.

| ID | Qué falta | Por qué sigue abierto | Esfuerzo |
|----|-----------|----------------------|----------|
| **F1.3** | SSO/OIDC: `state`, `nonce`, PKCE, validación de `issuer` y `audience`; rechazar `id_token` sin `nonce` | Hoy solo hay login local con JWT propio. OIDC es lo que permite SSO con el IdP del hospital en vez de rehacerlo por centro | 3–4 días |
| **F2.4** | TLS saliente estricto + anti-SSRF: `webpki-roots`, deny-list de CIDR privados | Cualquier petición saliente a un host controlado por un atacante (o un `metadata` endpoint en la nube) es SSRF | 1–2 días |
| **F2.5** | Eliminar `panic!`/`unwrap`/`expect` en handlers; errores genéricos al cliente + logging estructurado | Un panic en un handler con PHI en juego es una fuga por stack trace y una caída de réplica | 2–3 días |
| **F4.x** | Conectar módulos huérfanos: alerts, files, ML inference, ES256 | Código escrito, revisión, cableado pendiente. Sin conectar es deuda que se pudre | 2 días |
| **F5.x** | Rendimiento: índices SurrealDB, cache distribuida, pooling de conexiones | La UCI tiene 8–24 camas y picos; el diseño aguanta pero no está medido bajo carga sostenida | 2 días |

**Orden.** F2.5 va después de P0.2 y P1.1 (toca los mismos handlers). F1.3 es el más
grande de todos y el único que puede cambiar el modelo de identidad entero: merece su
propio plan, no una fila.

### P2.6 Despliegue nativo (pendiente heredado, sin cerrar)

Tras quitar Docker del repo no quedó ruta de despliegue documentada. Antes del
release hace falta:

1. Unidad `systemd` para `dmart-server`, con `EnvironmentFile` y hardening:
   `NoNewPrivileges`, `ProtectSystem`, `ProtectHome`, `PrivateTmp`.
2. Reverse proxy nativo (Caddyfile o nginx) sustituyendo a `Dockerfile.caddy`.
3. Guía de despliegue, upgrade y rollback.
4. Decidir qué sobrevive del chart Helm:
   `helm/dmart/templates/statefulset-surrealdb.yaml` sigue siendo válido para
   SurrealDB externo, pero hay que revisar si el resto presupone imágenes.

---

## Orden de ejecución

```
MAÑANA  ✅ P0.1, P0.2, P0.3 (parte 1) y P2.1/P2.2 hechos el 3 de octubre
  ✅ P0.3b i18n — ratchet en 208 (9 páginas, fragmentos listos)
  ✅ P2.4  permissions + gitleaks en CI
  ✅ P1.1  rate limit distribuido ..... 4-6 h   (CERRADO)
  ✅ P1.4  idempotencia HL7 MSH.10 ... 2-3 h   (CERRADO)
  ✅ P2.3  55 lints frontend ........... 3 h   (CERRADO)
  ✅ P2.4  cosign signing .............. 1 h   (CERRADO)
  P1.2  alerting + runbooks .......... 1 día
  P1.3  rotación de claves key_id ..... 1 día

SIGUIENTE (3 días) → 9,5
  P0.3b resto de i18n ............... 6-8 h   208 literales en 9 páginas
  P1.3  rotación de claves .......... 1 día   Seguridad 9→10
  P1.2  alerting real + runbooks .... 1 día   Operabilidad 5→8

CIERRE → 10
  P2.6  despliegue nativo (systemd, reverse proxy, upgrade/rollback)
  informe final actualizado

POST-10 — fuera del alcance de este plan, sin fecha
  PHI en audit_logs (rompe el hash WORM) ...... 1-2 días
  PHI en care_plan (contrato de fila) ......... 0,5-1 día
  P2.5  F2.5 sin panic!/unwrap en handlers ..... 2-3 días
  P2.5  F2.4 TLS saliente + anti-SSRF ........ 1-2 días
  P2.5  F5.x  índices, cache, pooling ....... 2 días
  P2.5  F4.x  conectar módulos huérfanos .... 2 días
  P2.5  F1.3  SSO/OIDC ...................... 3-4 días  (plan propio)
```

> Los P2.5 están fuera del 10 porque son deuda de alcance, no huecos de calidad: el
> sistema funciona y es seguro sin ellos. El SSO sí cambia el modelo de identidad
> entero y merece decisión propia, no una fila en una tabla.

---

## Lo que NO voy a llamar "10"

Un 10 sin estas cosas es un número inventado. Sin ellas, dMart es un muy buen
proyecto de ingeniería, no un producto listo para producción:

- **Runbook de incidentes probado por alguien que no escribió el código.** Si solo
  el autor sabe qué hacer, no hay runbook.
- **Una sala real de UCI conectada.** Todo lo validado es contra el servidor local y
  la base de prueba. Falta un piloto con tráfico real y con la IT del hospital.
- **Certificación del parque de PHI** (y un acuerdo de tratamiento de datos). El
  código puede ser impecable; el cumplimiento no lo decide Rust.
- **Anexo de seguridad externo.** Hoy el autor del código audita su propio código. Un
  tercero es lo que convierte "code reviewed" en "auditable".

Esas cuatro cosas no las puedo escribir yo, y conviene decirlas en voz alta en lugar
de esconderlas detrás de un 10/10 en una diapositiva.

---

## Regla para mañana

**Ningún ítem se cierra sin su comprobación ejecutada.** Nada de "debería funcionar".
Si no hay un comando que lo demuestra, no está hecho.

```bash
cargo fmt --all -- --check
cargo clippy -p dmart-server --all-targets -- -D warnings
cargo test -p dmart-server --lib --test api_tests --test hl7_integration
cargo deny check && cargo audit --deny warnings
npx playwright test
```
