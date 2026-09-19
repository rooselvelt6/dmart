use dmart_app::{app::App, i18n, theme, shortcuts};
use leptos::prelude::*;

fn main() {
    console_error_panic_hook::set_once();

    // MUST be the absolute first call: configures the global any_spawner
    // executor before anything else can trigger a spawn_local, otherwise
    // any_spawner panics with "no global 'spawn_local' function configured".
    let _ = any_spawner::Executor::init_wasm_bindgen();

    // Initialize i18n, theme, and shortcuts
    i18n::init_i18n();
    theme::init_theme();
    shortcuts::init_shortcuts();
    
    // Disable Service Worker registration for development
    #[cfg(debug_assertions)]
    {
        use wasm_bindgen::prelude::*;
        #[wasm_bindgen]
        extern "C" {
            #[wasm_bindgen(js_namespace = navigator, js_name = serviceWorker)]
            static SERVICE_WORKER: web_sys::ServiceWorkerContainer;
        }
        // Don't register service worker in development
    }
    
    mount_to_body(App);
}