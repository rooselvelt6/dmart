// i18n module for dMart UCI
// Simple key-value translation with embedded .ftl files (simple key=value format)

use leptos::prelude::*;
use std::collections::HashMap;
use std::sync::OnceLock;

/// Supported languages in order of preference
const LOCALES: &[&str] = &["es", "en", "pt", "fr"];

/// Bundle for each locale - simple HashMap of key -> translation
static BUNDLES: OnceLock<Vec<HashMap<String, String>>> = OnceLock::new();

/// Idioma actual, reactivo: cambiar idioma re-renderiza las vistas sin recarga.
static CURRENT_LANG: OnceLock<RwSignal<String>> = OnceLock::new();

/// Devuelve el signal global del idioma (reactivo).
pub fn lang_signal() -> RwSignal<String> {
    *CURRENT_LANG.get_or_init(|| {
        let initial = detect_lang();
        RwSignal::new(initial)
    })
}

/// Detecta el idioma desde localStorage o el navegador (syna).
fn detect_lang() -> String {
    use gloo_storage::{LocalStorage, Storage};
    match LocalStorage::get::<String>("dmart_lang") {
        Ok(stored) if LOCALES.contains(&stored.as_str()) => stored,
        _ => web_sys::window()
            .and_then(|w| w.navigator().language())
            .unwrap_or_else(|| "es".into())
            .split('-')
            .next()
            .unwrap_or("es")
            .to_string(),
    }
}

/// Initialize i18n bundles from embedded .ftl files (simple key=value format)
pub fn init_i18n() {
    let bundles: Vec<HashMap<String, String>> = LOCALES
        .iter()
        .map(|lang| {
            let ftl_content = match *lang {
                "es" => include_str!("../locales/es.ftl"),
                "en" => include_str!("../locales/en.ftl"),
                "pt" => include_str!("../locales/pt.ftl"),
                "fr" => include_str!("../locales/fr.ftl"),
                _ => include_str!("../locales/en.ftl"),
            };

            // Simple key=value parsing (one per line, ignoring comments and empty lines)
            let mut map = HashMap::new();
            for line in ftl_content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some((key, value)) = line.split_once('=') {
                    let value = value.trim();
                    // Las .ftl usan comillas alrededor del valor: `key = "texto"`.
                    let value =
                        if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
                            &value[1..value.len() - 1]
                        } else {
                            value
                        };
                    map.insert(key.trim().to_string(), value.to_string());
                }
            }
            map
        })
        .collect();

    BUNDLES.set(bundles).ok();
}

/// Get current language from localStorage or browser
pub fn get_current_lang() -> String {
    lang_signal().get_untracked()
}

/// Igual que `get_current_lang()`, pero suscribe al cambio de idioma (reactivo):
/// cualquier `move || tr(...)` re-renderiza al cambiar idioma.
fn lang_tracked() -> String {
    lang_signal().get_untracked()
}

/// Set language preference in localStorage (reactivo: actualiza la UI al instante)
pub fn set_lang(lang: &str) {
    use gloo_storage::{LocalStorage, Storage};
    if !LOCALES.contains(&lang) {
        return;
    }
    LocalStorage::set("dmart_lang", lang).ok();
    lang_signal().set(lang.to_string());

    // Update html lang attribute for SEO/accessibility
    if let Some(window) = web_sys::window() {
        if let Some(document) = window.document() {
            if let Some(html) = document.document_element() {
                html.set_attribute("lang", lang).ok();
            }
        }
    }
}

/// Sustituye los placeholders de un valor `.ftl` por los argumentos dados.
///
/// Los `.ftl` de dMart NO son Fluent: usan llaves **simples** (`{error}`), no
/// dobles (`{{error}}`). El `format!` de abajo debe producir exactamente
/// `{nombre}`; con llaves dobles la sustitución nunca ocurre y el placeholder
/// se queda literal en pantalla.
fn substitute(value: &str, args: &std::collections::HashMap<String, String>) -> String {
    let mut result = value.to_string();
    for (k, v) in args {
        result = result.replace(&format!("{{{k}}}"), v);
    }
    result
}

/// Resuelve `value` contra `args`, sin placeholders pendientes.
fn resolve(value: &String, args: Option<&std::collections::HashMap<String, String>>) -> String {
    match args {
        Some(a) => substitute(value, a),
        None => value.clone(),
    }
}

/// Translate a key with optional arguments
/// Returns the translated string, or the key itself if not found
pub fn tr(key: &str, args: Option<&std::collections::HashMap<String, String>>) -> String {
    if BUNDLES.get().is_none() {
        init_i18n();
    }

    let lang = lang_tracked();
    let lang_idx = LOCALES.iter().position(|&l| l == lang).unwrap_or(0);

    let bundles = BUNDLES.get().expect("i18n not initialized");

    for bundle in &bundles[lang_idx..] {
        if let Some(value) = bundle.get(key) {
            return resolve(value, args);
        }
    }

    // Fallback to first bundle (usually Spanish)
    if let Some(bundle) = bundles.first() {
        if let Some(value) = bundle.get(key) {
            return resolve(value, args);
        }
    }

    // Ultimate fallback: return the key itself
    key.to_string()
}

/// Hook reactivo: devuelve el idioma actual como Signal (re-renderiza al cambiar)
pub fn use_lang() -> ReadSignal<String> {
    lang_signal().read_only()
}

/// Helper to create args HashMap from key-value pairs
pub fn args_from_pairs(pairs: &[(&str, &str)]) -> std::collections::HashMap<String, String> {
    let mut args = std::collections::HashMap::new();
    for (k, v) in pairs {
        args.insert(k.to_string(), v.to_string());
    }
    args
}

/// Helper for single argument
pub fn arg(key: &str, value: &str) -> std::collections::HashMap<String, String> {
    let mut args = std::collections::HashMap::new();
    args.insert(key.to_string(), value.to_string());
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitute_uses_single_brace_placeholders() {
        let mut args = HashMap::new();
        args.insert("error".to_string(), "boom".to_string());
        args.insert("count".to_string(), "7".to_string());
        assert_eq!(substitute("Error: {error}", &args), "Error: boom");
        assert_eq!(substitute("{count} registros", &args), "7 registros");
        assert_eq!(
            substitute("{count} de {count} planes ({error})", &args),
            "7 de 7 planes (boom)"
        );
        assert_eq!(substitute("sin placeholders", &args), "sin placeholders");
    }

    #[test]
    fn substitute_keeps_unknown_placeholders_untouched() {
        let args = HashMap::new();
        assert_eq!(substitute("Error: {error}", &args), "Error: {error}");
    }

    #[test]
    fn resolve_without_args_is_a_clone() {
        let v = "Error: {error}".to_string();
        assert_eq!(resolve(&v, None), "Error: {error}");
    }
}
