//! P0.3 — Gate barato de i18n del frontend.
//!
//! Sustituye a "compilar el WASM" como forma de verificar el bundle de
//! traducciones. Reimplementa (a propósito, sin importar `dmart-app`, que es
//! un crate WASM) el parser de `dmart-app/src/i18n.rs` y comprueba:
//!
//! 1. Los 4 `.ftl` tienen el MISMO conjunto de claves, en el MISMO orden.
//! 2. Cada `crate::i18n::tr("clave", …)` del código existe en los 4 locales.
//! 3. Los placeholders `{x}` de los `.ftl` se sustituyen de verdad: la
//!    interpolación usa llaves SIMPLES (los `.ftl` no son Fluent) y toda clave
//!    con placeholders recibe `args` en al menos un punto de llamada.
//! 4. No queda hardcodeo en español en las 4 páginas migradas (P0.3).

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

const LOCALES: [&str; 4] = ["es", "en", "pt", "fr"];

/// Las 4 páginas del ítem P0.3: aquí el hardcodeo en español es un fallo.
const TARGET_PAGES: [&str; 4] = ["tenants.rs", "cds.rs", "audit.rs", "patient_timeline.rs"];

/// Literales que el ítem P0.3 decide NO traducir (tokens técnicos, cabeceras de
/// contrato de datos y separadores). Todo lo demás con español debe ser clave.
const ALLOWED_RAW: &[(&str, &str)] = &[
    ("cds.rs", "v"),
    ("cds.rs", "Fingerprint"),
    ("audit.rs", "Timestamp"),
    ("audit.rs", "IP"),
    ("audit.rs", "Hash"),
    ("audit.rs", "Fingerprint"),
    ("audit.rs", "Head batch hash"),
    ("audit.rs", "/"),
    ("patient_timeline.rs", "..."),
];

// ---------------------------------------------------------------------------
// Rutas
// ---------------------------------------------------------------------------

fn server_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn repo_root() -> PathBuf {
    server_dir()
        .parent()
        .expect("dmart-server debe vivir dentro del workspace")
        .to_path_buf()
}

fn locales_dir() -> PathBuf {
    repo_root().join("dmart-app/locales")
}

fn app_src_dir() -> PathBuf {
    repo_root().join("dmart-app/src")
}

// ---------------------------------------------------------------------------
// Parser: misma algoritmo que `dmart-app/src/i18n.rs::init_i18n`
// ---------------------------------------------------------------------------

/// Devuelve los pares `(clave, valor)` en el orden del fichero.
fn parse_ftl(path: &Path) -> Vec<(String, String)> {
    let content = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("i18n: no se pudo leer {}: {e}", path.display()));
    let mut out = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let value = value.trim();
            let value = if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
                &value[1..value.len() - 1]
            } else {
                value
            };
            out.push((key.trim().to_string(), value.to_string()));
        }
    }
    out
}

fn bundles() -> BTreeMap<String, Vec<(String, String)>> {
    LOCALES
        .iter()
        .map(|l| {
            let p = locales_dir().join(format!("{l}.ftl"));
            assert!(
                p.exists(),
                "i18n: falta el locale {}.ftl en {}",
                l,
                locales_dir().display()
            );
            ((*l).to_string(), parse_ftl(&p))
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Interpolación: espejo de `i18n.rs::substitute`
// ---------------------------------------------------------------------------

fn substitute(value: &str, args: &BTreeMap<String, String>) -> String {
    let mut result = value.to_string();
    for (k, v) in args {
        // Llaves SIMPLES. Con `{{`/`}}` (sintaxis Fluent) esto no sustituye y
        // el placeholder se queda literal en pantalla.
        result = result.replace(&format!("{{{k}}}"), v);
    }
    result
}

fn placeholders(value: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut rest = value;
    while let Some(start) = rest.find('{') {
        rest = &rest[start + 1..];
        let Some(end) = rest.find('}') else { break };
        let name = &rest[..end];
        if !name.is_empty() && !name.contains('{') && !name.contains(' ') {
            out.insert(name.to_string());
        }
        rest = &rest[end + 1..];
    }
    out
}

// ---------------------------------------------------------------------------
// Escaneo del código fuente
// ---------------------------------------------------------------------------

fn rust_files(dir: &Path, acc: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("i18n: no se pudo leer {}: {e}", dir.display()));
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, acc);
        } else if path.extension().is_some_and(|e| e == "rs") {
            acc.push(path);
        }
    }
}

/// Una llamada a `tr`: clave + si el segundo argumento es `None`.
#[derive(Debug)]
struct TrCall {
    key: String,
    passes_args: bool,
    file: String,
    line: usize,
}

fn find_from(haystack: &str, needle: &str, from: usize) -> Option<usize> {
    haystack
        .get(from..)
        .and_then(|s| s.find(needle))
        .map(|i| i + from)
}

/// Devuelve el texto entre los paréntesis de la llamada que empieza en `open`
/// (índice justo después de `i18n::tr(`), respetando literales y anidamiento.
fn call_body(content: &str, open: usize) -> Option<(String, usize)> {
    let bytes = content.as_bytes();
    let mut depth = 1usize;
    let mut i = open;
    let mut in_str = false;
    while i < bytes.len() {
        let c = bytes[i];
        if in_str {
            if c == b'"' {
                in_str = false;
            }
        } else if c == b'"' {
            in_str = true;
        } else if c == b'(' {
            depth += 1;
        } else if c == b')' {
            depth -= 1;
            if depth == 0 {
                return Some((content[open..i].to_string(), i + 1));
            }
        }
        i += 1;
    }
    None
}

fn split_top_level(body: &str) -> Vec<&str> {
    let bytes = body.as_bytes();
    let mut parts = Vec::new();
    let (mut depth, mut in_str, mut start) = (0usize, false, 0usize);
    for (i, b) in bytes.iter().enumerate() {
        let c = *b;
        if in_str {
            if c == b'"' {
                in_str = false;
            }
            continue;
        }
        match c {
            b'"' => in_str = true,
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b',' if depth == 0 => {
                parts.push(&body[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&body[start..]);
    parts
}

fn scan_tr_calls() -> Vec<TrCall> {
    let mut files = Vec::new();
    rust_files(&app_src_dir(), &mut files);
    files.sort();

    let mut out = Vec::new();
    for path in files {
        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let rel = path
            .strip_prefix(app_src_dir())
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let file = rel.rsplit('/').next().unwrap_or(&rel).to_string();

        let mut cursor = 0usize;
        while let Some(at) = find_from(&content, "i18n::tr(", cursor) {
            let after = at + "i18n::tr(".len();
            cursor = after;
            let Some((body, end)) = call_body(&content, after) else {
                continue;
            };
            cursor = end;
            let parts = split_top_level(&body);
            let Some(first) = parts.first() else { continue };
            let key = first.trim().trim_matches('"').to_string();
            if key.is_empty() {
                continue;
            }
            let second = parts.get(1).map(|s| s.trim()).unwrap_or("None");
            let line = content[..at].lines().count().max(1);
            out.push(TrCall {
                key,
                passes_args: second != "None",
                file: file.clone(),
                line,
            });
        }
    }
    out
}

/// Marcas inequívocas: tildes, eñes y signos de apertura del español.
const SPANISH_CHARS: [char; 11] = ['á', 'é', 'í', 'ó', 'ú', 'ñ', '¿', '¡', 'Á', 'É', 'Í'];

/// Palabras frequentemente hardcodeadas que NO llevan tilde, así que las tildes
/// solas no bastan (`"Tabla de planes"`, `"Nuevo tenant"`, `"Todos"`…).
/// Lista conservadora: palabras casi siempre traducibles en esta UI.
const SPANISH_WORDS: [&str; 35] = [
    "tabla",
    "total",
    "plan",
    "todos",
    "todas",
    "nuevo",
    "nueva",
    "nombre",
    "buscar",
    "paciente",
    "pacientes",
    "estado",
    "evento",
    "eventos",
    "nota",
    "ingreso",
    "egreso",
    "cargando",
    "sin",
    "accion",
    "copiar",
    "generado",
    "confirmar",
    "eliminar",
    "editar",
    "guardar",
    "resultado",
    "resultados",
    "siguiente",
    "anterior",
    "volver",
    "hospital",
    "medicion",
    "mediciones",
    "auditoria",
];

/// Terminaciones operatoras del español, para los hardcodes que se escribieron
/// **sin tilde**: `"Criticos"`, `"Estadisticas"`, `"Distribucion"`, `"Promedios"`.
/// Sin esto el detector original perdía la mitad de la deuda real, que es
/// exactamente la que sirve de línea base.
const SPANISH_SUFFIXES: [&str; 18] = [
    "cion", "ciones", "ico", "ica", "icos", "icas", "ivo", "iva", "ivos", "ivas", "dad", "ista",
    "istas", "logia", "grafia", "metro", "sico", "sica",
];

fn looks_spanish(s: &str) -> bool {
    if SPANISH_CHARS.iter().any(|c| s.contains(*c)) {
        return true;
    }
    // Normaliza acentos y signos para que `acción` cuente como `accion`.
    let lower = s.to_lowercase();
    let normalized: String = lower
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'ä' => 'a',
            'é' | 'è' => 'e',
            'í' | 'ï' => 'i',
            'ó' | 'ò' | 'ö' => 'o',
            'ú' | 'ù' | 'ü' => 'u',
            'ñ' => 'n',
            'ç' => 'c',
            _ => c,
        })
        .collect();
    let words: Vec<&str> = normalized
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    SPANISH_WORDS.iter().any(|needle| words.contains(needle))
}

/// [`looks_spanish`] + morfoxlogía por terminación. Más falsos positivos (falla
/// hacia el lado de "esto parece español"), que es lo correcto para un ratchet:
/// subir la línea base de más no rompe nada, subir la de menos sí.
fn looks_spanish_wide(s: &str) -> bool {
    if looks_spanish(s) {
        return true;
    }
    let normalized: String = s
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'ä' => 'a',
            'é' | 'è' => 'e',
            'í' | 'ï' => 'i',
            'ó' | 'ò' | 'ö' => 'o',
            'ú' | 'ù' | 'ü' => 'u',
            'ñ' => 'n',
            'ç' => 'c',
            _ => c,
        })
        .collect();
    normalized
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() >= 5)
        .any(|w| SPANISH_SUFFIXES.iter().any(|s| w.ends_with(s)))
}

// ---------------------------------------------------------------------------
// Deuda de i18n del resto de la app (RATCHET)
// ---------------------------------------------------------------------------
//
// Las 4 páginas de `TARGET_PAGES` están migradas y su test es una puerta dura.
// Las otras 12 no lo están: 375 literales en español medidos el 3 de octubre
// (el plan estimaba 2 h para 4 páginas; la deuda real es ~4× eso y son las
// páginasviejas, no las nuevas).
//
// Migrarlas todas son varias sesiones de trabajo mecánico. Lo que NO puede pasar
// es que la cifraCREZCA mientras tanto: por eso esto es un ratchet, no una
// puerta. Cada página migrada baja su línea base en la misma commit.

/// Nº máximo de literales en español por página. Medición del 3 de octubre.
const I18N_DEBT_BASELINE: &[(&str, usize)] = &[
    ("admin.rs", 37),
    ("patient_edit.rs", 28),
    ("patient_detail.rs", 28),
    ("register.rs", 28),
    ("dashboard.rs", 23),
    ("perfil.rs", 24),
    ("escalation.rs", 16),
    ("devices.rs", 13),
    ("support.rs", 11),
    ("audit.rs", 0),
    ("cds.rs", 0),
    ("data_quality.rs", 0),
    ("login.rs", 0),
    ("measurement.rs", 0),
    ("mod.rs", 0),
    ("patient_timeline.rs", 0),
    ("patients.rs", 0),
    ("tenants.rs", 0),
];

/// Valores que se quedan en crudo a propósito: identificadores del contrato de
/// datos, no texto de interfaz. Traducirlos rompería la API.
const ALLOWED_RAW_BY_PAGE: &[(&str, &str, &str)] = &[
    (
        "cds.rs",
        "cds-col-plan",
        "clave de columna del contrato de datos",
    ),
    (
        "tenants.rs",
        "tenants-col-total",
        "clave de columna del contrato de datos",
    ),
    (
        "patients.rs",
        "activos",
        "valor del filtro de estado que viaja a la API",
    ),
    (
        "patients.rs",
        "todos",
        "valor del filtro de estado que viaja a la API",
    ),
    (
        "admin.rs",
        "institucion",
        "clave de tab interna, no texto de interfaz",
    ),
    ("admin.rs", "auditoria", "clave de tab interna"),
    (
        "admin.rs",
        "fa-solid fa-hospital mr-2",
        "clase CSS de icono",
    ),
    (
        "admin.rs",
        "VentiladorMecanico",
        "valor de TipoEquipo que viaja a la API",
    ),
    ("admin.rs", "Activo", "valor de EstadoEquipo"),
    ("admin.rs", "Inactivo", "valor de EstadoEquipo"),
    ("admin.rs", "Reparacion", "valor de EstadoEquipo"),
    ("admin.rs", "Medico", "valor de RolPersonal"),
    ("admin.rs", "todos", "valor del filtro de rol"),
    (
        "admin.rs",
        "Pediatrica",
        "valor de TipoCama que viaja a la API",
    ),
    ("admin.rs", "/admin/institucion", "ruta del endpoint"),
    (
        "admin.rs",
        "Monitor",
        "valor de TipoEquipo en parse_tipo_equipo",
    ),
    (
        "admin.rs",
        "Computador",
        "valor de TipoEquipo en parse_tipo_equipo",
    ),
    (
        "admin.rs",
        "BombaInfusion",
        "valor de TipoEquipo en parse_tipo_equipo",
    ),
    (
        "admin.rs",
        "Bomba de Infusión",
        "valor de TipoEquipo en parse_tipo_equipo",
    ),
    (
        "admin.rs",
        "Ventilador Mecánico",
        "valor de TipoEquipo en parse_tipo_equipo",
    ),
    (
        "admin.rs",
        "Mantenimiento",
        "valor de EstadoEquipo en parse_estado_equipo",
    ),
    (
        "admin.rs",
        "En Mantenimiento",
        "valor de EstadoEquipo en parse_estado_equipo",
    ),
    (
        "admin.rs",
        "En Reparación",
        "valor de EstadoEquipo en parse_estado_equipo",
    ),
    ("admin.rs", "General", "valor por defecto de form_tipo cama"),
    ("admin.rs", "Libre", "valor por defecto de form_estado cama"),
    (
        "admin.rs",
        "VentiladorMecanico",
        "valor por defecto de form_tipo equipo",
    ),
    (
        "admin.rs",
        "Activo",
        "valor por defecto de form_estado equipo",
    ),
    ("admin.rs", "Medico", "valor por defecto de form_rol staff"),
    (
        "admin.rs",
        "Aislamiento",
        "valor de TipoCama en option value",
    ),
    (
        "admin.rs",
        "Pediatrica",
        "valor de TipoCama en option value",
    ),
    ("admin.rs", "Coronaria", "valor de TipoCama en option value"),
    ("admin.rs", "Quemados", "valor de TipoCama en option value"),
    ("admin.rs", "Ocupada", "valor de EstadoCama en option value"),
    (
        "admin.rs",
        "Limpieza",
        "valor de EstadoCama en option value",
    ),
    ("admin.rs", "Monitor", "valor de TipoEquipo en option value"),
    (
        "admin.rs",
        "Computador",
        "valor de TipoEquipo en option value",
    ),
    (
        "admin.rs",
        "BombaInfusion",
        "valor de TipoEquipo en option value",
    ),
    (
        "admin.rs",
        "Mantenimiento",
        "valor de EstadoEquipo en option value",
    ),
    (
        "admin.rs",
        "Inactivo",
        "valor de EstadoEquipo en option value",
    ),
    (
        "admin.rs",
        "Reparacion",
        "valor de EstadoEquipo en option value",
    ),
    ("admin.rs", "Admin", "valor de RolPersonal en option value"),
    (
        "admin.rs",
        "Enfermero",
        "valor de RolPersonal en option value",
    ),
    ("admin.rs", "Viewer", "valor de RolPersonal en option value"),
    (
        "admin.rs",
        "Soporte",
        "valor de RolPersonal en option value",
    ),
    ("admin.rs", "Cama", "formato de número de cama"),
    ("admin.rs", "Nombre completo", "placeholder de input"),
    (
        "admin.rs",
        "institucion",
        "clave de tab interna en set_active_tab",
    ),
    ("admin.rs", "/admin/institucion", "ruta de endpoint API"),
    (
        "admin.rs",
        "Activo",
        "valor de EstadoEquipo/EstadoCama detectado por sufijo -ivo",
    ),
    (
        "admin.rs",
        "Inactivo",
        "valor de EstadoEquipo detectado por sufijo -ivo",
    ),
    (
        "admin.rs",
        "Reparacion",
        "valor de EstadoEquipo detectado por sufijo -ción",
    ),
    (
        "admin.rs",
        "Medico",
        "valor de RolPersonal detectado por sufijo -ico",
    ),
    (
        "admin.rs",
        "Pediatrica",
        "valor de TipoCama detectado por sufijo -ica",
    ),
    (
        "admin.rs",
        "auditoria",
        "clave de tab interna en set_active_tab",
    ),
    ("admin.rs", "camas", "clave de tab interna"),
    ("admin.rs", "equipos", "clave de tab interna"),
    ("admin.rs", "staff", "clave de tab interna"),
    (
        "admin.rs",
        "Ventilador Mecánico",
        "valor de TipoEquipo en parse_tipo_equipo",
    ),
    (
        "admin.rs",
        "adm-audit-filter-todas",
        "clave de traducción en AUDIT_ACTIONS",
    ),
    (
        "admin.rs",
        "Ventilador MecÃ¡nico",
        "valor de TipoEquipo con codificación UTF-8 en parse_tipo_equipo",
    ),
    (
        "patient_edit.rs",
        "Electiva",
        "valor de TipoAdmision que viaja a la API",
    ),
    (
        "patient_detail.rs",
        "fa-solid fa-hospital text-xs",
        "clase CSS de icono",
    ),
    ("patient_detail.rs", "fa-hospital", "prop icon= de InfoRow"),
    ("register.rs", "Electiva", "valor de TipoAdmision"),
    (
        "register.rs",
        "fa-solid fa-hospital-user",
        "clase CSS de icono",
    ),
    (
        "dashboard.rs",
        "activos",
        "valor del filtro de estado que viaja a la API",
    ),
    ("dashboard.rs", "fa-solid fa-hospital", "clase CSS de icono"),
];

/// Quita comentarios de línea y de bloque, respetando literales de cadena: los
/// comentarios del código están en español y NO son texto de interfaz, así que
/// contarlos como hardcode daría una línea base inútil.
fn strip_comments(src: &str) -> String {
    let b = src.as_bytes();
    let mut out = String::with_capacity(src.len());
    let (mut i, mut in_str, mut in_char, mut in_line_comment, mut in_block) =
        (0, false, false, false, 0usize);
    while i < b.len() {
        let c = b[i];
        let next = b.get(i + 1).copied();
        if in_line_comment {
            if c == b'\n' {
                in_line_comment = false;
                out.push('\n');
            }
        } else if in_block > 0 {
            if c == b'*' && next == Some(b'/') {
                in_block -= 1;
                i += 1;
            } else if c == b'/' && next == Some(b'*') {
                in_block += 1;
                i += 1;
            } else if c == b'\n' {
                out.push('\n');
            }
        } else if in_str {
            out.push(c as char);
            if c == b'\\' {
                if let Some(n) = next {
                    out.push(n as char);
                }
                i += 1;
            } else if c == b'"' {
                in_str = false;
            }
        } else if c == b'"' {
            in_str = true;
            out.push('"');
        } else if c == b'\'' {
            in_char = !in_char;
            out.push('\'');
        } else if c == b'/' && next == Some(b'/') {
            in_line_comment = true;
            i += 1;
        } else if c == b'/' && next == Some(b'*') {
            in_block = 1;
            i += 1;
        } else {
            out.push(c as char);
        }
        i += 1;
    }
    out
}

/// Texto que se pinta en pantalla y está en español, con su línea 1-indexada.
///
/// Cubre las dos formas que usa Leptos: literal de cadena (`label="Cargando..."`)
/// y nodo de texto suelto (`>Cargando...<`), que NO es un literal y por eso se
/// le escapaba a cualquier escaneo de strings.
fn spanish_screen_text(
    src: &str,
    placeholders_conocidos: &BTreeSet<String>,
) -> Vec<(usize, String)> {
    let clean = strip_comments(src);
    let mut out = Vec::new();

    for (idx, line) in clean.lines().enumerate() {
        let lineno = idx + 1;
        let bytes = line.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'"' {
                let start = i + 1;
                let mut j = start;
                while j < bytes.len() && bytes[j] != b'"' {
                    j += if bytes[j] == b'\\' { 2 } else { 1 };
                }
                let lit = &line[start..j.min(line.len())];
                // No es texto de pantalla: el primer argumento de `tr()`
                // (`tr("dq-total", None)`) y los nombres de placeholder de los
                // `.ftl` (`args.insert("error", ...)`).
                let es_clave = line[..i].trim_end().ends_with("tr(")
                    || line[..i].trim_end().ends_with("trs(")
                    || placeholders_conocidos.contains(lit);
                let visible = !es_clave
                    && !lit.is_empty()
                    && !lit.contains('{')
                    && !lit.contains('=')
                    && lit.len() > 2
                    && looks_spanish_wide(lit);
                if visible {
                    out.push((lineno, lit.to_string()));
                }
                i = j + 1;
            } else {
                i += 1;
            }
        }

        // Nodos de texto: `>texto<`. Se descartan los `x > y && y < z` del propio
        // Rust filtrando los que llevan puntuación de código.
        let mut k = 0;
        while let Some(gt) = clean[k..].find('>') {
            let s = k + gt + 1;
            let Some(lt_rel) = clean[s..].find('<') else {
                break;
            };
            let e = s + lt_rel;
            k = e + 1;
            let txt = clean[s..e].trim();
            if txt.len() > 2
                && !txt.contains(['{', '}', ';', '=', '(', ')', '&', '|', ':', '"', '\\'])
                && !txt.contains("::")
                && looks_spanish_wide(txt)
            {
                out.push((lineno, txt.to_string()));
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn ftl_files_share_identical_key_set_and_order() {
    let b = bundles();
    let es: Vec<&str> = b["es"].iter().map(|(k, _)| k.as_str()).collect();
    assert!(!es.is_empty(), "i18n: es.ftl está vacío");

    for lang in LOCALES.iter().skip(1) {
        let other: Vec<&str> = b[*lang].iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(
            other.len(),
            es.len(),
            "i18n: {lang}.ftl tiene {} claves y es.ftl tiene {}",
            other.len(),
            es.len()
        );
        for (i, (a, o)) in es.iter().zip(other.iter()).enumerate() {
            assert_eq!(
                a, o,
                "i18n: clave #{i} fuera de orden o ausente: es={a} vs {lang}={o}"
            );
        }
    }

    let mut total = 0usize;
    for (lang, entries) in &b {
        let mut seen = HashSet::new();
        for (k, _) in entries {
            assert!(
                seen.insert(k.clone()),
                "i18n: clave duplicada '{k}' en {lang}.ftl"
            );
            assert!(
                !k.contains('.') && !k.starts_with("page-"),
                "i18n: '{k}' ({lang}.ftl) no sigue kebab-case sin puntos"
            );
        }
        total += entries.len();
    }
    assert!(
        total >= 400,
        "i18n: se esperaban >=400 claves (4 locales), hay {total}"
    );
}

#[test]
fn every_tr_key_exists_in_all_four_locales() {
    let b = bundles();
    let sets: BTreeMap<String, BTreeSet<String>> = b
        .iter()
        .map(|(lang, e)| (lang.clone(), e.iter().map(|(k, _)| k.clone()).collect()))
        .collect();

    let calls = scan_tr_calls();
    assert!(
        calls.len() >= 140,
        "i18n: solo se detectaron {} llamadas a tr(); el escaneo está roto",
        calls.len()
    );

    let mut missing = Vec::new();
    for call in &calls {
        for lang in LOCALES {
            if !sets[lang].contains(&call.key) {
                missing.push(format!(
                    "{}:{}: clave '{}' ausente en {lang}.ftl",
                    call.file, call.line, call.key
                ));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "i18n: claves de tr() inexistentes en algún locale:\n  {}",
        missing.join("\n  ")
    );
}

#[test]
fn placeholders_are_really_substituted() {
    // 1. El algoritmo de sustitución usa llaves simples y vacía los placeholders.
    let args: BTreeMap<String, String> = [("error".to_string(), "boom".to_string())]
        .into_iter()
        .collect();
    assert_eq!(substitute("Error: {error}", &args), "Error: boom");
    assert_eq!(
        substitute("No se pudieron cargar los tenants: {error}", &args),
        "No se pudieron cargar los tenants: boom"
    );
    // Regresión del bug: `{{error}}` (llaves dobles) nunca se sustituía.
    assert_ne!(substitute("Error: {error}", &args), "Error: {error}");

    // 2. Ningún `.ftl` puede usar sintaxis Fluent (`{{...}}`): aquí no se expande.
    let b = bundles();
    for (lang, entries) in &b {
        for (k, v) in entries {
            assert!(
                !v.contains("{{") && !v.contains("}}"),
                "i18n: {lang}.ftl clave '{k}' usa llaves dobles: {v}"
            );
        }
    }

    // 3. Toda clave con placeholders que se use en el código recibe `args`.
    let calls = scan_tr_calls();
    let with_args: HashMap<&str, bool> = calls.iter().fold(HashMap::new(), |mut acc, c| {
        let e = acc.entry(c.key.as_str()).or_insert(false);
        *e |= c.passes_args;
        acc
    });
    let mut sin_args = Vec::new();
    for (key, passes) in &with_args {
        if *passes {
            continue;
        }
        let mut missing = BTreeSet::new();
        for lang in LOCALES {
            if let Some((_, v)) = b[lang].iter().find(|(k, _)| k == key) {
                for ph in placeholders(v) {
                    missing.insert(ph);
                }
            }
        }
        if !missing.is_empty() {
            sin_args.push(format!(
                "'{key}' usa placeholders {missing:?} pero se llama con None"
            ));
        }
    }
    assert!(
        sin_args.is_empty(),
        "i18n: placeholders que nunca se sustituyen:\n  {}",
        sin_args.join("\n  ")
    );
}

#[test]
fn target_pages_have_no_residual_spanish_hardcodes() {
    let pages_dir = app_src_dir().join("pages");
    let mut offenders = Vec::new();
    let mut checked = 0usize;

    for page in TARGET_PAGES {
        let path = pages_dir.join(page);
        assert!(path.exists(), "i18n: no existe la página {path:?}");
        let content = fs::read_to_string(&path).expect("leer página");

        for (idx, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            // Nodo de texto de Leptos: la línea completa es un literal.
            let candidate =
                if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 3 {
                    Some(trimmed.trim_matches('"').to_string())
                } else if let Some(pos) = trimmed.rfind(">\"") {
                    // `</i>"Texto"` al final de la línea.
                    let tail = &trimmed[pos + 2..];
                    let tail = match tail.strip_suffix('"') {
                        Some(t) => t,
                        None => continue,
                    };
                    Some(tail.trim_end_matches('<').trim_matches('"').to_string())
                } else {
                    None
                };

            let Some(text) = candidate else { continue };
            if text.is_empty() || text.contains('{') || text.contains('=') {
                continue;
            }
            checked += 1;
            if !looks_spanish(&text) {
                continue;
            }
            let allowed = ALLOWED_RAW.iter().any(|(f, s)| *f == page && *s == text);
            if !allowed {
                offenders.push(format!("{page}:{}: {text:?}", idx + 1));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "i18n: hardcodeo en español residual en las páginas P0.3:\n  {}\n  \
         (si es intencionado, añádelo a ALLOWED_RAW con su justification)",
        offenders.join("\n  ")
    );
    assert!(
        checked > 0,
        "i18n: el detector de hardcodeo no encontró ningún literal (escaneo roto)"
    );
}

#[test]
fn spanish_hardcode_debt_does_not_grow() {
    let pages_dir = app_src_dir().join("pages");
    let mut files = Vec::new();
    rust_files(&pages_dir, &mut files);
    files.sort();

    // Nombres de placeholder de los `.ftl`: son claves de interpolación
    // (`args.insert("error", ...)`), no texto que se pinte.
    let mut phs: BTreeSet<String> = BTreeSet::new();
    for entries in bundles().values() {
        for (_, v) in entries {
            phs.extend(placeholders(v));
        }
    }

    let mut totales = 0usize;
    let mut crecio = Vec::new();
    let mut sin_linea_base = Vec::new();
    let mut detalle = Vec::new();

    for path in &files {
        let page = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let src = fs::read_to_string(path).expect("leer página");
        let permitidos: Vec<&str> = ALLOWED_RAW_BY_PAGE
            .iter()
            .filter(|(f, _, _)| *f == page)
            .map(|(_, s, _)| *s)
            .collect();

        let offenders: Vec<(usize, String)> = spanish_screen_text(&src, &phs)
            .into_iter()
            .filter(|(_, txt)| !permitidos.contains(&txt.as_str()))
            .collect();
        let n = offenders.len();
        totales += n;
        detalle.push(format!("  {page:<22} {n:>3}"));

        match I18N_DEBT_BASELINE.iter().find(|(f, _)| *f == page) {
            Some((_, base)) => {
                if n > *base {
                    crecio.push(format!(
                        "  {page}: {n} literales en español, la línea base es {base} \
                         (+{}). Sácalos con i18n::tr() o, si son intencionales, \
                         súbelos aquí con su razón.",
                        n - base
                    ));
                }
            }
            // Página nueva: entra limpia o no entra.
            None if n > 0 => sin_linea_base.push(format!(
                "  {page}: {n} literales en español y no está en I18N_DEBT_BASELINE"
            )),
            None => {}
        }
    }

    eprintln!(
        "i18n: deuda medida por página\n{}\n  TOTAL {totales}",
        detalle.join("\n")
    );
    assert!(
        crecio.is_empty(),
        "i18n: la deuda de hardcodes en español CRECIÓ:\n{}",
        crecio.join("\n")
    );
    assert!(
        sin_linea_base.is_empty(),
        "i18n: páginas nuevas con hardcodeo sin línea base:\n{}",
        sin_linea_base.join("\n")
    );
    assert!(
        files.len() >= 18,
        "i18n: el escaneo solo encontró {{files.len()}} páginas .rs en dmart-app/src/pages; detector roto"
    );
}
