// Theme module for dMart UCI
// Supports Light/Dark/System themes with CSS variables

use gloo_storage::{LocalStorage, Storage};
use leptos::prelude::*;
use std::sync::{Mutex, OnceLock};

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

/// Track last applied theme to prevent redundant DOM mutations (untracked)
static LAST_APPLIED_THEME: OnceLock<Mutex<Option<Theme>>> = OnceLock::new();

/// Aplica la clase `dark`/`light` en `<html>`: el mecanismo real que
/// `input.css` (Tailwind) usa para cambiar todas las variables `--uci-*`.
fn apply_dom_theme(t: &Theme) {
    let last_lock = LAST_APPLIED_THEME.get_or_init(|| Mutex::new(None));

    // Skip if theme hasn't changed (untracked - no reactive subscription)
    if last_lock.lock().ok().and_then(|v| *v) == Some(*t) {
        return;
    }

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
        // Track applied theme
        if let Ok(mut guard) = last_lock.lock() {
            *guard = Some(*t);
        }
    }
}

/// Hook to manage theme state with persistence and DOM updates
pub fn use_theme() -> (ReadSignal<Theme>, WriteSignal<Theme>) {
    let stored = LocalStorage::get(THEME_KEY)
        .ok()
        .flatten()
        .unwrap_or_default();
    let (theme, set_theme) = signal(stored);

    // Apply theme to document on changes
    Effect::new(move |_| {
        let t = theme.get();
        apply_dom_theme(&t);
        LocalStorage::set(THEME_KEY, t).ok();
    });

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

/// Global header visible on ALL pages (including login)
/// Contains: logo, theme selector, language selector
#[component]
pub fn GlobalHeader() -> impl IntoView {
    view! {
        <header class="global-header" style="
            position: fixed; top: 0; left: 0; right: 0; z-index: 1000;
            display: flex; align-items: center; justify-content: space-between;
            padding: 8px 16px; gap: 16px;
            background: var(--bg-card); border-bottom: 1px solid var(--border-primary);
            backdrop-filter: blur(8px);
            box-shadow: 0 1px 4px rgba(0,0,0,0.08);
        ">
            <div style="display: flex; align-items: center; gap: 12px;">
                <a href="/" style="display: flex; align-items: center; gap: 8px; text-decoration: none; color: inherit;">
                    <div style="
                        width: 36px; height: 36px; border-radius: 10px;
                        background: linear-gradient(135deg, #0EA5E9 0%, #2563EB 50%, #6366F1 100%);
                        display: flex; align-items: center; justify-content: center;
                        box-shadow: 0 2px 8px rgba(14, 165, 233, 0.35);
                    ">
                        <i class="fa-solid fa-heart-pulse" style="font-size: 18px; color: white;"></i>
                    </div>
                    <span style="font-weight: 700; font-size: 16px; color: var(--text-primary); letter-spacing: -0.3px;">
                        <span style="color: #0EA5E9;">UCI</span> <span style="font-weight: 800;">DMART</span>
                    </span>
                </a>
            </div>

            <div style="display: flex; align-items: center; gap: 12px;">
                <crate::theme::ThemeSelector />
                <crate::theme::LangSelector />
            </div>
        </header>
    }
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
