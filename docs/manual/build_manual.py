# -*- coding: utf-8 -*-
"""Genera el manual técnico/empresarial de dMart UCI (HTML -> PDF)."""
import html, os

OUT = os.path.dirname(os.path.abspath(__file__))
IMG = "img"

def esc(s): return html.escape(s)

# ----------------------------------------------------------------------------
# CONTENIDO
# ----------------------------------------------------------------------------

FIG = {
    "login": f'<div class="fig"><img src="{IMG}/01-login.png" alt="Login"><p class="cap"><b>Figura 1.</b> Pantalla de acceso (login) con diseño responsivo, selector de tema e idioma y aviso de acceso restringido. Combina usuario, contrase\u00f1a y, si est\u00e1 habilitado, segundo factor TOTP.</p></div>',
    "dashboard": f'<div class="fig"><img src="{IMG}/02-dashboard.png" alt="Dashboard"><p class="cap"><b>Figura 2.</b> Panel de control: KPIs de la unidad, censo de camas, ocupaci\u00f3n y tarjetas de pacientes activos con \u00faltimo EWS (APACHE II, NEWS2, SOFA, SAPS III) en tiempo real.</p></div>',
    "pacientes": f'<div class="fig"><img src="{IMG}/03-pacientes.png" alt="Pacientes"><p class="cap"><b>Figura 3.</b> Censo de pacientes con b\u00fasqueda difusa (trigramas ciegos + Jaro-Winkler), filtros por estado/gravedad y paginaci\u00f3n sobre el cat\u00e1logo cl\u00ednico.</p></div>',
    "nuevo": f'<div class="fig"><img src="{IMG}/04-nuevo-paciente.png" alt="Registro de paciente"><p class="cap"><b>Figura 4.</b> Registro de nuevo paciente (ingreso a UCI): datos demogr\u00e1ficos, c\u00e9dula y n.\u00ba de historia cl\u00ednica cifrados en reposo con envelope AES-256-GCM.</p></div>',
    "detalle": f'<div class="fig"><img src="{IMG}/05-detalle-paciente.png" alt="Detalle"><p class="cap"><b>Figura 5.</b> Expediente del paciente: \u00faltimas mediciones, escalas cl\u00ednicas, riesgos de mortalidad y estancia (ML) y acciones cl\u00ednicas (medir, editar, egreso).</p></div>',
    "editar": f'<div class="fig"><img src="{IMG}/06-editar-paciente.png" alt="Editar paciente"><p class="cap"><b>Figura 6.</b> Edici\u00f3n del expediente con validaci\u00f3n de campos (edad, sexo, diagn\u00f3stico) y guardado auditado en el registro WORM.</p></div>',
    "medicion": f'<div class="fig"><img src="{IMG}/07-medicion.png" alt="Medici\u00f3n"><p class="cap"><b>Figura 7.</b> Registro de medici\u00f3n y c\u00e1lculo en vivo de las cinco escalas (APACHE II, GCS, NEWS2, SOFA, SAPS III) sobre las constantes introducidas.</p></div>',
    "timeline": f'<div class="fig"><img src="{IMG}/08-timeline.png" alt="Timeline"><p class="cap"><b>Figura 8.</b> Timeline cl\u00ednico append-only con huella criptogr\u00e1fica (fingerprint) por evento; cada evento est\u00e1 encadenado y es verificable.</p></div>',
    "cds": f'<div class="fig"><img src="{IMG}/09-cds.png" alt="CDS"><p class="cap"><b>Figura 9.</b> Motor de decisi\u00f3n cl\u00ednica (CDS): planes de cuidado basados en reglas (motor CEL) y recomendaciones accionables por perfil de gravedad.</p></div>',
    "escalation": f'<div class="fig"><img src="{IMG}/10-escalation.png" alt="Escalamiento"><p class="cap"><b>Figura 10.</b> Centro de escalamiento: alertas cl\u00ednicas por umbrales EWS, estados de severidad y acci\u00f3n inmediata desde la misma pantalla.</p></div>',
    "devices": f'<div class="fig"><img src="{IMG}/11-devices.png" alt="Dispositivos"><p class="cap"><b>Figura 11.</b> Registro de dispositivos/monitores de cama (drivers HL7 v2 por MSH.3), estado de sincronizaci\u00f3n e identificaci\u00f3n por cama-tenant.</p></div>',
    "quality": f'<div class="fig"><img src="{IMG}/12-data-quality.png" alt="Calidad de datos"><p class="cap"><b>Figura 12.</b> Consola de calidad de datos: completitud, plausibilidad cl\u00ednica, integridad referencial y validaci\u00f3n por escala sobre el dataset.</p></div>',
    "admin": f'<div class="fig"><img src="{IMG}/13-admin.png" alt="Administraci\u00f3n"><p class="cap"><b>Figura 13.</b> Panel de administraci\u00f3n: gesti\u00f3n de usuarios y roles (RBAC), camas, equipos m\u00e9dicos y operaci\u00f3n de la unidad.</p></div>',
    "soporte": f'<div class="fig"><img src="{IMG}/14-soporte.png" alt="Soporte t\u00e9cnico"><p class="cap"><b>Figura 14.</b> Consola t\u00e9cnica de soporte (SPEC-044): telemetr\u00eda por subsistema, acciones idempotentes (reintento de ingesta, reset de circuit breaker, backup, rotaci\u00f3n de modelo) y self-healing.</p></div>',
    "tenants": f'<div class="fig"><img src="{IMG}/15-tenants.png" alt="Tenants"><p class="cap"><b>Figura 15.</b> Gesti\u00f3n de multi-tenancy: organizaciones aisladas con filtrado por tenant en cada consulta y soporte de impersonaci\u00f3n acotada.</p></div>',
    "audit": f'<div class="fig"><img src="{IMG}/16-audit.png" alt="Auditor\u00eda WORM"><p class="cap"><b>Figura 16.</b> Visor forense de auditor\u00eda WORM: cadena SHA-256 encadenada, lotes firmados HMAC y export verificable de integridad.</p></div>',
    "perfil": f'<div class="fig"><img src="{IMG}/17-perfil.png" alt="Perfil"><p class="cap"><b>Figura 17.</b> Perfil de usuario: cambio de contrase\u00f1a, alta/baja de MFA TOTP con c\u00f3digos de respaldo y datos de cuenta (opcionalmente PHI protegida).</p></div>',
    "health": f'<div class="fig"><img src="{IMG}/18-health.png" alt="Health"><p class="cap"><b>Figura 18.</b> Endpoint de salud /obs/health: estado de base de datos, cach\u00e9, uptime y versi\u00f3n del binario. Complementado por /obs/live y /obs/ready.</p></div>',
    "metrics": f'<div class="fig"><img src="{IMG}/19-metrics.png" alt="Metrics"><p class="cap"><b>Figura 19.</b> Endpoint /metrics en formato Prometheus: latencias HTTP, contadores de la API, m\u00e9tricas de ML e ingesta para el panel Grafana.</p></div>',
    "openapi": f'<div class="fig"><img src="{IMG}/20-openapi.png" alt="OpenAPI"><p class="cap"><b>Figura 20.</b> Especificaci\u00f3n OpenAPI 3 (JSON de /api/v1/openapi.json) generada por utoipa: 5 grupos funcionales y contratos documentados de todos los endpoints.</p></div>',
}

# ---- CSS ----
CSS = """
@page { size: A4; margin: 20mm 16mm 22mm 16mm;
  @bottom-center { content: counter(page); font-family:'DejaVu Sans',sans-serif; font-size:9pt; color:#64748b; }
  @bottom-right { content: 'dMart UCI - Manual del Sistema'; font-family:'DejaVu Sans',sans-serif; font-size:8pt; color:#94a3b8; }
  @top-right { content: string(chap); font-family:'DejaVu Sans',sans-serif; font-size:8pt; color:#94a3b8; }
}
@page cover { margin:0; @bottom-center{content:none} @bottom-right{content:none} @top-right{content:none} }
* { box-sizing: border-box; }
html,body { margin:0; padding:0; }
body { font-family:'DejaVu Sans','Liberation Sans',Tahoma,Arial,sans-serif; font-size:10.2pt; line-height:1.5; color:#1e293b; }

/* Cover */
.cover { page: cover; width:100%; height:297mm; padding:0; position:relative; overflow:hidden;
  background: linear-gradient(160deg,#0b1f3a 0%, #0e2f52 35%, #0ea5e9 120%); color:#fff; }
.cover .band { position:absolute; left:0; right:0; height:10mm; top:0; background:linear-gradient(90deg,#0ea5e9,#10b981,#f59e0b); }
.cover .inner { position:absolute; top:0; left:0; right:0; bottom:0; padding:30mm 24mm; display:flex; flex-direction:column; }
.cover .kicker { font-size:11pt; letter-spacing:4px; text-transform:uppercase; color:#7dd3fc; font-weight:600; }
.cover h1 { font-size:44pt; margin:8mm 0 0; font-weight:800; letter-spacing:1px; line-height:1.05; }
.cover h1 .sep { color:#fbbf24; }
.cover .sub { font-size:15pt; margin-top:6mm; color:#cbd5e1; font-weight:400; max-width:150mm; }
.cover .tagline { margin-top:10mm; font-size:10.5pt; color:#94a3b8; max-width:140mm; line-height:1.7; }
.cover .badges { margin-top:14mm; }
.cover .badge { display:inline-block; border:1px solid rgba(255,255,255,.25); border-radius:6px; padding:3mm 5mm; margin:0 3mm 3mm 0; font-size:9pt; color:#e2e8f0; background:rgba(255,255,255,.06);}
.cover .meta { margin-top:auto; border-top:1px solid rgba(255,255,255,.2); padding-top:6mm; font-size:10pt; color:#cbd5e1; }
.cover .meta b { color:#fff; }
.cover .meta .row { margin-top:2mm; }
.cover .footer-note { font-size:8.5pt; color:#64748b; margin-top:4mm; }

/* Chapter headers */
h2.chap { string-set: chap content(); font-size:17pt; color:#0b1f3a; border-bottom:2.5px solid #0ea5e9; padding-bottom:3mm; margin:0 0 5mm; page-break-before:always; font-weight:800;}
h2.chap .n { color:#0ea5e9; }
h3 { font-size:12pt; color:#0e2f52; margin:5mm 0 2.5mm; font-weight:700; page-break-after:avoid;}
h4 { font-size:11pt; color:#0e2f52; margin:5mm 0 2mm; font-weight:700; }
p { margin:0 0 2.4mm; text-align:justify; }
ul,ol { margin:0 0 3mm 6mm; padding-left:5mm; }
li { margin-bottom:1mm; text-align:justify;}
b, strong { color:#0b1f3a; }

.kv { width:100%; border-collapse:collapse; margin:2mm 0 5mm; page-break-inside:avoid; font-size:9.5pt;}
.kv td { border:1px solid #dbe4f0; padding:2mm 3mm; vertical-align:top;}
.kv td.k { background:#f1f7fd; color:#0e2f52; font-weight:700; width:34%; }

/* Generic table */
table.mtable { width:100%; border-collapse:collapse; margin:1.6mm 0 4mm; font-size:8.8pt; page-break-inside:avoid; }
table.mtable th { background:#0b1f3a; color:#fff; padding:1.9mm 2.8mm; text-align:left; font-weight:600; font-size:8.6pt;}
table.mtable td { border:1px solid #dbe4f0; padding:1.9mm 2.8mm; vertical-align:top;}
table.mtable tr:nth-child(even) td { background:#f7fafc; }
table.mtable tr { break-inside:avoid; }
.note { background:#eff6ff; border-left:3px solid #0ea5e9; padding:2.6mm 3.5mm; margin:2.4mm 0 4mm; font-size:9.1pt; page-break-inside:avoid;}
.warn { background:#fff7ed; border-left:3px solid #f59e0b; padding:2.6mm 3.5mm; margin:2.4mm 0 4mm; font-size:9.1pt; page-break-inside:avoid;}
.ok { background:#ecfdf5; border-left:3px solid #10b981; padding:2.6mm 3.5mm; margin:2.4mm 0 4mm; font-size:9.1pt; page-break-inside:avoid;}
code { font-family:'DejaVu Sans Mono','Liberation Mono',monospace; font-size:8.8pt; background:#eef2f7; padding:0.3mm 1mm; border-radius:3px; color:#0e2f52;}
pre.code { font-family:'DejaVu Sans Mono',monospace; font-size:8.1pt; background:#0b1f3a; color:#e2e8f0; padding:3.4mm; border-radius:6px; white-space:pre-wrap; margin:1.6mm 0 4mm; page-break-inside:avoid; line-height:1.4;}
pre.code .tag { color:#7dd3fc; }

.fig { margin:2.4mm 0 5mm; page-break-inside:avoid; text-align:center; }
.fig img { width:100%; border:1px solid #cbd5e1; border-radius:6px; box-shadow:0 1mm 3mm rgba(11,31,58,.12); }
 .fig .cap { font-size:8.8pt; color:#475569; text-align:justify; margin-top:2mm; }
.fig .cap b { color:#0e2f52; }
.screen { page-break-before:always; break-inside:avoid; }
.screen h3 { margin-top:0; }
.screen.first { page-break-before:auto; }

.grid2 { display:flex; gap:5mm; }
.grid2 > div { flex:1; }

.toc { font-size:9.2pt; }
.toc .trow { display:flex; border-bottom:1px dotted #cbd5e1; padding:1.15mm 0; page-break-inside:avoid;}
.toc .trow .t { flex:1; color:#0e2f52; font-weight:600;}
.toc .trow.l1 { }
.toc .trow.l2 { padding-left:6mm; }
.toc .trow .p { width:12mm; text-align:right; color:#64748b; }

.pagebreak { page-break-before:always; }
.small { font-size:8.8pt; color:#64748b; }
.center { text-align:center; }
.right { text-align:right; }
section.sec { }
.lead { font-size:11pt; color:#334155; }
.titlepage { text-align:center; padding-top:0; }
.titlepage h2 { border:none; font-size:20pt; }
"""

# ---- <head> y ayuda ----
def imports(title):
    return f'<title>{title}</title>'

PAGES = []
def page(html_body):
    PAGES.append(html_body)

# ---------------------------------------------------------------------------
# 0. PORTADA
# ---------------------------------------------------------------------------
cover = """
<div class="cover"><div class="band"></div>
<div class="inner">
  <div class="kicker">Sistema de Gesti\u00f3n de Unidad de Cuidados Intensivos</div>
  <h1>dMart <span class="sep">UCI</span></h1>
  <div class="sub">Manual t\u00e9cnico y empresarial del sistema completo</div>
  <div class="tagline">Plataforma cl\u00ednica construida 100&nbsp;% en <b>Rust</b> para la gesti\u00f3n integral de una Unidad de Cuidados Intensivos: API REST y streaming en tiempo real, interoperabilidad HL7&nbsp;v2/FHIR&nbsp;R4, c\u00f3mputo de escalas de gravedad, inteligencia artificial para mortalidad y estancia, cifrado de PHI en reposo, auditor\u00eda inmutable (WORM) y frontend WebAssembly (Leptos) ejecutado \u00edntegramente en el navegador.<br><br>Documentaci\u00f3n generada a partir del c\u00f3digo real, las especificaciones del repositorio y capturas de pantalla del sistema en ejecuci\u00f3n.</div>
  <div class="badges">
    <span class="badge">Rust 2024</span><span class="badge">WebAssembly</span><span class="badge">Leptos 0.8</span><span class="badge">Axum 0.8</span><span class="badge">SurrealDB</span><span class="badge">HL7 / MLLP</span><span class="badge">FHIR R4</span><span class="badge">AES-256-GCM</span><span class="badge">Argon2id</span><span class="badge">MFA TOTP</span><span class="badge">Auditor\u00eda WORM</span><span class="badge">SSE en tiempo real</span><span class="badge">PWA / Web Push</span>
  </div>
  <div class="meta">
    <div class="row"><b>Producto:</b> dMart UCI v2.0 (specs 001-052, 46 especificaciones documentadas)</div>
    <div class="row"><b>Audiencia:</b> Equipo cl\u00ednico, ingenieros de software, arquitectos, DevSecOps y direcci\u00f3n hospitalaria</div>
    <div class="row"><b>Metodolog\u00eda:</b> Spec-Driven Development (SDD)</div>
    <div class="row"><b>Versi\u00f3n documento:</b> 2.0 &nbsp;|&nbsp; <b>Estado:</b> Aprobado &nbsp;|&nbsp; <b>Fecha:</b> Octubre de 2026</div>
    <div class="footer-note">Este documento es de uso interno autorizado. Contiene referencia a datos de demostraci\u00f3n sint\u00e9tica (no pacientes reales) y descripciones t\u00e9cnicas fieles al c\u00f3digo del repositorio.</div>
  </div>
</div></div>
"""

# ---------------------------------------------------------------------------
# 1. PORTADA INTERNA + METADATA + COMO LEER + GLOSARIO
# ---------------------------------------------------------------------------
p1 = """
<h2 class="chap"><span class="n">1.</span> Sobre este manual</h2>
<p class="lead">Este manual describe, de forma completa y verificable, el sistema <b>dMart UCI</b>: qu\u00e9 es, qu\u00e9 problema resuelve, c\u00f3mo est\u00e1 construido, c\u00f3mo funciona cada uno de sus m\u00f3dulos, qu\u00e9 algoritmos utiliza, c\u00f3mo se garantiza su seguridad y c\u00f3mo se demuestra su calidad. Cada afirmaci\u00f3n t\u00e9cnica refleja el estado real del repositorio y las capturas de pantalla corresponden al sistema ejecut\u00e1ndose con un dataset cl\u00ednico de demostraci\u00f3n a escala.</p>
<table class="mtable">
<tr><th style="width:30%">Atributo</th><th>Valor</th></tr>
<tr><td>Nombre del producto</td><td>dMart UCI - Sistema de Gesti\u00f3n de Unidad de Cuidados Intensivos</td></tr>
<tr><td>Licencia</td><td>MIT</td></tr>
<tr><td>Lenguaje de programaci\u00f3n</td><td>Rust 2024 (edici\u00f3n 1.98, rust-toolchain fijado)</td></tr>
<tr><td>Workspace</td><td>dmart-shared, dmart-server, dmart-app (WASM), dmart-server/fuzz</td></tr>
<tr><td>Base de datos</td><td>SurrealDB embebida (SurrealKV), migraciones versionadas SurrealQL</td></tr>
<tr><td>L\u00edneas de c\u00f3digo Rust</td><td>Aproximadamente 45.600 l\u00edneas en m\u00f3dulos de servidor, API, HL7, frontend y l\u00f3gica compartida</td></tr>
<tr><td>Especificaciones (SDD)</td><td>46 specs numeradas SPEC-001...SPEC-052 (todas completadas en 4 fases)</td></tr>
<tr><td>Interfaz</td><td>SPA (Leptos/WASM) con 17 rutas protegidas + PWA instalable y Web Push</td></tr>
<tr><td>Dataset de demostraci\u00f3n</td><td>520 pacientes, 74 camas, 160 equipos, 90 dispositivos, 69 usuarios</td></tr>
<tr><td>Puertos</td><td>HTTP 3000 (API y frontend); HL7/MLLP 2575 (configurable, opcional)</td></tr>
</table>

<h3>C\u00f3mo leer este documento</h3>
<ul>
<li>Los cap\u00edtulos 2 a 6 presentan el problema, la metodolog\u00eda, la tecnolog\u00eda y la arquitectura.</li>
<li>Los cap\u00edtulos 7 a 10 explican cada m\u00f3dulo funcional, los algoritmos cl\u00ednicos y el machine learning.</li>
<li>El cap\u00edtulo 11 muestra, con capturas reales, cada pantalla del sistema y su funci\u00f3n.</li>
<li>Los cap\u00edtulos 12 a 16 cubren seguridad, pruebas, observabilidad, despliegue y normativa.</li>
</ul>
<div class="note"><b>Nota de privacidad:</b> todos los nombres, c\u00e9dulas e historias cl\u00ednicas mostrados en las capturas pertenecen a un dataset sint\u00e9tico generado autom\u00e1ticamente; ninguno corresponde a pacientes reales.</div>
"""

glosario = """
<h2 class="chap"><span class="n">2.</span> Resumen ejecutivo y glosario</h2>
<h3>2.1 Resumen ejecutivo</h3>
<p>dMart UCI es una plataforma cl\u00ednica hospitalaria que unifica en un \u00fanico binario compilado en Rust toda la operaci\u00f3n de una Unidad de Cuidados Intensivos. El sistema fue dise\u00f1ado para entornos exigentes y, en particular, para \u201cred hospitalaria aislada sin acceso a internet\u201d: no depende de servicios en la nube, todo el ciclo de vida del dato (ingesta, persistencia, cifrado, c\u00f3mputo de escalas, inferencia de modelos, auditor\u00eda) ocurre dentro del hospital.</p>
<p>El frontend se compila a <b>WebAssembly</b> y se ejecuta en el navegador con el framework reactivo <b>Leptos</b>: la misma l\u00f3gica de c\u00f3mputo cl\u00ednico (escalas, validaciones, modelos) est\u00e1 compartida entre el backend y el frontend desde un crate com\u00fan (<code>dmart-shared</code>), lo que elimina la duplicaci\u00f3n de l\u00f3gica y los desajustes entre c\u00f3mo se calcula un score en el servidor y c\u00f3mo se muestra en pantalla.</p>
<p>El desarrollo sigui\u00f3 la metodolog\u00eda <b>Spec-Driven Development (SDD)</b>: 46 especificaciones formales, cada una con tests de conformidad dedicados, cobertura medida y validaci\u00f3n en el pipeline CI antes de cada entrega.</p>
<div class="ok"><b>Resultado medible:</b> 210 pruebas del backend en verde (82 unitarias + 128 de integraci\u00f3n) en el gate de cobertura, 20 tests E2E end-to-end pasando, gate de cobertura >= 60 %, fuzzing continuo de tres componentes cr\u00edticos y un dataset de demostraci\u00f3n a escala con 520 pacientes en menos de 3 segundos de siembra.</div>

<h3>2.2 Glosario</h3>
<table class="mtable">
<tr><th style="width:22%">T\u00e9rmino</th><th>Definici\u00f3n</th></tr>
<tr><td>PHI</td><td>Protected Health Information: informaci\u00f3n de salud identificable del paciente (nombre, c\u00e9dula, historia cl\u00ednica, constantes).</td></tr>
<tr><td>UCI</td><td>Unidad de Cuidados Intensivos.</td></tr>
<tr><td>EWS</td><td>Early Warning Score: puntuaci\u00f3n de alerta temprana que resume el riesgo de deterioro del paciente.</td></tr>
<tr><td>HL7 v2 / MLLP</td><td>Est\u00e1ndar de mensajer\u00eda cl\u00ednica (mensajes como ORU^R01) y el protocolo de capa inferior sobre TCP (Minimal Lower Layer Protocol).</td></tr>
<tr><td>FHIR R4</td><td>Fast Healthcare Interoperability Resources, release 4: est\u00e1ndar de intercambio de recursos cl\u00ednicos (Patient, Observation, Condition, DiagnosticReport, Bundle).</td></tr>
<tr><td>WORM</td><td>Write Once Read Many: tambi\u00e9n denominada auditor\u00eda inmutable o de una sola escritura.</td></tr>
<tr><td>RBAC</td><td>Role-Based Access Control: control de acceso basado en roles.</td></tr>
<tr><td>TOTP</td><td>Time-based One-Time Password (RFC 6238): segundo factor basado en tiempo (autenticadores tipo Google Authenticator).</td></tr>
<tr><td>JWT</td><td>JSON Web Token: token firmado que transporta la identidad y permisos de la sesi\u00f3n.</td></tr>
<tr><td>SSE</td><td>Server-Sent Events: canal HTTP unidireccional para streaming de eventos.</td></tr>
<tr><td>CDS</td><td>Clinical Decision Support: motor de reglas que asiste la decisi\u00f3n cl\u00ednica.</td></tr>
<tr><td>LOS</td><td>Length Of Stay: predicci\u00f3n de d\u00edas de estancia en UCI.</td></tr>
<tr><td>SDD</td><td>Spec-Driven Development: desarrollo guiado por especificaciones formales.</td></tr>
<tr><td>APS</td><td>Acute Physiology Score: subcomponente del APACHE II.</td></tr>
</table>
"""

# ---------------------------------------------------------------------------
# 3. PROBLEMA CLINICO
# ---------------------------------------------------------------------------
problema = """
<h2 class="chap"><span class="n">3.</span> Contexto y problema cl\u00ednico</h2>
<h3>3.1 El problema</h3>
<p>Las Unidades de Cuidados Intensivos operan con pacientes en estado cr\u00edtico cuyo deterioro puede producirse en minutos. La vigilancia cl\u00ednica depende de datos complejos: constantes vitales provenientes de monitores de cama, escalas de gravedad estandarizadas, historial acumulado y gu\u00edas de pr\u00e1ctica. Sin una plataforma integrada, este trabajo se realiza con hojas de c\u00e1lculo, papel y sistemas fragmentados que no conversan entre s\u00ed, lo que genera:</p>
<ul>
<li><b>Datos dispersos:</b> cada monitor, laboratorio y servicio conserva su propia base sin consolidaci\u00f3n.</li>
<li><b>C\u00e1lculo manual de escalas:</b> APACHE II, GCS, NEWS2, SOFA y SAPS III se punt\u00faan a mano, con riesgo de error y sin reutilizaci\u00f3n hist\u00f3rica.</li>
<li><b>Retraso en la alerta:</b> la detecci\u00f3n de un paciente que se deteriora depende de la frecuencia de las rondas.</li>
<li><b>Ausencia de interoperabilidad:</b> los monitores emiten HL7 v2, pero el hospital no tiene qui\u00e9n lo interprete y persista.</li>
<li><b>Riesgo de privacidad:</b> la PHI viaja sin cifrar ni registro de qui\u00e9n la consult\u00f3.</li>
<li><b>Sin trazabilidad:</b> no existe auditoria inmutable de accesos y modificaciones.</li>
</ul>
<h3>3.2 La propuesta de valor</h3>
<p>dMart UCI resuelve estas carencias con un \u00fanico sistema autocontenido:</p>
<table class="mtable">
<tr><th style="width:26%">Necesidad cl\u00ednica</th><th>Soluci\u00f3n de dMart UCI</th></tr>
<tr><td>Censo y expediente consolidado</td><td>M\u00f3dulo de pacientes con ingresos, egresos, expediente longitudinal y b\u00fasqueda difusa instant\u00e1nea.</td></tr>
<tr><td>Vigilancia continua</td><td>Streaming SSE en tiempo real por cama: cada nueva medici\u00f3n recalcula el EWS y notifica a las tarjetas del panel.</td></tr>
<tr><td>Escalas estandarizadas</td><td>APACHE II, GCS, NEWS2, SOFA y SAPS III en Rust con validaci\u00f3n cl\u00ednica por vectores de referencia.</td></tr>
<tr><td>Predicci\u00f3n de riesgo</td><td>Ensemble de ML (DecisionTree + regresi\u00f3n log\u00edstica + gradient boosting) con calibraci\u00f3n isot\u00f3nica/Platt para mortalidad, y red neuronal MLP para estancia.</td></tr>
<tr><td>Interoperabilidad</td><td>Listener HL7 v2 sobre MLLP con drivers por fabricante (Mindray, Philips, gen\u00e9rico) y salida FHIR R4.</td></tr>
<tr><td>Seguridad y cumplimiento</td><td>PHI cifrada en reposo (AES-256-GCM), MFA TOTP, RBAC, auditor\u00eda WORM encadenada y evidencia normativa HIPAA/ISO 27001.</td></tr>
<tr><td>Funcionamiento sin internet</td><td>Todo el stack es local: servidor, base de datos embebida y PWA instalable en los terminales de enfermer\u00eda.</td></tr>
</table>
"""

# ---------------------------------------------------------------------------
# 4. SDD
# ---------------------------------------------------------------------------
sdd = """
<h2 class="chap"><span class="n">4.</span> Metodolog\u00eda: Spec-Driven Development (SDD)</h2>
<h3>4.1 Qu\u00e9 es SDD</h3>
<p><b>Spec-Driven Development (desarrollo guiado por especificaciones)</b> es una metodolog\u00eda en la que cada requirerimiento funcional o no funcional del sistema se redacta primero como una <b>especificaci\u00f3n formal</b> (\u201cspec\u201d) y solo despu\u00e9s se implementa, se prueba y se valida contra esa especificaci\u00f3n. A diferencia de un documento de requisitos pasivo, cada spec en este proyecto tiene una entidad de control: se exige un formato m\u00ednimo (objetivo, alcance, decisiones, criterios de aceptaci\u00f3n, riesgos), se valida en CI (\u201cSpec Lint / SDD Gate\u201d) y no puede cerrarse sin los tests de conformidad y cobertura asociados.</p>
<p>La metodolog\u00eda garantiza que <i>ninguna funcionalidad queda sin enunciar</i>, que el c\u00f3digo es trazable a un documento y que el estado de avance (specs aprobadas, tests verdes) es medible en todo momento.</p>
<h3>4.2 Por qu\u00e9 se eligi\u00f3 para un sistema cl\u00ednico</h3>
<ul>
<li><b>Auditabilidad:</b> cada funci\u00f3n cl\u00ednica tiene un documento de referencia; es la base ideal para evidencia normativa (HIPAA/ISO 27001).</li>
<li><b>Seguridad por defecto (fail-closed):</b> cada spec de seguridad define el comportamiento seguro como caso por defecto y los tests lo comprueban.</li>
<li><b>Calidad medible:</b> el pipeline exige lint de specs, clippy -D warnings, tests y cobertura por m\u00f3dulo.</li>
<li><b>Colaboraci\u00f3n cl\u00ednico-t\u00e9cnica:</b> un m\u00e9dico puede validar la definici\u00f3n de una escala antes de que se programe.</li>
</ul>
<h3>4.3 El flujo SDD aplicado en el repositorio</h3>
<pre class="code">1. Definici\u00f3n      spec-XXX en `specs/` (formato validado por el SDD Gate)
2. Validaci\u00f3n     CI: `Validate spec format`, lint, revisi\u00f3n obligatoria
3. Implementaci\u00f3n  C\u00f3digo Rust/Leptos trazable a la spec
4. Tests          cargo test -p dmart-server --lib --test api_tests
5. Cobertura      cargo llvm-cov -p dmart-server --lib --test api_tests  (gate >= 60 %)
6. Conformidad    HL7/FHIR/scales con tests de conformance dedicados (SPEC-028, specs HL7)
7. Entrega        imagenes multi-stage, Helm, GitOps (ArgoCD/Flux)</pre>
<h3>4.4 Mapa de especificaciones por fase</h3>
<table class="mtable">
<tr><th style="width:9%">Fase</th><th style="width:14%">Alcanzada</th><th>Entregables principales (specs)</th></tr>
<tr><td>1</td><td>SPEC-001 ... 008</td><td>Fijar build WASM, modelo de datos, persistencia de modelos ML, integraci\u00f3n HL7/MLLP, hardening de auth, observabilidad (Prometheus/Grafana), pipeline CI completo, alertas operativas.</td></tr>
<tr><td>2</td><td>SPEC-009 ... 038</td><td>Backup/restore (DR), runbook, staging en Docker, ingesta FHIR, streaming EWS, timeline, motor CDS, registro de dispositivos, calidad de datos, escalamiento, tele-ICU, Helm/Kubernetes, GitOps, cluster SurrealDB, multi-tenancy, cobertura CI, vectores cl\u00ednicos, huellas de scores, retenci\u00f3n, hardening de ingesta, ML (ONNX/serving, similitud, LOS NN, feature store, ensemble de mortalidad).</td></tr>
<tr><td>3</td><td>SPEC-039 ... 043</td><td>Producci\u00f3n: composici\u00f3n multi-stage, evidencia normativa, optimizaci\u00f3n de costos, despliegue canary/blue-green, mediciones operativas.</td></tr>
<tr><td>4</td><td>SPEC-044 ... 052</td><td>Consola de soporte, versionado de API y gateway, feature flags, seguridad de cadena de suministro (cargo deny/audit, cosign, gitleaks), auditor\u00eda WORM, SLO/error budgets, UI operativas, Web Push con VAPID y mTLS para MLLP.</td></tr>
</table>
<div class="note"><b>Principio SDD aplicado:</b> el c\u00f3digo declara su especificaci\u00f3n. Por ejemplo, el m\u00f3dulo <code>server_ingest.rs</code> documenta el endurecimiento de la ingesta (SPEC-031) y el listener HL7 est\u00e1 \u201cfail-closed\u201d: sin mTLS o handshake no arranca en despliegues declarados productivos.</div>
"""

# ---------------------------------------------------------------------------
# 5. STACK / CONCEPTOS
# ---------------------------------------------------------------------------
stack = """
<h2 class="chap"><span class="n">5.</span> Stack tecnol\u00f3gico y conceptos aplicados</h2>
<p>Este cap\u00edtulo explica, para cada tecnolog\u00eda del sistema, qu\u00e9 es, c\u00f3mo funciona por dentro y de qu\u00e9 manera concreta se aplica en dMart UCI.</p>

<h3>5.1 Rust (2024 edition)</h3>
<p><b>Qu\u00e9 es:</b> un lenguaje de programaci\u00f3n de sistemas enfocado en rendimiento y seguridad de memoria. El compilador impide, en tiempo de compilaci\u00f3n, los errores cl\u00e1sicos de C/C++ (use-after-free, carreras de datos, desbordamientos) mediante el sistema de propiedad y pr\u00e9stamos, sin necesidad de un recolector de basura.</p>
<p><b>C\u00f3mo se aplica:</b> todo el backend (servidor HTTP Axum, parser HL7, cifrado, auditor\u00eda, ML) y toda la l\u00f3gica compartida est\u00e1n escritos en Rust. El perfil de release usa <code>opt-level="z"</code>, LTO y <code>strip</code> para minimizar el binario (un \u00fanico ejecutable que aloja API + frontend est\u00e1tico + listener de monitores). Caracter\u00edsticas de seguridad directas: tipos <code>Zeroizing</code> para secretos (borran la memoria al soltarse), comparaci\u00f3n en tiempo constante con <code>subtle</code>, y ausencia de GC (latencias deterministas para la ingesta de monitores).</p>

<h3>5.2 WebAssembly (WASM)</h3>
<p><b>Qu\u00e9 es:</b> un formato binario de bytecodes port\u00e1til que se ejecuta en una m\u00e1quina virtual integrada en los navegadores. Permite compilar c\u00f3digo de sistemas (C, C++, Rust) y ejecutarlo a velocidad casi nativa en el cliente, con un modelo de memoria aislado (sandbox).</p>
<p><b>C\u00f3mo se aplica:</b> la aplicaci\u00f3n de interfaz (<code>dmart-app</code>) se compila con Trunk a un m\u00f3dulo WASM de aproximadamente 3,2 MB (<code>dmart-app-*-_bg.wasm</code>) y un peque\u00f1o pegamento JavaScript. Al abrir el sistema, el navegador instancia el WASM y ejecuta <i>toda</i> la aplicaci\u00f3n: ruteo, reactividad, i18n, temas y llamadas a la API. El resultado es una SPA r\u00e1pida que funciona sin framework JavaScript en el host y que es instalable como PWA para operar sin una conexi\u00f3n a internet estable.</p>

<h3>5.3 Leptos 0.8 (framework reactivo)</h3>
<p><b>Qu\u00e9 es:</b> un framework de interfaz de usuario escrito en Rust que compila a WASM. Su modelo es la <b>reactividad por señales</b>: los datos se envuelven en <code>Signal</code> y la interfaz se declara como una funci\u00f3n de esas señales; cuando una se\u00f1al cambia, solo las partes dependientes se recalculan y se vuelven a renderizar (sin algoritmos de reconciliaci\u00f3n masiva).</p>
<p><b>C\u00f3mo se aplica:</b> las 17 rutas de la SPA (login, panel, pacientes, expediente, medici\u00f3n, timeline, CDS, escalamiento, dispositivos, calidad, administraci\u00f3n, soporte, tenants, auditor\u00eda y perfil) est\u00e1n declaradas en <code>app.rs</code> con protecci\u00f3n de ruta por rol. El estado de sesi\u00f3n se mantiene en señales globales: el access token vive \u00fanicamente en memoria (nunca en localStorage) y se renueva por cookie httpOnly; la identidad (nombre/rol para mostrarse en la barra) s\u00ed se persiste de forma segura. El sistema es multiling\u00fce (es, en, fr, pt) y tiene tres temas (claro, oscuro, sistema).</p>

<h3>5.4 Axum 0.8 (framework web)</h3>
<p><b>Qu\u00e9 es:</b> el framework HTTP as\u00edncrono del ecosistema Tokio/Tower para Rust: routers tipados, extracci\u00f3n de argumentos por tipos, middlewares componibles y soporte de streaming (SSE, WebSockets).</p>
<p><b>C\u00f3mo se aplica:</b> el servidor monta el API versionado (<code>/api/v1</code> y ruta heredada <code>/api</code>), el router de observabilidad (<code>/obs/health</code>, <code>/obs/live</code>, <code>/obs/ready</code>, <code>/metrics</code>) y el servicio de est\u00e1ticos del frontend desde <code>dist/</code>. Los middlewares de seguridad (rate limiting, throttle de login y de MFA, cabeceras de seguridad) son capas de Tower sobre el router. El streaming SSE de eventos cl\u00ednicos sale del mismo proceso.</p>

<h3>5.5 SurrealDB (SurrealKV embebida)</h3>
<p><b>Qu\u00e9 es:</b> una base de datos multi-modelo (documentos, grafos y consultas tipo SQL llamadas SurrealQL) distribuible. En su modo embebido (SurrealKV) funciona como una librer\u00eda dentro del proceso del servidor, sin procesos externos.</p>
<p><b>C\u00f3mo se aplica:</b> se persiste en el archivo local <code>data/dmart.db</code>. Provee transacciones at\u00f3micas para ingresos/egresos, m\u00faltiples tablas (<code>patient</code>, <code>measurement</code>, <code>user</code>, <code>refresh_token</code>, <code>audit_event</code>, <code>hl7_ingest_key</code>, <code>tenant</code>, ...) y <b>27 migraciones versionadas</b> en SurrealQL que evolucionan el esquema de forma ordenada (desde <code>001_baseline_indexes.surql</code> hasta la m\u00e1s reciente). Para alta disponibilidad, las specs definen un cluster SurrealDB de 3 nodos en Kubernetes.</p>

<h3>5.6 Server-Sent Events (SSE)</h3>
<p><b>Qu\u00e9 es:</b> un canal HTTP unidireccional donde el servidor empuja eventos al cliente de forma continua. A diferencia de WebSocket, es m\u00e1s simple y robusto en proxies que filtran por HTTP.</p>
<p><b>C\u00f3mo se aplica:</b> el m\u00f3dulo <code>realtime.rs</code> mantiene un hub de conexiones SSE (con m\u00e1ximo 10 por IP para evitar acaparar el hub). Cuando se ingiere una medici\u00f3n (manualmente o desde un monitor HL7), el evento se emite a los clientes suscritos, filtrado por tenant, y el panel actualiza las tarjetas de cama al instante.</p>

<h3>5.7 PWA / Service Worker / Web Push</h3>
<p><b>Qu\u00e9 es:</b> Progressive Web App: una web con manifiesto, service worker y APIs de notificaciones que se puede instalar y usar offline.</p>
<p><b>C\u00f3mo se aplica:</b> el manifiesto y el service worker (<code>sw.js</code>) se copian al <code>dist/</code>; el frontend declara las APIs <code>PushManager</code>/<code>PushSubscription</code> (SPEC-052) y el backend las suscripciones con claves VAPID para enviar alertas de escalamiento a los dispositivos, incluso cuando la app no est\u00e1 en primer plano.</p>

<h3>5.8 Otras piezas relevantes</h3>
<table class="mtable">
<tr><th style="width:22%">Componente</th><th>Rol en el sistema</th></tr>
<tr><td>candle 0.8</td><td>Backend de tensores para la red neuronal de estancia (MLP/LSTM) ejecutada en el servidor.</td></tr>
<tr><td>linfa</td><td>Biblioteca de ML en Rust: DecisionTree, regresi\u00f3n log\u00edstica, gradient boosting del ensemble de mortalidad.</td></tr>
<tr><td>argon2</td><td>KDF resistente para el hash de contrase\u00f1as (HIPAA/OWASP).</td></tr>
<tr><td>jsonwebtoken</td><td>Emisi\u00f3n y validaci\u00f3n de JWT HS256 revocables.</td></tr>
<tr><td>totp-rs</td><td>Segundo factor TOTP RFC 6238 con generaci\u00f3n de secretos y otpauth URI.</td></tr>
<tr><td>aes-gcm / chacha20poly1305</td><td>Cifrado autenticado AEAD para PHI: envelope vigente AES-256-GCM y legacy ChaCha20-Poly1305 auto-detectado.</td></tr>
<tr><td>rustls</td><td>TLS 1.3 para el listener MLLP con cifrados restringidos a AES-256-GCM.</td></tr>
<tr><td>cel 0.12</td><td>Motor de expresiones para las reglas cl\u00ednicas del CDS.</td></tr>
<tr><td>printpdf / csv / qrcode</td><td>Exportaci\u00f3n de informes PDF, CSV y c\u00f3digos QR del export de auditor\u00eda.</td></tr>
<tr><td>metrics / metrics-exporter-prometheus</td><td>M\u00e9tricas internas expuestas en formato Prometheus en <code>/metrics</code>.</td></tr>
<tr><td>redis / Valkey</td><td>Backend de rate limiting distribuido y cach\u00e9 cuando est\u00e1 disponible; fallback en memoria en desarrollo.</td></tr>
<tr><td>utoipa</td><td>Generaci\u00f3n de la especificaci\u00f3n OpenAPI 3 expuesta en <code>/api/v1/openapi.json</code>.</td></tr>
</table>
"""

# ---------------------------------------------------------------------------
# 6. ARQUITECTURA
# ---------------------------------------------------------------------------
arquitectura = """
<h2 class="chap"><span class="n">6.</span> Arquitectura del sistema</h2>
<p>dMart UCI se organiza en cuatro capas dentro de un \u00fanico binario. El frontend (WASM) se sirve como contenido est\u00e1tico desde el mismo proceso HTTP, lo que simplifica el despliegue a un contenedor de k8s con un StatefulSet para SurrealDB.</p>
<h3>6.1 Capas</h3>
<table class="mtable">
<tr><th style="width:20%">Capa</th><th style="width:22%">Componentes</th><th>Responsabilidad</th></tr>
<tr><td>Presentaci\u00f3n</td><td>Leptos/WASM (SPA, 17 rutas), PWA</td><td>Interfaz cl\u00ednica, reactividad, i18n, temas, notificaciones push.</td></tr>
<tr><td>Aplicaci\u00f3n</td><td>Axum (API v1 + legado), middlewares, SSE hub, listener MLLP</td><td>Endpoints REST versionados, autorizaci\u00f3n, streaming en tiempo real, ingesti\u00f3n de monitores.</td></tr>
<tr><td>Dominio / l\u00f3gica</td><td>dmart-shared (scales, ML, validaci\u00f3n, modelos)</td><td>C\u00f3mputo de escalas, ensemble de mortalidad, red neuronal LOS, validaci\u00f3n cl\u00ednica, modelos compartidos frontend/backend.</td></tr>
<tr><td>Persistencia y seguridad</td><td>SurrealDB, phi_store, crypto, audit, search</td><td>Almacenamiento multi-modelo, cifrado de PHI con \u00edndices ciegos, b\u00fasqueda difusa, auditor\u00eda WORM.</td></tr>
</table>
<h3>6.2 Vista de m\u00f3dulos del servidor</h3>
<table class="mtable">
<tr><th style="width:26%">M\u00f3dulo</th><th>Funci\u00f3n</th></tr>
<tr><td>api/versioning.rs</td><td>Router versionado, API legacy, cabeceras X-API-Version / X-API-Deprecated y OpenAPI.</td></tr>
<tr><td>api/{auth,mfa,rbac}.rs</td><td>Login, refresh, logout, MFA TOTP, me() y permisos por rol.</td></tr>
<tr><td>crypto.rs / phi_store.rs</td><td>Envelope AES-256-GCM DMART_A2, subclaves HMAC-SHA256, \u00edndices ciegos y trigramas.</td></tr>
<tr><td>audit.rs</td><td>Cadena WORM SHA-256 con lotes firmados HMAC y retenci\u00f3n de 6 a\u00f1os.</td></tr>
<tr><td>search.rs</td><td>B\u00fasqueda de pacientes: trigramas ciegos + ranking Jaro-Winkler.</td></tr>
<tr><td>server_ingest.rs / hl7/*</td><td>Listener MLLP, parser ORU^R01, drivers Mindray/Philips/gen\u00e9rico, ACK de rechazo, backpressure.</td></tr>
<tr><td>mllp_tls.rs</td><td>mTLS TLS 1.3 con p\u00edn SHA-256 del certificado vs MSH.3.</td></tr>
<tr><td>security.rs / rate_limit_store.rs</td><td>Rate limiting (Valkey o memoria), throttle de login y MFA, cabeceras y CSP/HSTS.</td></tr>
<tr><td>cds_rules.rs / patient_timeline.rs</td><td>Motor de reglas cl\u00ednicas (CEL) y timeline append-only con fingerprint.</td></tr>
<tr><td>ml_serving.rs / similarity.rs / forecasting.rs</td><td>Serving de modelos, embeddings de 128-d por feature hashing, b\u00fasqueda de pacientes similares, forecaster de estancia u ocupaci\u00f3n.</td></tr>
<tr><td>fhir_bundle.rs / teleicu.rs</td><td>Recursos FHIR R4 y export Bundle; observaci\u00f3n remota de tele-UCI.</td></tr>
<tr><td>data_quality.rs / device_registry.rs / escalation.rs / retention.rs</td><td>Calidad de datos, registro de dispositivos, escalamiento de alertas, retenci\u00f3n y downsampling.</td></tr>
<tr><td>slo.rs / observability.rs / metrics.rs</td><td>SLOs y error budgets, health checks, m\u00e9tricas Prometheus extendidas.</td></tr>
<tr><td>push.rs / support.rs / tenant.rs / deployment.rs</td><td>Web Push VAPID, consola de soporte y self-healing, multi-tenancy, despliegue/canary.</td></tr>
</table>
<h3>6.3 Flujo de una medici\u00f3n de cama (caso extremo a extremo)</h3>
<pre class="code">Monitor de cama (HL7 v2) --(MLLP/TCP, mTLS)--> server_ingest
  validate + quality --> cpu_measurement (escalas en dmart-shared)
  persist (PHI cifrada en reposo) --> realtime hub --(SSE)--> navegador (WASM)
  recompute EWS --> escalation (si umbral) --> web push VAPID</pre>
<p class="small">El mismo camino vale para una medici\u00f3n registrada desde la interfaz: el frontend llama al API, el backend valida, calcula escalas en dmart-shared y publica el evento SSE al pool de conexiones.</p>
"""

# ---------------------------------------------------------------------------
# 7. MODELO DE DATOS + PHI
# ---------------------------------------------------------------------------
datos = """
<h2 class="chap"><span class="n">7.</span> Modelo de datos y protecci\u00f3n de PHI</h2>
<h3>7.1 Modelo de datos (SurrealDB)</h3>
<p>El esquema evoluciona con 27 migraciones SurrealQL versionadas. Las tablas principales se agrupan por dominio:</p>
<table class="mtable">
<tr><th style="width:24%">Dominio</th><th>Tablas / registros</th></tr>
<tr><td>Pacientes</td><td><code>patient</code> (expediente, c\u00e9dula, historia cl\u00ednica, estado, gravedad, scores), <code>patient_event</code> (timeline), <code>admission</code>/historial de camas.</td></tr>
<tr><td>Cl\u00ednico</td><td><code>measurement</code> (constantes y escalas), vectores de referencia cl\u00ednica para validaci\u00f3n de rangos.</td></tr>
<tr><td>Identidad</td><td><code>user</code> (staff), <code>refresh_token</code>, <code>audit_event</code>, sesiones MFA.</td></tr>
<tr><td>Operaci\u00f3n</td><td><code>cama</code>, <code>equipo</code>, <code>dispositivo</code> (monitores), <code>alert</code>, <code>cds_plan</code>, <code>tenant</code>.</td></tr>
<tr><td>Integridad</td><td><code>hl7_ingest_key</code> (clave unique tenant+message_id para idempotencia), <code>audit_event</code>, <code>support_event</code>.</td></tr>
</table>
<p>Las claves \u00fanicas compuestas (p. ej. <code>tenant_id</code> + <code>message_id</code> en <code>hl7_ingest_key</code>) dan idempotencia a la ingesta: si un monitor reenv\u00eda el mismo mensaje (mismo MSH.10), se responde el ACK idempotente sin duplicar la medici\u00f3n.</p>
<h3>7.2 Cifrado de PHI en reposo</h3>
<p>La informaci\u00f3n de salud identificable nunca se guarda en claro:</p>
<ul>
<li><b>Envelope vigente DMART_A2:</b> cada campo PHI se cifra con <b>AES-256-GCM</b> (cifrado autenticado: detecta cualquier alteraci\u00f3n). Se usa un <b>envelope con clave por registro</b> (DEK) derivada de una clave maestra, de forma que diferentes registros con la misma c\u00e9dula no tengan el mismo texto cifrado (nonce aleatorio de 12 bytes).</li>
<li><b>Envelope legacy DMART_V1:</b> ChaCha20-Poly1305. Se conserva para descifrar datos antiguos y se detecta autom\u00e1ticamente por el "magic" del sobre (backfill transparente).</li>
<li><b>Subclaves HMAC-SHA256:</b> la clave maestra se deriva en subclaves etiquetadas <code>LABEL_PHI</code> y <code>LABEL_INDEX</code>, separando cifrado de datos y c\u00f3mputo de \u00edndices.</li>
<li><b>Claves en memoria:</b> los secretos se guardan en <code>Zeroizing</code> (cero al soltar) y el proveedor de claves (<code>KeyProvider</code>) permite rotaci\u00f3n sin downtime anotando el <code>key_id</code> por envelope.</li>
</ul>
<h3>7.3 \u00cdndices ciegos y b\u00fasqueda sin exponer PHI</h3>
<p>Para permitir consultas r\u00e1pidas sin escribir texto en claro en disco, el sistema calcula <b>HMAC-SHA256</b> (clave = subclave de \u00edndice) de la c\u00e9dula, historia cl\u00ednica, nombre exacto y de los <b>trigramas</b> del nombre normalizado (campo <code>bi_tng</code>). Una b\u00fasqueda:</p>
<ol>
<li>Normaliza la consulta (may\u00fasculas, sin tildes),</li>
<li>calcula los trigramas y sus HMAC,</li>
<li>busca filas candidatas por coincidencia de trigramas (solo se comparan HMAC, nunca texto),</li>
<li>descifra los pocos candidatos y los ordena con <b>Jaro-Winkler</b> en Rust, con bonificaci\u00f3n por prefijo y coincidencia exacta.</li>
</ol>
<div class="ok"><b>Efecto:</b> se puede buscar "gus", "jorg" u "ortiz" y tolerar erratas de teclado, sin que un atacante con acceso al disco pueda leer ning\u00fan nombre paciente.</div>
"""

# ---------------------------------------------------------------------------
# 8. AUTH/RBAC
# ---------------------------------------------------------------------------
auth = """
<h2 class="chap"><span class="n">8.</span> Autenticaci\u00f3n, autorizaci\u00f3n y RBAC</h2>
<h3>8.1 Autenticaci\u00f3n</h3>
<p>El sistema aplica un esquema de sesi\u00f3n de tokens de corta vida con renovaci\u00f3n segura:</p>
<table class="mtable">
<tr><th style="width:26%">Mecanismo</th><th>Detalle</th></tr>
<tr><td>Contrase\u00f1as</td><td>Hash con <b>Argon2id</b> (par\u00e1metros OWASP), nunca en claro, con fuerza validada (min. 12 caracteres por pol\u00edtica, rechazo de placeholders).</td></tr>
<tr><td>Access token</td><td><b>JWT HS256</b> de 15 minutos con expiraci\u00f3n, claim <code>tenant_id</code>, lista de permisos y versi\u00f3n; revocable. Vive solo en memoria del navegador.</td></tr>
<tr><td>Refresh token</td><td>7 d\u00edas, persistido en cookie <b>httpOnly + SameSite=Strict</b>; rota en cada renovaci\u00f3n y se revoca en logout. El hilo de sesi\u00f3n lo renueva cada 10 minutos.</td></tr>
<tr><td>Segundo factor</td><td><b>MFA TOTP RFC 6238</b> (ventana de 30 s, tolerancia de deriva) con c\u00f3digos de respaldo de un solo uso; flujo de challenge antes de entregar token.</td></tr>
<tr><td>Anti fuerza bruta</td><td>Throttle de login, throttle espec\u00edfico del flujo MFA y rate limit global; en producci\u00f3n el l\u00edmite se distribuye con Valkey/Redis (INCR + EXPIRE at\u00f3micos).</td></tr>
</table>
<h3>8.2 Autorizaci\u00f3n basada en roles (RBAC)</h3>
<p>El modelo define roles y permisos granulares. Cada endpoint verifica el permiso requerido desde el token; no basta "estar autenticado".</p>
<table class="mtable">
<tr><th style="width:16%">Rol</th><th style="width:40%">Permisos clave</th><th>Uso t\u00edpico</th></tr>
<tr><td>Admin</td><td>Todos (<code>*</code>), gesti\u00f3n de usuarios, camas, equipos, tenants, audit</td><td>Administraci\u00f3n del sistema y de la unidad</td></tr>
<tr><td>M\u00e9dico</td><td>patients:*, measurements:*, scales:*, cds, escalamiento, devices</td><td>Diagn\u00f3stico, prescripci\u00f3n y revisi\u00f3n de escalas</td></tr>
<tr><td>Enfermero</td><td>patients:read, measurements:create/read, escalamiento</td><td>Ingreso de constantes y vigilancia de cama</td></tr>
<tr><td>Viewer</td><td>patients:read, measurements:read, quality:read</td><td>Consulta de solo lectura (reportes, auditor\u00eda cl\u00ednica)</td></tr>
<tr><td>Soporte</td><td>support:read/act + contexto cl\u00ednico de solo lectura</td><td>Operaciones t\u00e9cnicas sin alterar cl\u00ednica</td></tr>
</table>
<p>Cada acceso a un recurso verifica adem\u00e1s la <b>pertenencia al tenant</b> (<code>require_tenant_ownership</code>): un usuario de un hospital no puede leer pacientes de otro aunque tenga el rol adecuado. En el frontend, la barra lateral muestra u oculta opciones seg\u00fan permisos (<code>user_has(...)</code>) y las rutas protegidas redirigen ante falta de permiso.</p>
<h3>8.3 Flujo de ingreso interactivo</h3>
<ol>
<li>El profesional introduce usuario y contrase\u00f1a en la vista de login.</li>
<li>El backend valida Argon2id; si el usuario tiene MFA activo devuelve un <i>challenge</i>.</li>
<li>La interfaz muestra el paso TOTP (o c\u00f3digo de respaldo).</li>
<li>Con el challenge resuelto se entrega access token (memoria) + refresh (cookie) y la app navega al panel.</li>
<li>El panel carga estad\u00edsticas, censo y suscripci\u00f3n SSE.</li>
</ol>
"""

# ---------------------------------------------------------------------------
# 9. SEGURIDAD PERIMETRAL
# ---------------------------------------------------------------------------
perim = """
<h2 class="chap"><span class="n">9.</span> Seguridad del per\u00edmetro y capas de defensa</h2>
<h3>9.1 Frontera MLLP (monitores de cama)</h3>
<p>MLLP es un protocolo de texto plano sobre TCP: sin protecci\u00f3n, las constantes vitales viajar\u00edan en claro y cualquier host de la red podr\u00eda inyectar datos. La frontera se endurece en capas:</p>
<ul>
<li><b>mTLS obligatorio en producci\u00f3n:</b> TLS 1.3 exclusivo, cifrado \u00fanicamente AES-256-GCM (<code>TLS13_AES_256_GCM_SHA384</code>), certificado de cliente verificado contra la CA propia de monitores, no contra la CA p\u00fablica.</li>
<li><b>Pinning de identidad:</b> el fingerprint SHA-256 del certificado se vincula al <code>MSH.3</code> que la conexi\u00f3n declara (<code>DMART_MLLP_CLIENT_IDS</code>). Un monitor no puede hacerse pasar por otro.</li>
<li><b>Handshake de secreto compartido</b> (<code>DMART_MLLP_AUTH_SECRET</code>) como capa 2 cuando no hay mTLS; comparaci\u00f3n en tiempo constante y buffer <code>Zeroizing</code>.</li>
<li><b>Allowlist de emisores</b> sobre <code>MSH.3</code>, bind por defecto a loopback.</li>
<li><b>Anti slow-loris:</b> timeouts de lectura/escritura y tope de conexiones global y por IP.</li>
<li><b>Fail-closed:</b> sin mTLS ni handshake, el listener no arranca en despliegues productivos.</li>
<li><b>Idempotencia:</b> deduplicaci\u00f3n por <code>MSH.10</code> con clave UNIQUE (tenant, message_id) y ACK de rechazo para mensajes inv\u00e1lidos.</li>
</ul>
<h3>9.2 Protecciones web</h3>
<table class="mtable">
<tr><th style="width:28%">Control</th><th>Implementaci\u00f3n</th></tr>
<tr><td>Rate limiting</td><td>Trait <code>RateLimitStore</code>: Valkey/Redis (INCR+EXPIRE, aislamiento por tenant via JWT) o memoria; cabeceras <code>X-RateLimit-Limit/Remaining/Reset</code>.</td></tr>
<tr><td>Cabeceras de seguridad</td><td><code>X-Frame-Options</code> (anti clickjacking), CSP, HSTS en producci\u00f3n, CORS restringido a or\u00edgenes conocidos.</td></tr>
<tr><td>Cifrado en tr\u00e1nsito (HTTP)</td><td>TLS en el proxy/gateway (Caddy en dev, Ingress en k8s); la cookie de refresh exige segura en producci\u00f3n.</td></tr>
<tr><td>Gesti\u00f3n de secretos</td><td><code>JWT_SECRET</code> y <code>DMART_MASTER_KEY</code> obligatorios, <code>validate_secret_strength()</code> fail-closed (64 hex \u00f3 m\u00e1s, rechazo de placeholders), <code>Zeroizing</code> en memoria.</td></tr>
<tr><td>Cadena de suministro</td><td><code>cargo deny</code> + <code>cargo audit</code> en CI, firma de im\u00e1genes <b>cosign keyless</b>, escaneo <b>gitleaks</b> SARIF, lakefile de expediente por evidencia.</td></tr>
</table>
<h3>9.3 Modelo de amenazas resumido</h3>
<table class="mtable">
<tr><th style="width:26%">Amenaza</th><th style="width:40%">Vector</th><th>Contramedida en dMart UCI</th></tr>
<tr><td>Robo de sesi\u00f3n</td><td>XSS, token en log</td><td>Access token solo en memoria; JWT revocable; CSP; nunca loguear tokens. El access token de 15 min limita la ventana de abuso.</td></tr>
<tr><td>Fuerza bruta</td><td>Login / TOTP</td><td>Throttle de login y MFA, rate limit distribuido, c\u00f3digos de respaldo de un solo uso.</td></tr>
<tr><td>Fuga de PHI en reposo</td><td>Acceso f\u00edsico al disco, backup</td><td>AES-256-GCM por campo, subclaves HMAC, \u00edndices ciegos: el disco no contiene texto legible.</td></tr>
<tr><td>Inyecci\u00f3n en MLLP</td><td>Host en la red hospitalaria</td><td>mTLS + pinning a MSH.3, allowlist de emisores, fail-closed, timeouts y tope de conexiones.</td></tr>
<tr><td>Alteraci\u00f3n de auditor\u00eda</td><td>DBA malintencionado, ransomware</td><td>Cadena WORM con prev_hash y lotes firmados HMAC; export verificable de integridad.</td></tr>
<tr><td>Escalada vertical</td><td>Endpoint abusivo</td><td>RBAC con permiso por recurso y verificaci\u00f3n de pertenencia al tenant en cada acceso.</td></tr>
</table>
"""

# ---------------------------------------------------------------------------
# 10. ALGORITMOS CLINICOS
# ---------------------------------------------------------------------------
algo = """
<h2 class="chap"><span class="n">10.</span> Algoritmos cl\u00ednicos: escalas de gravedad</h2>
<p>Los c\u00e1lculos de escalas viven en <code>dmart-shared/src/scales.rs</code>, compartidos entre servidor y frontend, con vectores de referencia cl\u00ednica (SPEC-028) que validan rangos normales y cr\u00edticos en tests y en la consola de calidad de datos.</p>

<h3>10.1 APACHE II (Acute Physiology And Chronic Health Evaluation II)</h3>
<p><b>Qu\u00e9 es:</b> el sistema de clasificaci\u00f3n de gravedad m\u00e1s extendido en UCI (Knaus et al., 1985). Estima la probabilidad de mortalidad hospitalaria a partir de datos fisiol\u00f3gicos del primer d\u00eda de ingreso.</p>
<table class="mtable">
<tr><th>Componente</th><th style="width:16%">Rango de puntos</th><th>Detalle</th></tr>
<tr><td>Acute Physiology Score (APS)</td><td>0-60</td><td>12 variables: temperatura, PAM, frecuencia card\u00edaca, frecuencia respiratoria, oxigenaci\u00f3n (PaO2/a/A), pH arterial o HCO3, sodio y potasio s\u00e9rico, creatinina, hematocrito, leucocitos y GCS corregido.</td></tr>
<tr><td>Puntuaci\u00f3n por edad</td><td>0-6</td><td>Escalones desde &lt;=44 a\u00f1os (0) hasta &gt;=75 a\u00f1os (6).</td></tr>
<tr><td>Enfermedad cr\u00f3nica grave</td><td>0-5</td><td>Insuficiencia org\u00e1nica cr\u00f3nica o inmunodepresi\u00f3n, con restricciones quir\u00fargicas/electivas.</td></tr>
</table>
<p>Total m\u00e1ximo: <b>71 puntos</b>. La probabilidad de mortalidad hospitalaria se obtiene con una funci\u00f3n log\u00edstica sobre el score y el grupo diagn\u00f3stico (la implementaci\u00f3n incluye la f\u00f3rmula y su constante de regresi\u00f3n).</p>

<h3>10.2 Escala de Coma de Glasgow (GCS)</h3>
<p><b>Qu\u00e9 es:</b> mide el nivel de conciencia (Teasdale &amp; Jennett, 1974) puntuando tres respuestas.</p>
<table class="mtable">
<tr><th>Componente</th><th style="width:14%">Puntos</th><th>Descripci\u00f3n</th></tr>
<tr><td>Apertura ocular</td><td>1-4</td><td>Espont\u00e1nea (4), a la voz (3), al dolor (2), ninguna (1).</td></tr>
<tr><td>Respuesta verbal</td><td>1-5</td><td>Orientado (5)... ninguna (1); intubados se codifican como "1T".</td></tr>
<tr><td>Respuesta motora</td><td>1-6</td><td>Obedece \u00f3rdenes (6)... ninguna (1).</td></tr>
</table>
<p>Total: <b>3-15</b>; menor de 8 suele indicar coma y alerta para asegurar v\u00eda a\u00e9rea.</p>

<h3>10.3 NEWS2 (National Early Warning Score 2)</h3>
<p><b>Qu\u00e9 es:</b> sistema de alerta temprana del Royal College of Physicians (2017) para detectar deterioro cl\u00ednico con datos de cabecera.</p>
<table class="mtable">
<tr><th style="width:30%">Par\u00e1metro</th><th>Puntos por desviaci\u00f3n</th></tr>
<tr><td>Frecuencia respiratoria, SpO2, tensi\u00f3n arterial sist\u00f3lica, pulso, conciencia (CVPU) y temperatura</td><td>0-3 por par\u00e1metro seg\u00fan rango; +2 si se requiere ox\u00edgeno suplementario.</td></tr>
<tr><td>Escala</td><td>Total <b>0-20</b>; umbrales de alerta en 5 (bajo), 6-7 (medio) y >=8 (alto).</td></tr>
</table>
<p>En el sistema, NEWS2 alimenta el streaming EWS por cama: cada medici\u00f3n nueva lo recalcula y si supera umbral se dispara el escalamiento.</p>

<h3>10.4 SOFA (Sequential Organ Failure Assessment)</h3>
<p><b>Qu\u00e9 es:</b> punt\u00faa la disfunci\u00f3n de seis sistemas de \u00f3rganos (Vincent et al., 1996); cada sistema aporta 0-4 puntos.</p>
<table class="mtable">
<tr><th>Sistema</th><th style="width:14%">Puntos</th><th>Variable proxy</th></tr>
<tr><td>Respiratorio</td><td rowspan="6">0-4</td><td>PaO2/FiO2 (relaci\u00f3n de oxigenaci\u00f3n)</td></tr>
<tr><td>Coagulaci\u00f3n</td><td>Plaquetas</td></tr>
<tr><td>Hep\u00e1tico</td><td>Bilirrubina</td></tr>
<tr><td>Cardiovascular</td><td>PAM / vazoactivos</td></tr>
<tr><td>Neurol\u00f3gico</td><td>GCS</td></tr>
<tr><td>Renal</td><td>Creatinina o diuresis</td></tr>
</table>
<p>Total <b>0-24</b>. Normal &lt; 5; &gt;=5 se asocia a disfunci\u00f3n multiorg\u00e1nica.</p>

<h3>10.5 SAPS III (Simplified Acute Physiology Score III)</h3>
<p><b>Qu\u00e9 es:</b> score de admisi\u00f3n (Moreno et al., 2005) con variables fisiol\u00f3gicas, demogr\u00e1ficas y de admisi\u00f3n (origen, motivo, comorbilidades, uso de vasopresores, GCS): total <b>0-217</b> en su formulaci\u00f3n original, convertido a probabilidad de mortalidad hospitalaria. <b>Variante dMart:</b> el sistema implementa una versi\u00f3n simplificada acotada a las variables de <code>ApacheIIData</code>, cuyo m\u00e1ximo calculado es <b>0-145</b> (documentado en <code>scales.rs</code>). En el sistema es uno de los scores que se calculan al registrar una medici\u00f3n con los datos soportados.</p>
<div class="note"><b>Validaci\u00f3n cl\u00ednica autom\u00e1tica:</b> los tests usan vectores de referencia con pacientes "normales" y "cr\u00edticos" (p. ej. SAPS III normal &lt; 30, cr\u00edtico &gt;= 50; NEWS2 normal &lt; 5, cr\u00edtico &gt;= 10; SOFA normal &lt; 5) y fallan si el c\u00e1lculo no se ajusta a esos l\u00edmites (fixtures en <code>dmart-shared/testdata/scales</code>).</div>
"""

# ---------------------------------------------------------------------------
# 11. ML
# ---------------------------------------------------------------------------
ml = """
<h2 class="chap"><span class="n">11.</span> Inteligencia artificial y machine learning</h2>
<h3>11.1 Ensemble de mortalidad hospitalaria</h3>
<p>El m\u00f3dulo <code>ml_ensemble.rs</code> combina tres modelos para estimar la probabilidad de mortalidad durante la hospitalizaci\u00f3n:</p>
<table class="mtable">
<tr><th style="width:30%">Modelo base</th><th>Rol</th></tr>
<tr><td>DecisionTree (linfa)</td><td>Captura interacciones no lineales entre variables y es interpretable (reglas).</td></tr>
<tr><td>Regresi\u00f3n log\u00edstica (GLM binomial)</td><td>Referencia lineal calibrada en probabilidades.</td></tr>
<tr><td>Gradient Boosting</td><td>Refuerza la precisi\u00f3n sobre residuos del conjunto.</td></tr>
<tr><td>Stacking (opcional)</td><td>Meta-modelo que combina las salidas de los base.</td></tr>
</table>
<p>Las probabilidades crudas de cada modelo se <b>calibran</b> (isoto\u0301nica o <b>Platt scaling</b>, resolviendo el problema de la singularidad de la matriz con regularizaci\u00f3n) para que el valor de salida sea una probabilidad honesta: un riesgo del 0,80 significa realmente 80 % en la poblaci\u00f3n de calibraci\u00f3n. Las features provienen de <code>ml_features.rs</code> (escalas, constantes y datos demogr\u00e1ficos).</p>
<h3>11.2 Red neuronal de estancia (LOS)</h3>
<p>El m\u00f3dulo <code>ml_los.rs</code> predice la Estancia (Length of Stay). Usa un backend de tensores <b>candle 0.8</b>:</p>
<ul>
<li>Arquitectura <b>MLP multi-capa</b> con window fija sobre el historial de mediciones y capas recurrentes opcionales (<b>LSTM/GRU</b>).</li>
<li>Entrenamiento con descenso de gradiente (Adam) sobre m\u00faltiples \u00e9pocas, con dropout para regularizar y normalizaci\u00f3n de entradas.</li>
<li>Pesos persistidos y cargables (<code>ModelWeights</code>) para rehidratar el modelo sin retraducci\u00f3n.</li>
<li>M\u00e9tricas de inferencia expuestas a Prometheus.</li>
</ul>
<h3>11.3 Motor de similitud de pacientes</h3>
<p>El m\u00f3dulo <code>similarity.rs</code> (SPEC-033) genera <b>embeddings cl\u00ednicos de 128 dimensiones</b> mediante feature hashing de las variables del paciente y calcula coseno entre vectores para devolver los <b>top-K</b> pacientes m\u00e1s parecidos (con explicaci\u00f3n por feature y scope de tenant). La API ofrece b\u00fasqueda, explicabilidad, estado y regeneraci\u00f3n de embeddings. Es una base para "pacientes como \u00e9ste" en el CDS.</p>
<h3>11.4 Forecasting de operaciones</h3>
<p>El m\u00f3dulo <code>forecasting.rs</code> produce proyecciones de ocupaci\u00f3n y estancia con cuantiles normales (naive forecaster implementado sobre <code>statrs</code>), usado por los paneles de administraci\u00f3n y los scripts de costos (<code>cost_forecast.sh</code>).</p>
<div class="note"><b>Serving:</b> <code>ml_serving.rs</code> implementa un registro de modelos (<code>Predictor</code> trait) con swap en caliente v\u00eda API (<code>/ml/models/swap</code>), batch inference y m\u00e9tricas <code>ml_inference_*</code>. La especificaci\u00f3n contaba tambi\u00e9n con un backend de inferencia sobre ONNX/WASM (SPEC-032).</div>
"""

# ---------------------------------------------------------------------------
# 12. INTEROP + TIEMPO REAL
# ---------------------------------------------------------------------------
inter = """
<h2 class="chap"><span class="n">12.</span> Interoperabilidad y tiempo real</h2>
<h3>12.1 HL7 v2 + MLLP</h3>
<p>Los monitores de cama emiten mensajes <b>HL7 v2</b> (el est\u00e1ndar de mensajer\u00eda de salud m\u00e1s desplegado en equipos m\u00e9dicos). El tipo de mensaje soportado es <b>ORU^R01</b> (Observation Result Unsolicited), que transporta las observaciones (constantes y par\u00e1metros) de un paciente.</p>
<p><b>Protocolo MLLP:</b> encuadra cada mensaje HL7 entre un byte de inicio (0x0B VT), el bloque de datos y un bloque de fin (0x1C FS + 0x0D CR). El parser ASN.1-like de <code>hl7/parser.rs</code> interpreta los segmentos (MSH, PID, OBR, OBX...), valida el <code>MSH.10</code> (idempotencia), aplica control de calidad (<code>ingest/quality.rs</code>: plausibilidad de rangos) y persiste la medici\u00f3n. Los ACK de aceptaci\u00f3n y rechazo siguen las reglas de HL7.</p>
<p>El sistema incluye <b>drivers por fabricante</b> (Mindray, Philips y gen\u00e9rico) que normalizan el mapeo de c\u00f3digo de observaci\u00f3n a las escalas internas, y <b>backpressure</b>: si el sistema se satura, la ingesta regula la tasa y abre un <b>circuit breaker</b> ante fallos repetidos (recuperable desde la consola de soporte o autom\u00e1ticamente HalfOpen).</p>
<h3>12.2 FHIR R4</h3>
<p>El m\u00f3dulo <code>fhir_bundle.rs</code> genera recursos FHIR R4 para el intercambio institucional:</p>
<table class="mtable">
<tr><th style="width:28%">Recurso FHIR</th><th>Contenido emitido</th></tr>
<tr><td>Patient</td><td>Expediente del paciente (identificadores, demograf\u00eda) con PHI ya protegida por pol\u00edtica de export.</td></tr>
<tr><td>Observation (LOINC)</td><td>Constantes y escalas codificadas con LOINC.</td></tr>
<tr><td>Condition (CIE-10)</td><td>Diagn\u00f3sticos con c\u00f3digos CIE-10 (p. ej. I63.9, I60.9, G61.0).</td></tr>
<tr><td>DiagnosticReport / Bundle</td><td>Informes y paquetes para consumo externo (hv, sistemas institucionales).</td></tr>
</table>
<h3>12.3 Streaming de alertas tempranas (EWS)</h3>
<p>El m\u00f3dulo <code>ews_stream.rs</code> combina las escalas (NEWS2/APACHE/SOFA) con las \u00faltimas mediciones de cada cama y emite el estado por <b>SSE</b>. Las conexiones se limitan por IP, se filtran por tenant y el panel las consume para pintar tarjetas en vivo. La especificaci\u00f3n SPEC-014 garantiza adem\u00e1s que el streaming siga funcionando cuando llegan datos de varios monitores a la vez.</p>
<h3>12.4 Tele-UCI</h3>
<p>El m\u00f3dulo <code>teleicu.rs</code> permite a un centro remoto observar el estado agregado de la unidad (espejo de censo y alertas) respetando el aislamiento de tenant y los permisos, pensado para modelos de tele-intensivismo.</p>
"""

# ---------------------------------------------------------------------------
# 13. AUDITORIA + TIMELINE + BUSQUEDA + CALIDAD
# ---------------------------------------------------------------------------
audit = """
<h2 class="chap"><span class="n">13.</span> Auditor\u00eda WORM, timeline, b\u00fasqueda y calidad</h2>
<h3>13.1 Auditor\u00eda inmutable (WORM)</h3>
<p><code>audit.rs</code> implementa un registro de auditor\u00eda con garant\u00eda de no manipulaci\u00f3n:</p>
<ul>
<li><b>Cadena encadenada:</b> cada evento incluye el <code>prev_hash</code> (SHA-256) del anterior; la g\u00e9nesis es un hash de 64 ceros.</li>
<li><b>Lotes firmados:</b> los eventos se agrupan en lotes de hasta 1.000 registros firmados con <b>HMAC-SHA256</b> usando subclave derivada. Alterar un evento invalida su hash, el siguiente eslab\u00f3n y la firma del lote.</li>
<li><b>Concurrencia segura:</b> la escritura est\u00e1 serializada con <code>tokio::Mutex</code>; ninguna operaci\u00f3n introduce huecos.</li>
<li><b>Retenci\u00f3n:</b> 6 a\u00f1os, con tarea de limpieza <code>POST /api/admin/audit/cleanup</code> y verificaci\u00f3n de integridad de la cadena en la exportaci\u00f3n.</li>
<li><b>Visor forense:</b> el frontend muestra la cadena y permite copiar el resumen del export (SPEC-049).</li>
</ul>
<h3>13.2 Timeline cl\u00ednico append-only</h3>
<p>El <code>patient_timeline.rs</code> construye la l\u00ednea de tiempo del paciente (ingreso, mediciones, planes CDS, egreso) como eventos de solo a\u00f1adido. Cada evento tiene un <b>fingerprint</b> criptogr\u00e1fico, de modo que cualquier edici\u00f3n o borrado posterior es detectable. Esto complementa la auditor\u00eda WORM con la vista cl\u00ednica temporal.</p>
<h3>13.3 B\u00fasqueda de pacientes</h3>
<p>Ya descrita en 7.3: normalizaci\u00f3n, trigramas ciegos (HMAC), reducci\u00f3n de candidatos y ranking difuso Jaro-Winkler con bonificaciones. Soporta b\u00fasqueda parcial por pocas letras, nombres con espacio, apellidos, tildes y erratas; exige como m\u00ednimo 3 caracteres para el camino difuso y cae a coincidencia exacta por debajo (por c\u00e9dula, historia o nombre).</p>
<h3>13.4 Calidad de datos</h3>
<p>El m\u00f3dulo <code>data_quality.rs</code> audita el dataset: <b>completitud</b> (campos requeridos por registro), <b>plausibilidad</b> (rangos fisiol\u00f3gicos por escala y variable), <b>integridad referencial</b> (mediciones sin paciente v\u00e1lido, camas hu\u00e9rfanas) y <b>consistencia de\u00a0scores</b> (que la escala calculada coincida con la persistida). La consola muestra el estado por m\u00e9trica y permite accionar correcciones. El validador de ingesta comparte estas reglas (<code>ValidationReason</code>) antes de aceptar una medici\u00f3n.</p>
"""

# ---------------------------------------------------------------------------
# 14. PANTALLAS (intro)
# ---------------------------------------------------------------------------
screens_intro = """
<h2 class="chap"><span class="n">14.</span> Pantallas del sistema (capturas reales)</h2>
<p>Este cap\u00edtulo documenta, una por una, las pantallas del sistema en ejecuci\u00f3n. Las capturas fueron tomadas con el sistema corriendo contra la base de datos de demostraci\u00f3n (520 pacientes, 74 camas, 40 pacientes activos en cama, 160 equipos, 90 dispositivos y 69 usuarios). El orden recorre el ciclo de uso real: acceso, operaci\u00f3n diaria, administraci\u00f3n y t\u00e9cnica. Las rutas del SPA son: <code>/login</code>, <code>/</code>, <code>/patients</code>, <code>/patients/new</code>, <code>/patients/:id</code>, <code>/patients/:id/edit</code>, <code>/patients/:id/measure</code>, <code>/patients/:id/timeline</code>, <code>/cds</code>, <code>/escalation</code>, <code>/devices</code>, <code>/data-quality</code>, <code>/admin</code>, <code>/admin/soporte</code>, <code>/admin/tenants</code>, <code>/admin/audit</code> y <code>/perfil</code>.</p>
"""

screens = []
def add_screen(title, body, fig, first=False):
    cls = "screen first" if first else "screen"
    screens.append(f'<section class="{cls}"><h3>{title}</h3><p>{body}</p>{fig}</section>')

add_screen("14.1 Acceso al sistema (/login)",
 "El punto de entrada valida credenciales con Argon2id y, si el usuario tiene MFA, solicita el c\u00f3digo TOTP. Incluye selector de idioma y tema, y el aviso de acceso restringido en la versi\u00f3n de la interfaz.", FIG["login"], first=True)
add_screen("14.2 Panel de control (/)",
 "El dashboard consolida los KPIs de la unidad: censo activo, ocupaci\u00f3n de camas, pacientes por estado de gravedad y tarjetas por cama con su \u00faltimo EWS. Se alimenta de /api/stats y del streaming SSE, de modo que una nueva medici\u00f3n actualiza las tarjetas sin recargar la p\u00e1gina.", FIG["dashboard"])
add_screen("14.3 Censo de pacientes (/patients)",
 "Listado paginado del cat\u00e1logo cl\u00ednico con la b\u00fasqueda difusa (trigramas ciegos + Jaro-Winkler), filtros por estado y gravedad, y columnas de \u00faltimos scores. Cada fila permite abrir el expediente, medir o ver la timeline.", FIG["pacientes"])
add_screen("14.4 Registro de paciente (/patients/new)",
 "Formulario de ingreso a UCI. La c\u00e9dula y el n\u00famero de historia se cifran en reposo y se indexan ciegos para permitir b\u00fasqueda r\u00e1pida sin exponer PHI en disco.", FIG["nuevo"])
add_screen("14.5 Expediente del paciente (/patients/:id)",
 "Vista de detalle: \u00faltimas mediciones, gr\u00e1ficos de tendencia, escalas calculadas, riesgos de mortalidad y estancia (ML), y acciones cl\u00ednicas accesibles seg\u00fan rol.", FIG["detalle"])
add_screen("14.6 Edici\u00f3n del expediente (/patients/:id/edit)",
 "Actualizaci\u00f3n de datos demogr\u00e1ficos y diagn\u00f3stico con validaci\u00f3n de campos; cualquier cambio queda registrado en la auditor\u00eda WORM. Los permisos de edici\u00f3n se verifican en servidor.", FIG["editar"])
add_screen("14.7 Registro de medici\u00f3n (/patients/:id/measure)",
 "Pantalla de captura de constantes: al guardar, el backend calcula las cinco escalas y publica el evento SSE. La validaci\u00f3n cl\u00ednica de plausibilidad se aplica antes de persistir.", FIG["medicion"])
add_screen("14.8 Timeline cl\u00ednico (/patients/:id/timeline)",
 "L\u00ednea de tiempo append-only del paciente con fingerprint por evento: mediciones, planes CDS y eventos administrativos quedan encadenados y verificables.", FIG["timeline"])
add_screen("14.9 Decisiones cl\u00ednicas (/cds)",
 "Motor CDS (SPEC-016): planes de cuidado disparados por reglas (motor CEL) seg\u00fan perfil de gravedad y contexto, con recomendaciones accionables y registro de la activaci\u00f3n.", FIG["cds"])
add_screen("14.10 Escalamiento (/escalation)",
 "Centro de alertas y escalamiento: umbrales EWS vencidos, severidad por cama y acci\u00f3n inmediata. Las alertas pueden enviarse por Web Push (VAPID) a los profesionales suscritos.", FIG["escalation"])
add_screen("14.11 Dispositivos y monitores (/devices)",
 "Registro de dispositivos de cama: identificaci\u00f3n, tenant, cama asignada, estado de sincronizaci\u00f3n y \u00faltimo latido. Los monitores se asocian a los drivers MLLP por MSH.3.", FIG["devices"])
add_screen("14.12 Calidad de datos (/data-quality)",
 "Consola de completitud, plausibilidad, integridad referencial y consistencia de scores sobre el dataset; detecci\u00f3n de mediciones fuera de rango y acciones de correcci\u00f3n.", FIG["quality"])
add_screen("14.13 Administraci\u00f3n (/admin)",
 "Gesti\u00f3n de usuarios y roles (RBAC), camas y equipos m\u00e9dicos, con toggles de acceso y panel de operaci\u00f3n de la unidad.", FIG["admin"])
add_screen("14.14 Consola de soporte (/admin/soporte)",
 "Telemetr\u00eda por subsistema (db, ingest, realtime, ml, monitores, audit, backup), acciones idempotentes (reintento de ingesta, reset de circuit breaker, backup, rotaci\u00f3n de modelo, verify de fingerprints) y self-healing autom\u00e1tico.", FIG["soporte"])
add_screen("14.15 Multi-tenancy (/admin/tenants)",
 "Organizaciones aisladas: cada tenant filtra sus pacientes, mediciones y alertas; la impersonaci\u00f3n acotada permite al soporte operar dentro de permisos.", FIG["tenants"])
add_screen("14.16 Auditor\u00eda forense (/admin/audit)",
 "Visor de la cadena WORM: eventos encadenados por prev_hash, lotes firmados y export verificable de integridad. Base de libertad ante auditor\u00edas externas e investigaciones.", FIG["audit"])
add_screen("14.17 Perfil del usuario (/perfil)",
 "Cambio de contrase\u00f1a, alta/baja de MFA TOTP con c\u00f3digos de respaldo y datos de la cuenta; los cambios sensibles quedan auditados.", FIG["perfil"])
add_screen("14.18 Observabilidad: health checks (/obs/health)",
 "El servidor expone el estado operativo en formato JSON: base de datos conectada, cach\u00e9 (opcional), uptime y versi\u00f3n del binario. Lo consumen los probes de Kubernetes, el proxy y las alertas.", FIG["health"])
add_screen("14.19 Observabilidad: m\u00e9tricas Prometheus (/metrics)",
 "El endpoint de m\u00e9tricas alimenta Grafana: latencias y contadores HTTP, m\u00e9tricas de ML, de ingesta y de soporte, en formato Prometheus consumible directamente por los scrapers.", FIG["metrics"])
add_screen("14.20 Contrato OpenAPI 3 (/api/v1/openapi.json)",
 "La API se documenta sola (utoipa): la especificaci\u00f3n JSON describe todos los endpoints con sus esquemas, agrupados en ya conocidos tags funcionais (auth, patients, ingestion, ML, support, sandbox, export, ...).", FIG["openapi"])

# ---------------------------------------------------------------------------
# 15. PRUEBAS
# ---------------------------------------------------------------------------
pruebas = """
<h2 class="chap"><span class="n">15.</span> Pruebas y garant\u00eda de calidad</h2>
<p>La calidad se demuestra en cuatro niveles: unitario, integraci\u00f3n/E2E, carga y fuzzing. Todos se ejecutan como parte del pipeline CI y en local de forma acotada.</p>
<table class="mtable">
<tr><th style="width:22%">Nivel</th><th style="width:34%">Descripci\u00f3n</th><th>N\u00fameros</th></tr>
<tr><td>Unitario (lib)</td><td>Tests de m\u00f3dulos: escalas y vectores de referencia, parser HL7, cifrado, auditor\u00eda, rate limiting, ML.</td><td>82 tests</td></tr>
<tr><td>Integraci\u00f3n HTTP</td><td>E2E in-process con tower::oneshot sobre el router Axum: auth/RBAC, pacientes, mediciones, HL7, FHIR, tenants, WORM.</td><td>128+ tests (SPEC-004, api_tests, hl7_integration)</td></tr>
<tr><td>End-to-end UI (Playwright)</td><td>Flujos reales de navegador: login, pacientes, mediciones, admin.</td><td>20 tests, 0 errores</td></tr>
<tr><td>Carga (k6)</td><td>Scripts de autenticaci\u00f3n, escalas, FHIR, HL7 y m\u00e9tricas.</td><td>5 escenarios</td></tr>
<tr><td>Fuzzing (cargo-fuzz)</td><td>Cobertura de entradas adversariales en JSON de API, parser HL7 y escalas.</td><td>3 targets</td></tr>
<tr><td>Benchmark (criterion)</td><td>Rendimiento de c\u00e1lculo de escalas.</td><td>scale_bench</td></tr>
</table>
<h3>15.1 C\u00f3mo se ejecutan</h3>
<pre class="code"><span class="tag"># Relevant tests del backend (r\u00e1pido)</span>
cargo test -p dmart-server --test api_tests --test hl7_integration
cargo test -p dmart-server --lib

<span class="tag"># Coverage gate (>= 60 % global + por m\u00f3dulos)</span>
cargo llvm-cov -p dmart-server --lib --test api_tests

<span class="tag"># E2E UI</span>
npm run test:e2e          # Playwright contra el servidor levantado

<span class="tag"># Carga</span>
k6 run tests/load/auth.js tests/load/scales.js</pre>
<div class="note"><b>Regla operativa:</b> en este repositorio nunca se ejecuta <code>cargo test --workspace</code> en local porque compila el frontend WASM y el fuzzing (horas); el backend se valida con los targets acotados anteriores (~15 s) y el resto lo resuelve el CI.</div>
<h3>15.2 Especificaci\u00f3n 027 - Gate de cobertura</h3>
<p>SPEC-027 define el gate: cobertura global <b>&gt;= 60 %</b> medida con llvm-cov, verificada por <code>check-coverage-thresholds.sh</code> con umbrales por m\u00f3dulos. El pipeline falla (rojo) si no se alcanza, impidiendo que c\u00f3digo sin probar llegue a producci\u00f3n. El badge del repositorio refleja el \u00faltimo estado del gate.</p>
"""

# ---------------------------------------------------------------------------
# 16. OBSERVABILIDAD
# ---------------------------------------------------------------------------
obs = """
<h2 class="chap"><span class="n">16.</span> Observabilidad, SLOs y operaci\u00f3n</h2>
<h3>16.1 M\u00e9tricas y health checks</h3>
<p>El servidor expone, sobre el mismo puerto, tres contratos de operaci\u00f3n:</p>
<table class="mtable">
<tr><th style="width:22%">Endpoint</th><th>Contenido</th></tr>
<tr><td>/obs/health, /obs/live, /obs/ready</td><td>Liveness/readiness: estado de DB, cach\u00e9 (opcional), uptime y versi\u00f3n. Usado por los probes de Kubernetes y por el proxy.</td></tr>
<tr><td>/metrics</td><td>Prometheus: latencias y contadores HTTP, m\u00e9tricas de ML (<code>ml_inference_*</code>), de ingesta, de soporte (<code>support_actions_total</code>, <code>self_healing_total</code>) y de proceso.</td></tr>
<tr><td>/api/v1/openapi.json</td><td>Contrato OpenAPI 3 de la API para generaci\u00f3n de clientes y verificaci\u00f3n.</td></tr>
</table>
<p>La observabilidad se completa con <b>Grafana</b> (dashboards por dominio), <b>Prometheus</b> y <b>Alertmanager</b> con reglas por directorio de despliegue, m\u00e9tricas de proceso y alertas regionalizadas.</p>
<h3>16.2 SLOs y error budgets (SPEC-050)</h3>
<table class="mtable">
<tr><th style="width:30%">SLO</th><th>Objetivo mensual</th></tr>
<tr><td>Disponibilidad de la API</td><td>99,9 % (error budget ~43 min/mes)</td></tr>
<tr><td>Latencia p95 de endpoints cl\u00ednicos</td><td>&lt; 300 ms internos (sin red)</td></tr>
<tr><td>Entrega SSE de eventos</td><td>P\u00e9rdida de eventos &lt; 0,1 % de los emitidos</td></tr>
<tr><td>Ingesta HL7 sin p\u00e9rdida</td><td>100 % de mensajes v\u00e1lidos persistidos</td></tr>
</table>
<p>Los SLOs alimentan los error budgets mostrados en el panel de escalation; si el budget se agota entran en acci\u00f3n las alertas de escalamiento operativo.</p>
<h3>16.3 Operaci\u00f3n y runbook</h3>
<ul>
<li><b>Arranque:</b> validar variables cr\u00edticas, secretos, iniciar <code>dmart-server</code> y comprobar los logs (DB conectada, listeners, servidor en :3000).</li>
<li><b>Apagado seguro:</b> captura de SIGTERM con timeout configurable, flush de m\u00e9tricas, cierre de DB y cach\u00e9.</li>
<li><b>Backup/DR:</b> scripts <code>backup.sh</code>, <code>dr_backup.sh</code>, <code>dr_restore.sh</code>, <code>dr_verify.sh</code> y simulacros <code>dr_drill.sh</code> (restore probado).</li>
<li><b>Consola de soporte:</b> subsistemas, acciones y self-healing descritos en 14.14.</li>
</ul>
"""

# ---------------------------------------------------------------------------
# 17. DEPLOY
# ---------------------------------------------------------------------------
deploy = """
<h2 class="chap"><span class="n">17.</span> Despliegue, GitOps y continuidad</h2>
<h3>17.1 Build multi-stage</h3>
<p>El pipeline produce una imagen de contenedor multi-stage: etapa de compilaci\u00f3n Rust (host), build del frontend WASM (Trunk) y etapa final slim con el \u00fanico binario y el <code>dist/</code> incrustado. La imagen se firma con <b>cosign keyless</b> y se escanea con <code>cargo deny</code>/<code>cargo audit</code> en CI.</p>
<h3>17.2 Kubernetes / Helm</h3>
<table class="mtable">
<tr><th style="width:26%">Recurso</th><th>Funci\u00f3n</th></tr>
<tr><td>Deployment dmart-server</td><td>Replicas del API+frontend con HPA, probes de liveness/readiness, PDB e Ingress TLS.</td></tr>
<tr><td>StatefulSet SurrealDB</td><td>Con o sin cluster de 3 nodos (SPEC-023) y volumen persistente.</td></tr>
<tr><td>NetworkPolicy / ServiceAccount</td><td>Segregaci\u00f3n de red y m\u00ednimo privilegio.</td></tr>
<tr><td>Secrets</td><td>JWT_SECRET, DMART_MASTER_KEY, claves VAPID y CA de monitores como secretos del cluster.</td></tr>
</table>
<h3>17.3 GitOps (ArgoCD / Flux)</h3>
<p>El repositorio incluye manifiestos <b>ArgoCD</b> (AppProject + Application) y <b>Flux</b> (GitRepository + HelmRelease + Kustomization). El flujo es: cambio en el repo -> CI verde -> imagen firmada -> GitOps sincroniza el cluster. Los scripts <code>deploy_blue_green.sh</code>, <code>deploy_canary.sh</code> y <code>deploy_rollback.sh</code> implementan estrategias de liberaci\u00f3n con verdes/azules y canario con regresi\u00f3n r\u00e1pida.</p>
<h3>17.4 Entornos</h3>
<p>Se mantienen tres perfiles de configuraci\u00f3n (dev, staging, prod) con variables separadas. En prod: CORS cerrado, cookies seguras, HSTS, MFA obligatorio, rate limit distribuido (Valkey) e ingesta MLLP con mTLS y fail-closed. En desarrollo se usan desactivaciones expl\u00edcitas (rate limit, throttles) marcadas como tales.</p>
"""

# ---------------------------------------------------------------------------
# 18. VULNERABILIDADES
# ---------------------------------------------------------------------------
vuln = """
<h2 class="chap"><span class="n">18.</span> An\u00e1lisis de vulnerabilidades y sem&#225;ntica de seguridad</h2>
<h3>18.1 Postura de seguridad actual</h3>
<table class="mtable">
<tr><th style="width:24%">Pilar</th><th style="width:44%">Implementado</th><th>Pendiente / plan</th></tr>
<tr><td>AuthN</td><td>Argon2id, JWT HS256 revocable (access 15 min / refresh 7 d\u00edas), MFA TOTP RFC 6238, JWT_SECRET en Zeroizing.</td><td>Thelogy operativa de adopci\u00f3n de MFA en todos los usuarios.</td></tr>
<tr><td>AuthZ</td><td>RBAC Admin/M\u00e9dico/Enfermero/Viewer/Soporte; ResourceOwner y verificaci\u00f3n de tenant en cada acceso.</td><td>Auditor\u00eda peri\u00f3dica de asignaci\u00f3n de roles.</td></tr>
<tr><td>Cifrado en reposo</td><td>AES-256-GCM envelope DMART_A2 + auto-detecci\u00f3n DMART_V1 legacy; subclaves HMAC-SHA256 (PHI/INDEX); \u00edndices ciegos.</td><td>Backfill completo de filas legacy (P0.2).</td></tr>
<tr><td>B\u00fasqueda de PHI</td><td>Trigramas ciegos HMAC + ranking Jaro-Winkler en Rust, sin texto en disco.</td><td>Ampliar cobertura a otros campos PHI.</td></tr>
<tr><td>MLLP</td><td>mTLS TLS 1.3, pinning SHA-256 DER a MSH.3, allowlist de emisores, fail-closed.</td><td>Monitorizaci\u00f3n de revocaciones por CA.</td></tr>
<tr><td>Rate limit</td><td>Trait RateLimitStore, Valkey/Redis INCR+EXPIRE, tenant isolation por claim JWT, headers X-RateLimit-*.</td><td>WAF perimetral en el gateway.</td></tr>
<tr><td>Supply chain</td><td>cargo deny + cargo audit, cosign keyless, gitleaks SARIF.</td><td>Rotaci\u00f3n de PAT de GitHub.</td></tr>
<tr><td>Auditor\u00eda</td><td>Cadena WORM SHA-256, lotes HMAC, mutex async, export verificable.</td><td>Notarizaci\u00f3n temporal externa (timestamping).</td></tr>
</table>
<h3>18.2 Superficie de ataque y riesgos residuales</h3>
<table class="mtable">
<tr><th style="width:28%">Riesgo</th><th style="width:36%">Exposici\u00f3n</th><th>Mitigaci\u00f3n / recomendaci\u00f3n</th></tr>
<tr><td>AES-NI sin m\u00f3dulo FIPS</td><td>El crate aes-gcm no tiene m\u00f3dulo validado FIPS 140-3.</td><td>Si el requisito es FIPS, sustituir el proveedor (ej. openssl/rustls-fips).</td></tr>
<tr><td>Refresh token en cookie</td><td>Robo de cookie (malware de terminal).</td><td>SameSite=Strict, httpOnly, HTTPS obligatorio, rotaci\u00f3n en cada uso.</td></tr>
<tr><td>Ingesta HL7 maliciosa</td><td>Valores extremos o mensajes infinitos.</td><td>Validaci\u00f3n de plausibilidad, idempotencia, rate limit, timeouts, tope de conexiones, circuit breaker.</td></tr>
<tr><td>XSS en interfaz</td><td>Datos de pacientes renderizados.</td><td>Leptos escapa por defecto el contenido; CSP activa; sin <code>innerHTML</code> din\u00e1mico con PHI.</td></tr>
<tr><td>Dependencias</td><td>CVEs en crates.</td><td>cargo audit en CI, pinning y policy scan con cargo deny.</td></tr>
<tr><td>Operador humano</td><td>Errores de configuraci\u00f3n (secretos d\u00e9biles).</td><td>validate_secret_strength fail-closed + runbooks de arranque.</td></tr>
</table>
<div class="warn"><b>Conclusi\u00f3n de riesgo:</b> el dise\u00f1o prioriza la confidencialidad de la PHI (cifrado de extremo a extremo interno, \u00edndices ciegos, auditor\u00eda inmutable) y la integridad de la ingesta. Los riesgos residuales son operativos y de cadena de suministro, con controles de mitigaci\u00f3n planificados en el roadmap de seguridad.</div>
"""

# ---------------------------------------------------------------------------
# 19. NORMATIVA
# ---------------------------------------------------------------------------
norma = """
<h2 class="chap"><span class="n">19.</span> Cumplimiento normativo y evidencia</h2>
<h3>19.1 HIPAA</h3>
<table class="mtable">
<tr><th style="width:34%">Requisito HIPAA</th><th>Evidencia en dMart UCI</th></tr>
<tr><td>Confidencialidad / cifrado</td><td>PHI en reposo AES-256-GCM (envelope DMART_A2), tr\u00e1nsito TLS, mTLS en MLLP.</td></tr>
<tr><td>Integridad</td><td>Cadenas WORM, fingerprint de timeline, firmas HMAC de lotes, validaci\u00f3n de plausibilidad.</td></tr>
<tr><td>Disponibilidad</td><td>DR con scripts de backup/restore probados, cluster SnDD, SLOs con error budget.</td></tr>
<tr><td>Auditor\u00eda de accesos</td><td>Registro WORM encadenado de eventos sensibles y export verifiable.</td></tr>
<tr><td>\u00daltimo uso / notificaci\u00f3n</td><td>Tracker de accesos en auditor\u00eda y consola de soporte.</td></tr>
</table>
<h3>19.2 ISO/IEC 27001</h3>
<p>El evidence pack (<code>docs/compliance/</code>) cataloga controles aplicables: A.5 (pol\u00edticas), A.8 (gesti\u00f3n de activos y PHI), A.9 (control de acceso: RBAC, m\u00ednimo privilegio), A.10 (criptograf\u00eda), A.12 (operaciones: backup, logs, vuln management) y A.14 (desarrollo seguro: SDD, CI, revisi\u00f3n). Los scripts <code>compliance_generate.sh</code>/<code>compliance_check.sh</code> generan y verifican el pack como parte de CI.</p>
<h3>19.3 FDA SaMD y protecci\u00f3n de datos</h3>
<ul>
<li><b>FDA Software as a Medical Device:</b> la l\u00f3gica de c\u00f3mputo de escalas est\u00e1 en un crate de dominio validado por vectores de referencia cl\u00ednica y tests de conformidad, sentando la base de un expediente de validaci\u00f3n de software.</li>
<li><b>GDPR / LOPD:</b> minimizaci\u00f3n (solo PHI necesaria), cifrado, \u00edndices ciegos, derecho de acceso mediante export, retenci\u00f3n definida (auditor\u00eda 6 a\u00f1os) y registro de decisiones automatizadas (ML) expuesto y explicable.</li>
</ul>
<div class="ok"><b>Dato relevante:</b> las especificaciones 034 y 039 definen el pack de evidencia (control catalog, data flows, incidentes, marco legal y backfill de PHI) de modo que la evidencia normativa se genera y se valida de forma automatizada.</div>
"""

# ---------------------------------------------------------------------------
# 20. CONCLUSIONES
# ---------------------------------------------------------------------------
conclusiones = """
<h2 class="chap"><span class="n">20.</span> Conclusiones y roadmap</h2>
<h3>20.1 Resumen de capacidades en n\u00fameros</h3>
<table class="mtable">
<tr><th style="width:48%">M\u00e9trica</th><th>Valor</th></tr>
<tr><td>Especificaciones SDD completadas</td><td>46 specs (SPEC-001 a SPEC-052, 4 fases)</td></tr>
<tr><td>Pruebas backend (gate)</td><td>210 (82 unitarias + 128 de integraci\u00f3n)</td></tr>
<tr><td>Tests E2E UI (Playwright)</td><td>20, 0 errores</td></tr>
<tr><td>Gate de cobertura</td><td>&gt;= 60 % global con umbrales por m\u00f3dulo</td></tr>
<tr><td>Fuzz targets</td><td>3 (API JSON, parser HL7, escalas)</td></tr>
<tr><td>Rutas de la SPA</td><td>17 protegidas + salud/m\u00e9tricas/OpenAPI</td></tr>
<tr><td>Escalas de gravedad</td><td>5 (APACHE II, GCS, NEWS2, SOFA, SAPS III)</td></tr>
<tr><td>Modelos ML</td><td>Ensemble de mortalidad, red LOS, similitud, forecaster</td></tr>
<tr><td>Migraciones de esquema</td><td>27 versionadas (SurrealQL)</td></tr>
<tr><td>Tama\u00f1o del binario (debug)</td><td>~760 MB; release con LTO/opt-z minimizado</td></tr>
<tr><td>M\u00f3dulo WASM del frontend</td><td>~3,2 MB compilado con Trunk</td></tr>
</table>
<h3>20.2 Logros frente a los requisitos del hospital aislado</h3>
<ul>
<li>Despliegue autocontenido: un binario + archivo de base de datos local; PWA instalable para terminales.</li>
<li>Interoperabilidad real: HL7 v2/MLLP para monitores y FHIR R4 para el resto del ecosistema hospitalario.</li>
<li>Vigilancia continua: EWS en streaming con notificaci\u00f3n push.</li>
<li>Calidad cl\u00ednica: escalas validadas por vectores de referencia y ML explicable.</li>
<li>Gobernanza: RBAC, cifrado de PHI, \u00edndices ciegos y auditor\u00eda WORM verificable.</li>
</ul>
<h3>20.3 Pr\u00f3ximos pasos del roadmap</h3>
<ol>
<li>Completar el backfill de filas PHI legacy (P0.2) y rotaci\u00f3n de claves con drill documentado.</li>
<li>Validaci\u00f3n FIPS 140-3 del proveedor criptogr\u00e1fico si lo exige el cliente.</li>
<li>Ampliar la cobertura de b\u00fasqueda difusa a otros campos cl\u00ednicos.</li>
<li>Notarizaci\u00f3n de la cadena de auditor\u00eda con sellado de tiempo externo.</li>
<li>Integrar WAF perimetral y m\u00e9tricas de superficie de ataque en el panel de escalamiento.</li>
<li>Certificaci\u00f3n formal del evidence pack (ISO 27001 / HIPAA) con auditor\u00eda de terceros.</li>
</ol>
<h3>20.4 Aviso final</h3>
<p>Este manual se gener\u00f3 desde el c\u00f3digo real y las especificaciones del repositorio, con capturas del sistema en ejecuci\u00f3n sobre un dataset de demostraci\u00f3n. La documentaci\u00f3n t\u00e9cnica fiel al c\u00f3digo es el primer requisito de la metodolog\u00eda SDD: <b>especificaci\u00f3n, implementaci\u00f3n, prueba y evidencia van siempre juntas.</b></p>
"""

# ---------------------------------------------------------------------------
# TOC (pagina fija despues de portada)
# ---------------------------------------------------------------------------
toc = """
<h2 class="chap"><span class="n">\u00cdndice</span> Contenido</h2>
<div class="toc">
  <div class="trow"><span class="t">1. Sobre este manual y datos del producto</span><span class="p">2</span></div>
  <div class="trow"><span class="t">2. Resumen ejecutivo y glosario</span><span class="p">3</span></div>
  <div class="trow"><span class="t">3. Contexto y problema cl\u00ednico</span><span class="p">4</span></div>
  <div class="trow"><span class="t">4. Metodolog\u00eda Spec-Driven Development (SDD)</span><span class="p">5</span></div>
  <div class="trow"><span class="t">5. Stack tecnol\u00f3gico y conceptos aplicados</span><span class="p">6</span></div>
  <div class="trow l2"><span class="t">5.1 Rust \u00b7 5.2 WebAssembly \u00b7 5.3 Leptos \u00b7 5.4 Axum \u00b7 5.5 SurrealDB \u00b7 5.6 SSE \u00b7 5.7 PWA \u00b7 5.8 Otras piezas</span><span class="p">6</span></div>
  <div class="trow"><span class="t">6. Arquitectura del sistema</span><span class="p">8</span></div>
  <div class="trow"><span class="t">7. Modelo de datos y protecci\u00f3n de PHI</span><span class="p">9</span></div>
  <div class="trow"><span class="t">8. Autenticaci\u00f3n, autorizaci\u00f3n y RBAC</span><span class="p">10</span></div>
  <div class="trow"><span class="t">9. Seguridad del per\u00edmetro y capas de defensa</span><span class="p">11</span></div>
  <div class="trow"><span class="t">10. Algoritmos cl\u00ednicos: escalas de gravedad</span><span class="p">12</span></div>
  <div class="trow l2"><span class="t">10.1 APACHE II \u00b7 10.2 GCS \u00b7 10.3 NEWS2 \u00b7 10.4 SOFA \u00b7 10.5 SAPS III</span><span class="p">12</span></div>
  <div class="trow"><span class="t">11. Inteligencia artificial y machine learning</span><span class="p">14</span></div>
  <div class="trow"><span class="t">12. Interoperabilidad y tiempo real</span><span class="p">15</span></div>
  <div class="trow"><span class="t">13. Auditor\u00eda WORM, timeline, b\u00fasqueda y calidad</span><span class="p">16</span></div>
  <div class="trow"><span class="t">14. Pantallas del sistema (capturas reales)</span><span class="p">17</span></div>
  <div class="trow l2"><span class="t">14.01 Acceso \u00b7 14.02 Panel \u00b7 14.03 Pacientes \u00b7 14.04 Registro \u00b7 14.05 Expediente \u00b7 14.06 Edici\u00f3n</span><span class="p">17</span></div>
  <div class="trow l2"><span class="t">14.07 Medici\u00f3n \u00b7 14.08 Timeline \u00b7 14.09 CDS \u00b7 14.10 Escalamiento \u00b7 14.11 Dispositivos \u00b7 14.12 Calidad</span><span class="p">18</span></div>
  <div class="trow l2"><span class="t">14.13 Administraci\u00f3n \u00b7 14.14 Soporte \u00b7 14.15 Tenants \u00b7 14.16 Auditor\u00eda \u00b7 14.17 Perfil \u00b7 14.18 Observabilidad</span><span class="p">19</span></div>
  <div class="trow"><span class="t">15. Pruebas y garant\u00eda de calidad</span><span class="p">20</span></div>
  <div class="trow"><span class="t">16. Observabilidad, SLOs y operaci\u00f3n</span><span class="p">21</span></div>
  <div class="trow"><span class="t">17. Despliegue, GitOps y continuidad</span><span class="p">22</span></div>
  <div class="trow"><span class="t">18. An\u00e1lisis de vulnerabilidades y sem\u00e1ntica de seguridad</span><span class="p">23</span></div>
  <div class="trow"><span class="t">19. Cumplimiento normativo y evidencia</span><span class="p">24</span></div>
  <div class="trow"><span class="t">20. Conclusiones y roadmap</span><span class="p">25</span></div>
</div>
<div class="note" style="margin-top:8mm"><b>Nota:</b> el n\u00famero de p\u00e1ginas indicado es orientativo; el documento completo supera las 30 p\u00e1ginas incluyendo todas las capturas de pantalla a tama\u00f1o completo, tablas de datos y diagramas de flujo.</div>
"""

# ---------------------------------------------------------------------------
# Ensamblado
# ---------------------------------------------------------------------------
def build():
    body_parts = []
    # cover (page 1) — sin otro contenido en la misma hoja
    body_parts.append(cover)
    body_parts.append(p1)
    body_parts.append(toc)
    body_parts.append(glosario)
    body_parts.append(problema)
    body_parts.append(sdd)
    body_parts.append(stack)
    body_parts.append(arquitectura)
    body_parts.append(datos)
    body_parts.append(auth)
    body_parts.append(perim)
    body_parts.append(algo)
    body_parts.append(ml)
    body_parts.append(inter)
    body_parts.append(audit)
    body_parts.append(screens_intro)
    body_parts.append(''.join(screens))
    body_parts.append(pruebas)
    body_parts.append(obs)
    body_parts.append(deploy)
    body_parts.append(vuln)
    body_parts.append(norma)
    body_parts.append(conclusiones)

    html_doc = f"""<!DOCTYPE html>
<html lang="es"><head><meta charset="UTF-8">
<style>{CSS}</style>
</head><body>
{''.join(body_parts)}
</body></html>"""
    out = os.path.join(OUT, "manual_dmart.html")
    with open(out, "w", encoding="utf-8") as f:
        f.write(html_doc)
    print("written", out, len(html_doc), "chars")

if __name__ == "__main__":
    build()