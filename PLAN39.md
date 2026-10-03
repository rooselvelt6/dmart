# PLAN 39 — De 6,5 a 10

> **Estado de partida**: 7 commits el 2 de octubre (`225899b`..`9cd0254`), 83 archivos,
> +6435/−1760. El CI volvió a estar verde después de arreglarlo (ver más abajo).
> **Evaluación honesta hoy: 6,5/10.** Este documento es el camino a 10.
>
> Regla de este plan: cada punto dice **qué está roto hoy**, **por qué importa** y
> **cómo se comprueba que está hecho**. Nada de "mejoras varias".

---

## Cómo se mide el nivel

No es percepción. Es siete dimensiones con evidencia observable:

| Dimensión | Hoy | Peso | Por qué |
|---|---|---|---|
| Seguridad | 8 | ×2 | Maneja PHI de pacientes reales |
| Verificación (tests+CI) | 8 | ×2 | Sin esto, todo lo demás es fe |
| Backend/API | 8 | x1.5 | El dominio clínico está bien modelado |
| Operabilidad | 5 | ×1.5 | ¿Se puede saber si está roto? |
| Frontend | 5 | ×1 | Lo que ve el médico |
| Documentación | 6 | ×0.5 | |
| Supply chain | 6 | ×0.5 | |

**Falta para 10:** cerrar el hueco navegador→API (E2E), idempotencia de PHI,
rate limit distribuido, observabilidad real, e i18n del contenido.

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

---

## 🔴 P0 — Rompen la promesa del producto

### P0.1 El E2E nunca se ha ejecutado — el hueco navegador→API

**Qué está roto.** Los 4 specs Playwright (`login`, `patients`, `measurements`,
`admin`) llevan días en `skipped`. No hay un solo registro de que la UI funcione
contra el backend. Todo lo que verifiqué hoy fue por HTTP directo (curl, k6): el
camino real que usa el médico —navegador → WASM → API— no está probado ni una vez.

**Por qué importa.** Es exactamente el hueco de mi evaluación: backend
validado, frontend nunca ejecutado. Si el WASM no monta una ruta, si un `t!()` falta,
si el token no viaja bien, **el E2E es lo único que lo detecta** y ahora no lo
detecta nadie.

**Cómo se hace.**
1. Levantar el stack completo en local: `trunk serve` + server con `DMART_DIST_PATH`.
2. Correr `npx playwright test` y triage de cada fallo.
3. Que el job del CI **no pueda volver a saltarse**: si el spec no corre, el job
   falla. Un `skipped` silencioso es peor que un rojo.

**Comprobación.** El job termina `success` con ≥1 spec ejecutado por archivo, y su
log lista los tests por nombre.

**Esfuerzo.** 1–2 h. **Ganancia de nivel.** Es el salto de 6,5 a 7 más barato que
existe: no escribes código, dejas de tener un agujero.

### P0.2 Cifrado PHI sin idempotencia (backfill incompleto)

**Qué está roto.** `backfill_phi_patients` existe y es idempotente para `patients`,
pero el resto de tablas con PHI (`care_plan`, `audit`, `push`, `device_registry`,
`reports`, `measurements`) se cifró **en el camino de escritura**. Filas legacy ya
en disco siguen en claro.

**Por qué importa.** El control de PHI se anunció como completo, y en una base con
histórico real no lo está. Un escaneo de PHI en claro devuelve datos de pacientes.
Es el mayor riesgo de cumplimiento que queda abierto.

**Cómo se hace.**
1. Extender el binario de backfill a cada tabla (mismo patrón `dry-run` + métricas).
2. Ejecutar en dry-run y **revisar el reporte** antes de aplicar: cuántas filas, qué
   timestamps.
3. Verificación post-backfill: escaneo que falle si encuentra PHI en claro.
4. Documentar el procedimiento de upgrade en `docs/`.

**Comprobación.** Un test de integración que siembre una fila legacy en claro, corra
el backfill y verifique que (a) queda cifrada, (b) el segundo run es no-op, (c) los
índices ciegos siguen funcionando para búsqueda exacta.

**Esfuerzo.** 3–4 h. **Ganancia.** Seguridad 8→9. Esto es lo que separa "dice que
cifra" de "cifra".

### P0.3 i18n: el contenido de las páginas nuevas está en español

**Qué está roto.** La navegación se traduce (ES/EN/PT/FR completos), pero el
**contenido** de las 4 páginas nuevas está hardcodeado en español: 15–38 `t!()`
faltantes por archivo. El `locale` del usuario cambia el menú y no el contenido.

**Por qué importa.** Es visible al minuto de usar la app y rompe la promesa de las
4 linguas. Un hospital no italiano recibe la interfaz en italiano con todo el cuerpo
clínico en español.

**Cómo se hace.** Un `t!()` por string visible, con claves en
`dmart-app/locales/{es,en,pt,fr}.ftl`. Los términos clínicos (APACHE, SOFA, NEWS2,
SAPS III) **no se traducen**: son nombres de escala clínica registrada.

**Comprobación.** Un test que itere los 4 locales y falle si ninguna clave falta en
ninguno. Sin ese test, el siguiente hardcodeo reintroduce el bug.

**Esfuerzo.** 2 h (mecánico). **Ganancia.** Frontend 5→7.

---

## 🟡 P1 — Confianza en producción

### P1.1 Rate limiting por tenant, no por IP

**Qué está roto.** El limiter es un `HashMap<String, Vec<Instant>>` **en memoria
del proceso**. Con dos réplicas, un atacante tiene el doble de cuota; con un
restart, la cuota se reinicia. Hoy son 100 rpm por IP leídas de
`DMART_RATE_LIMIT_RPM` (lo parametrizé ayer), pero sigue siendo por proceso.

**Por qué importa.** Un limiter en memoria es decorativo en horizontal, y toda la
defensa anti-brute-force (5 fallos → bloqueo de 5 min) depende de su memoria. Además
el login es el objetivo natural de un atacante.

**Cómo se hace.**
1. `KeyProvider`/`Limiter` como trait, implementación en memoria (dev) y SurrealDB o
   Valkey (prod) — ya hay `DMART_VALKEY_URL` en el código.
2. Clave por `(tenant_id, ip)` para que un tenant no agote la cuota de otro.
3. Headers `X-RateLimit-Limit/Remaining/Reset` en las respuestas.
4. Migrar también el `LoginThrottle` (bloqueo de 5 min) — hoy es el más crítico
   porque es el que protege el login.

**Comprobación.** Test de integración con dos "réplicas" (dos `SecurityState`
compartiendo backend) que demuestra que la cuota es global, no por proceso.

**Esfuerzo.** 4–6 h. **Ganancia.** Seguridad 8→9.5, Operabilidad 5→6.

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

### P1.4 Idempotencia de reintentos en ingestión HL7

**Qué está roto.** Si un emisor MLLP reintenta un mensaje (o el ACK se pierde y
reintenta), ¿se duplica el paciente/medición? No hay clave de idempotencia por
`MSH.10` (message control ID), que es justo para lo que existe.

**Por qué importa.** HL7 no garantiza entrega exactly-once. Duplicar una medición
SOFA o un Apache altera la evolución clínica del paciente.

**Cómo se hace.** Índice único sobre `(tenant_id, msh10)` + respuesta idempotente
(ACK OK con el mismo resultado, no error).

**Comprobación.** Test que envía el mismo `MSH.10` dos veces y verifica una sola fila.

**Esfuerzo.** 2–3 h. **Ganancia.** Integridad clínica. Con PHI en un sistema de
monitorización, no es cosmético.

---

## 🟢 P2 — Deuda visible y supply chain

### P2.1 Montar `GlobalHeader` o borrarlo

**Qué está roto.** Existe en `dmart-app/src/theme.rs` y **no está montado**. Código
muerto en producción, con el layout compensado por `md:ml-[280px]` en el `main`.

**Cómo se hace.** Montarlo y quitar el padding compensatorio, o borrarlo. Decidir en
5 minutos y ejecutar.

**Esfuerzo.** 30 min. **Ganancia.** Limpieza; el sidebar deja de estar emparchado.

### P2.2 Borrar `dmart-app/src/locales/`

**Qué está roto.** Duplicado obsoleto no versionado. Los locales válidos están en
`dmart-app/locales/`.

**Esfuerzo.** 1 min. **Ganancia.** Que nadie edite el fichero equivocado.

### P2.3 Los 55 lints de Clippy del frontend

**Qué está roto.** `dmart-app` tiene 55 lints (mayoría de la expansión de `view!`).
El job de CI lo marca `continue-on-error`.

**Por qué importa.** Un lint silenciado es deuda que vuelve. Y mientras sea
informativo, el frontend **no tiene lint efectivo**: nadie lo revisa.

**Cómo se hace.** Por lotes, con los 4 locales como red de seguridad (si un cambio
rompe la UI, lo ve el E2E de P0.1). Sacar `continue-on-error` cuando llegue a 0.

**Orden.** P0.1 **primero**: arreglar 55 lints sin red de seguridad E2E es
volar a ciegas.

**Esfuerzo.** 3 h. **Ganancia.** Frontend 7→8, y el lint vuelve a tener valor.

### P2.4 Supply chain

- **Firmar artefactos** (cosign/SLSA) y verificar en despliegue.
- **gitleaks/trufflehog** en CI (hoy nadie escanea secretos: el PAT expuestos se
  detectó tarde).
- **`permissions:` least-privilege** en cada job de `ci.yml`.
- **Revocar el PAT de GitHub** que sigue expuesto (`gh auth logout -h github.com` +
  Settings → Tokens). ⏳ 5 min manual, sigue abierto desde el hardening de octubre.

**Esfuerzo.** 3 h. **Ganancia.** Supply chain 6→8.

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
MAÑANA (4 h) → 7,5
  P0.1  E2E real ................... 1,5 h   el hueco navegador→API
  P0.3  i18n del contenido ......... 2,0 h   4 páginas × 4 locales
  P2.1  GlobalHeader ............... 0,5 h
  P2.2  borrar locales/ ............ 0,1 h
        → frontend 5→7, verificación 8→9

ESTA SEMANA (1 día) → 8,5
  P2.3  55 lints (con la red del E2E)
  P1.4  idempotencia HL7 MSH.10
  P2.4  PAT + gitleaks + permissions
        → supply chain 6→8, integridad clínica

SIGUIENTE (2 días) → 9,5
  P0.2  backfill PHI completo ....... 3-4 h   el riesgo legal #1
  P1.2  alerting + runbooks .......... 1 día
  P1.1  rate limit distribuido ...... 4-6 h

CIERRE (3 días) → 10
  P1.3  rotación de claves sin downtime
  P2.6  despliegue nativo (systemd, reverse proxy, upgrade/rollback)
  informe final actualizado

POST-10 — fuera del alcance de este plan, sin fecha
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
