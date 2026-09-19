// i18n module for dMart UCI
// Simple key-value translation with embedded .ftl files (simple key=value format)

use std::sync::OnceLock;
use std::collections::HashMap;

/// Supported languages in order of preference
const LOCALES: &[&str] = &["es", "en", "pt", "fr"];

/// Bundle for each locale - simple HashMap of key -> translation
static BUNDLES: OnceLock<Vec<HashMap<String, String>>> = OnceLock::new();

/// Initialize i18n bundles from embedded .ftl files (simple key=value format)
pub fn init_i18n() {
    let bundles: Vec<HashMap<String, String>> = LOCALES.iter().map(|lang| {
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
                map.insert(key.trim().to_string(), value.trim().to_string());
            }
        }
        map
    }).collect();
    
    BUNDLES.set(bundles).ok();
}

/// Get current language from localStorage or browser
pub fn get_current_lang() -> String {
    use gloo_storage::{LocalStorage, Storage};
    LocalStorage::get("dmart_lang").unwrap_or_else(|_| {
        web_sys::window()
            .and_then(|w| w.navigator().language())
            .unwrap_or_else(|| "es".into())
            .split('-')
            .next()
            .unwrap_or("es")
            .to_string()
    })
}

/// Set language preference in localStorage
pub fn set_lang(lang: &str) {
    use gloo_storage::{LocalStorage, Storage};
    LocalStorage::set("dmart_lang", lang).ok();
}

/// Translate a key with optional arguments
/// Returns the translated string, or the key itself if not found
pub fn tr(key: &str, args: Option<&std::collections::HashMap<String, String>>) -> String {
    if BUNDLES.get().is_none() {
        init_i18n();
    }
    
    let lang = get_current_lang();
    let lang_idx = LOCALES.iter().position(|&l| l == lang).unwrap_or(0);
    
    let bundles = BUNDLES.get().expect("i18n not initialized");
    
    for bundle in &bundles[lang_idx..] {
        if let Some(value) = bundle.get(key) {
            let mut result = value.clone();
            if let Some(args) = args {
                for (k, v) in args {
                    result = result.replace(&format!("{{{{{}}}}}", k), v);
                }
            }
            return result;
        }
    }
    
    // Fallback to first bundle (usually Spanish)
    if let Some(bundle) = bundles.first() {
        if let Some(value) = bundle.get(key) {
            let mut result = value.clone();
            if let Some(args) = args {
                for (k, v) in args {
                    result = result.replace(&format!("{{{{{}}}}}", k), v);
                }
            }
            return result;
        }
    }
    
    // Ultimate fallback: return the key itself
    key.to_string()
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