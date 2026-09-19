// Theme module for dMart UCI
// Supports Light/Dark/System themes with CSS variables

use leptos::prelude::*;
use gloo_storage::{LocalStorage, Storage};

const THEME_KEY: &str = "dmart_theme";

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Theme {
    System = 0,
    Light = 1,
    Dark = 2,
}

impl Default for Theme {
    fn default() -> Self {
        Theme::System
    }
}

impl Theme {
    pub fn label(&self) -> &'static str {
        match self {
            Theme::System => "Sistema",
            Theme::Light => "Claro",
            Theme::Dark => "Oscuro",
        }
    }
    
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Theme::Light,
            2 => Theme::Dark,
            _ => Theme::System,
        }
    }
    
    pub fn as_str(&self) -> &'static str {
        match self {
            Theme::Light => "light",
            Theme::Dark => "dark",
            Theme::System => "system",
        }
    }
}

/// Hook to manage theme state with persistence and DOM updates
pub fn use_theme() -> (ReadSignal<Theme>, WriteSignal<Theme>) {
    let stored = LocalStorage::get(THEME_KEY).ok().flatten().unwrap_or_default();
    let (theme, set_theme) = signal(stored);
    
    // Apply theme to document on changes
    Effect::new(move |_| {
        let t = theme.get();
        let doc = web_sys::window().unwrap().document().unwrap();
        let html = doc.document_element().unwrap();
        
        match t {
            Theme::Light => {
                html.set_attribute("data-theme", "light").ok();
                html.remove_attribute("data-theme-system").ok();
            }
            Theme::Dark => {
                html.set_attribute("data-theme", "dark").ok();
                html.remove_attribute("data-theme-system").ok();
            }
            Theme::System => {
                html.remove_attribute("data-theme").ok();
                html.set_attribute("data-theme-system", "").ok();
            }
        }
        LocalStorage::set(THEME_KEY, theme.get()).ok();
    });
    
    (theme, set_theme)
}

/// Theme selector component for header/settings
#[component]
pub fn ThemeSelector() -> impl IntoView {
    let (theme, set_theme) = use_theme();
    
    view! {
        <select
            class="theme-select"
            prop:value=move || theme.get() as u8
            on:change=move |ev| {
                let v = event_target_value(&ev).parse::<u8>().unwrap_or(0);
                set_theme.set(Theme::from_u8(v));
            }
            aria-label="Seleccionar tema"
            title="Tema de la interfaz"
        >
            <option value=0>{crate::i18n::tr("settings-theme-system", None)}</option>
            <option value=1>{crate::i18n::tr("settings-theme-light", None)}</option>
            <option value=2>{crate::i18n::tr("settings-theme-dark", None)}</option>
        </select>
    }
}

/// Initialize theme on app startup (call from App component)
pub fn init_theme() {
    let _ = use_theme(); // Initializes the effect
}