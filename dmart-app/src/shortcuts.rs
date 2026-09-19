// Keyboard shortcuts module for dMart UCI
// Power-user shortcuts for clinical workflows

use leptos::prelude::*;
use web_sys::{KeyboardEvent, window, HtmlElement};
use wasm_bindgen::prelude::*;
use wasm_bindgen::closure::Closure;
use js_sys;

/// Shortcut definitions
pub struct Shortcut {
    pub key: &'static str,
    pub alt: bool,
    pub ctrl: bool,
    pub shift: bool,
    pub description: &'static str,
    pub action: fn(),
}

/// Global shortcut registry
static SHORTCUTS: once_cell::sync::Lazy<Vec<Shortcut>> = once_cell::sync::Lazy::new(|| {
    vec![
        Shortcut {
            key: "n",
            alt: true,
            ctrl: false,
            shift: false,
            description: "Nuevo paciente",
            action: || navigate("/patients/new"),
        },
        Shortcut {
            key: "b",
            alt: true,
            ctrl: false,
            shift: false,
            description: "Buscar paciente",
            action: || {
                let window = window().unwrap();
                let document = window.document().unwrap();
                if let Some(input) = document.get_element_by_id("search-input") {
                    let _ = input.dyn_into::<HtmlElement>().unwrap().focus();
                }
            },
        },
        Shortcut {
            key: "e",
            alt: true,
            ctrl: false,
            shift: false,
            description: "Exportar PDF",
            action: || export_current_pdf(),
        },
        Shortcut {
            key: "m",
            alt: true,
            ctrl: false,
            shift: false,
            description: "Nueva medición",
            action: || open_measurement_modal(),
        },
        Shortcut {
            key: "Escape",
            alt: false,
            ctrl: false,
            shift: false,
            description: "Cerrar modal / Limpiar búsqueda",
            action: || close_modals(),
        },
        Shortcut {
            key: "?",
            alt: false,
            ctrl: false,
            shift: true,
            description: "Mostrar ayuda de atajos",
            action: || show_shortcuts_help(),
        },
    ]
});

/// Initialize global keyboard listener (call once in App)
pub fn init_shortcuts() {
    let handler = move |ev: KeyboardEvent| {
        let alt = ev.alt_key();
        let ctrl = ev.ctrl_key();
        let shift = ev.shift_key();
        let key = ev.key();
        
        // Don't trigger shortcuts when typing in inputs
        if let Some(target) = ev.target() {
            if let Ok(element) = target.dyn_into::<web_sys::HtmlElement>() {
                let tag = element.tag_name().to_lowercase();
                let is_input = matches!(tag.as_str(), "input" | "textarea" | "select");
                let is_contenteditable = element.is_content_editable();
                if is_input || is_contenteditable {
                    return;
                }
            }
        }
        
        for shortcut in SHORTCUTS.iter() {
            if shortcut.key == key
                && shortcut.alt == alt
                && shortcut.ctrl == ctrl
                && shortcut.shift == shift
            {
                ev.prevent_default();
                (shortcut.action)();
                break;
            }
        }
    };
    
    // Convert the closure to a JavaScript function using wasm_bindgen
    let closure = Closure::wrap(Box::new(handler) as Box<dyn FnMut(KeyboardEvent)>);
    
    let window = window().unwrap();
    let _ = window.add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref());
    
    // Note: In Leptos, we'd typically use on_cleanup to remove the listener
    // but for global shortcuts we want them for the whole session
    // The closure will be kept alive by leaking it (intentional for app lifetime)
    closure.forget();
}

/// Navigate to a route (uses leptos_router)
fn navigate(path: &str) {
    use leptos_router::hooks::use_navigate;
    let navigate = use_navigate();
    navigate(path, leptos_router::NavigateOptions::default());
}

/// Focus search input
fn focus_search() {
    let window = window().unwrap();
    let document = window.document().unwrap();
    if let Some(input) = document.get_element_by_id("search-input") {
        let _ = input.dyn_into::<HtmlElement>().unwrap().focus();
    }
}

/// Export current patient PDF (if on patient detail page)
fn export_current_pdf() {
    // Get current patient ID from URL
    let location = window().unwrap().location();
    let pathname = location.pathname().unwrap_or_default();
    if let Some(patient_id) = pathname.strip_prefix("/patients/") {
        if let Some(id) = patient_id.split('/').next() {
            let url = format!("/api/patients/{}/export/pdf", id);
            let window = window().unwrap();
            let _ = window.open_with_url_and_target(&url, "_blank");
        }
    }
}

/// Open measurement modal for current patient
fn open_measurement_modal() {
    let location = window().unwrap().location();
    let pathname = location.pathname().unwrap_or_default();
    if let Some(patient_id) = pathname.strip_prefix("/patients/") {
        if let Some(id) = patient_id.split('/').next() {
            let url = format!("/patients/{}/measure?escala=apache", id);
            let window = window().unwrap();
            let _ = window.location().set_href(&url);
        }
    }
}

/// Close all open modals/dropdowns
fn close_modals() {
    // Dispatch escape key event to close any open modals
    let win = window().unwrap();
    let document = win.document().unwrap();
    
    // Create a simple KeyboardEvent for Escape key
    if let Ok(event) = web_sys::KeyboardEvent::new("keydown") {
        // Set the key property via reflection
        js_sys::Reflect::set(&event, &"key".into(), &"Escape".into()).ok();
        let _ = document.dispatch_event(&event);
    }
    
    // Also blur any focused element
    let win2 = window().unwrap();
    let document2 = win2.document().unwrap();
    if let Some(active) = document2.active_element() {
        let _ = active.dyn_into::<HtmlElement>().unwrap().blur();
    }
}

/// Show shortcuts help modal
fn show_shortcuts_help() {
    let shortcuts = SHORTCUTS.iter().map(|s| {
        let mut keys = Vec::new();
        if s.alt { keys.push("Alt"); }
        if s.ctrl { keys.push("Ctrl"); }
        if s.shift { keys.push("Shift"); }
        keys.push(s.key);
        (keys.join(" + "), s.description)
    }).collect::<Vec<_>>();
    
    // Create and show help modal
    let modal_html = format!(
        r#"<div class="shortcuts-modal" role="dialog" aria-labelledby="shortcuts-title">
            <div class="shortcuts-modal-content">
                <header><h2 id="shortcuts-title">Atajos de teclado</h2><button class="modal-close" onclick="this.closest('.shortcuts-modal').remove()">&times;</button></header>
                <table class="shortcuts-table">
                    <thead><tr><th>Atajo</th><th>Acción</th></tr></thead>
                    <tbody>
                        {}
                    </tbody>
                </table>
            </div>
        </div>"#,
        shortcuts.iter().map(|(k, d)| format!("<tr><kbd>{}</kbd><td>{}</td></tr>", k, d)).collect::<Vec<_>>().join("")
    );
    
let window = window().unwrap();
    let document = window.document().unwrap();
    let body = document.body().unwrap();
    let div = document.create_element("div").unwrap();
    div.set_inner_html(&modal_html);
    div.set_class_name("shortcuts-modal-overlay");
    let _ = body.append_child(&div);
    
    // Close on overlay click
    let overlay = div.clone();
    let close_handler = move |_: web_sys::Event| {
        let _ = overlay.remove();
    };
    let closure = Closure::wrap(Box::new(close_handler) as Box<dyn FnMut(web_sys::Event)>);
    div.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref()).ok();
    closure.forget();
}

/// Get list of shortcuts for help display
pub fn get_shortcuts_list() -> Vec<(&'static str, &'static str)> {
    (*SHORTCUTS).iter().map(|s| {
        let mut keys = Vec::new();
        if s.alt { keys.push("Alt"); }
        if s.ctrl { keys.push("Ctrl"); }
        if s.shift { keys.push("Shift"); }
        keys.push(s.key);
        let combined: &'static str = Box::leak(keys.join(" + ").into_boxed_str());
        (combined, s.description)
    }).collect()
}

/// Hook to use shortcuts in components (for local shortcuts)
pub fn use_shortcuts(shortcuts: Vec<(&'static str, bool, bool, bool, fn())>) {
    let handler = move |ev: KeyboardEvent| {
        let alt = ev.alt_key();
        let ctrl = ev.ctrl_key();
        let shift = ev.shift_key();
        let key = ev.key();
        
        for (sk, sa, sc, ssh, action) in &shortcuts {
            if sk == &key && *sa == alt && *sc == ctrl && *ssh == shift {
                ev.prevent_default();
                action();
                break;
            }
        }
    };
    
    // Convert closure to JavaScript function
    let closure = Closure::wrap(Box::new(handler) as Box<dyn FnMut(KeyboardEvent)>);
    
    let window = window().unwrap();
    let _ = window.add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref());
    
    // Keep closure alive for the component lifetime
    // Note: In a real app, you'd want to properly clean up on component unmount
    // but Closure doesn't implement Clone, so we leak it intentionally
    std::mem::forget(closure);
}