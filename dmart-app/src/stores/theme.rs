use gloo_storage::{LocalStorage, Storage};
use leptos::prelude::*;

const THEME_KEY: &str = "dmart_theme";

#[derive(Clone, Debug, PartialEq)]
pub enum Theme {
    Dark,
    Light,
}

impl Theme {
    pub fn is_dark(&self) -> bool {
        matches!(self, Theme::Dark)
    }

    pub fn as_str(&self) -> &str {
        match self {
            Theme::Dark => "dark",
            Theme::Light => "light",
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        match LocalStorage::get::<String>(THEME_KEY) {
            Ok(stored) if stored == "light" => Theme::Light,
            _ => Theme::Dark,
        }
    }
}

impl From<String> for Theme {
    fn from(s: String) -> Self {
        if s == "light" {
            Theme::Light
        } else {
            Theme::Dark
        }
    }
}

pub fn create_theme_store() -> (RwSignal<Theme>, impl Fn(Theme) -> ()) {
    let initial = Theme::default();
    let signal = RwSignal::new(initial.clone());

    apply_theme(&initial);

    let signal_clone = signal.clone();
    let setter = move |theme: Theme| {
        apply_theme(&theme);
        let _ = LocalStorage::set(THEME_KEY, theme.as_str());
        signal_clone.set(theme);
    };

    (signal, setter)
}

fn apply_theme(theme: &Theme) {
    if let Some(window) = web_sys::window() {
        if let Some(doc) = window.document() {
            if let Some(html) = doc.document_element() {
                let class_list = html.class_list();
                if theme.is_dark() {
                    let _ = class_list.add_1("dark");
                    let _ = class_list.remove_1("light");
                } else {
                    let _ = class_list.remove_1("dark");
                    let _ = class_list.add_1("light");
                }
            }
        }
    }
}
