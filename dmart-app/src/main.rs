use dmart_app::{app::App, i18n, theme, shortcuts};
use leptos::prelude::*;

fn main() {
    console_error_panic_hook::set_once();
    
    // Initialize i18n, theme, and shortcuts
    i18n::init_i18n();
    theme::init_theme();
    shortcuts::init_shortcuts();
    
    mount_to_body(App);
}