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

/// Detecta la preferencia del SO cuando el tema es `System`.
fn system_is_dark() -> bool {
    if let Some(window) = web_sys::window()
        && let Ok(Some(media)) = window.match_media("(prefers-color-scheme: dark)")
        && media.matches()
    {
        return true;
    }
    false
}

/// Aplica la clase `dark`/`light` en `<html>`: el mecanismo real que
/// `input.css` (Tailwind) usa para cambiar todas las variables `--uci-*`.
fn apply_dom_theme(t: &Theme) {
    if let Some(window) = web_sys::window()
        && let Some(doc) = window.document()
        && let Some(html) = doc.document_element()
    {
        let is_dark = match t {
            Theme::Light => false,
            Theme::Dark => true,
            Theme::System => system_is_dark(),
        };
        let class_list = html.class_list();
        if is_dark {
            let _ = class_list.add_1("dark");
            let _ = class_list.remove_1("light");
        } else {
            let _ = class_list.remove_1("dark");
            let _ = class_list.add_1("light");
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
        apply_dom_theme(&t);
        LocalStorage::set(THEME_KEY, t).ok();
    });
    
    // Apply inmediato (el Effect corre tras la primera renderización).
    apply_dom_theme(&stored);
    
    (theme, set_theme)
}

/// Theme selector: botón cíclico con icono que al hacer clic cambia
/// Sistema -> Claro -> Oscuro (contoja el `<html>` con la clase dark/light).
#[component]
pub fn ThemeSelector() -> impl IntoView {
    let (theme, set_theme) = use_theme();
    let is_dark = move || match theme.get() {
        Theme::Dark => true,
        Theme::Light => false,
        Theme::System => system_is_dark(),
    };

    view! {
        <button
            class="theme-switch"
            on:click=move |_| {
                let next = match theme.get() {
                    Theme::System => Theme::Light,
                    Theme::Light => Theme::Dark,
                    Theme::Dark => Theme::System,
                };
                set_theme.set(next);
            }
            aria-label=crate::i18n::tr("aria-theme", None)
            title=move || crate::i18n::tr("settings-theme", None)
        >
            <span class="theme-switch-icon">
                {move || if is_dark() {
                    view! { <i class="fa-solid fa-moon"></i> }
                } else {
                    view! { <i class="fa-solid fa-sun"></i> }
                }}
            </span>
            <span class="theme-switch-label">
                {move || match theme.get() {
                    Theme::System => crate::i18n::tr("settings-theme-system", None),
                    Theme::Light => crate::i18n::tr("settings-theme-light", None),
                    Theme::Dark => crate::i18n::tr("settings-theme-dark", None),
                }}
            </span>
        </button>
    }
}

/// Initialize theme on app startup (call from App component)
pub fn init_theme() {
    let _ = use_theme(); // Initializes the effect
}

/// Selector de idioma: flags + nombre de cada locale soportado.
#[component]
pub fn LangSelector() -> impl IntoView {
    const LANGS: &[(&str, &str, &str)] = &[
        ("es", "🇪🇸", "Español"),
        ("en", "🇺🇸", "English"),
        ("pt", "🇧🇷", "Português"),
        ("fr", "🇫🇷", "Français"),
    ];
    let current = crate::i18n::use_lang();

    view! {
        <div class="lang-switch" role="group" aria-label=crate::i18n::tr("aria-language", None)>
            {LANGS.iter().map(move |(code, flag, name)| {
                let code = *code;
                let flag = *flag;
                let name = *name;
                view! {
                    <button
                        class=move || {
                            let mut cls = "lang-switch-btn".to_string();
                            if current.get() == code {
                                cls.push_str(" active");
                            }
                            cls
                        }
                        on:click=move |_| {
                            if current.get() != code {
                                crate::i18n::set_lang(code);
                            }
                        }
                        title=name
                        aria-label=name
                    >
                        <span class="lang-flag" aria-hidden="true">{flag}</span>
                        <span class="lang-name">{name}</span>
                    </button>
                }
            }).collect_view()}
        </div>
    }
}