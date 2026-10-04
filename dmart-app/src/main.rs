use dmart_app::{app, app::App, i18n, stores};
use leptos::prelude::*;

fn main() {
    console_error_panic_hook::set_once();

    // MUST be the absolute first call: configures the global any_spawner
    // executor before anything else can trigger a spawn_local, otherwise
    // any_spawner panics with "no global 'spawn_local' function configured".
    let _ = any_spawner::Executor::init_wasm_bindgen();

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

    // Las señales globales se crean AQUÍ, fuera de todo owner reactivo: si
    // nacen dentro de un componente, Leptos las destruye al desmontarlo y el
    // router se queda muerto en la navegación siguiente. Ver
    // `i18n::init_lang_signal`.
    i18n::init_lang_signal();
    app::init_pending_path_signal();
    stores::init_toasts_signal();

    mount_to_body(App);
}
