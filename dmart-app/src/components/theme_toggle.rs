use crate::stores::Theme;
use leptos::prelude::*;

#[component]
pub fn ThemeToggle() -> impl IntoView {
    let (theme, set_theme) = crate::stores::create_theme_store();
    let is_dark = Signal::derive(move || theme.get().is_dark());

    view! {
        <button
            on:click=move |_| {
                let current = theme.get();
                let new_theme = if current.is_dark() { Theme::Light } else { Theme::Dark };
                set_theme(new_theme);
            }
            class="flex items-center gap-2 px-3 py-2 rounded-lg bg-uci-surface hover:bg-uci-card transition-all cursor-pointer"
        >
            <span class="text-lg">
                {move || if is_dark.get() { "🌙" } else { "☀️" }}
            </span>
            <span class="text-xs text-uci-muted">
                {move || if is_dark.get() { "Oscuro" } else { "Claro" }}
            </span>
        </button>
    }
}
