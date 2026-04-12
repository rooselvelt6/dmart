use dmart_shared::models::SeverityLevel;
use leptos::prelude::*;

#[component]
pub fn SeverityBadge(level: SeverityLevel) -> impl IntoView {
    let label = level.label();
    let class = level.color_class();

    view! {
        <span class=format!("px-3 py-1 rounded-full text-xs font-bold uppercase tracking-wider {}", class)>
            {label}
        </span>
    }
}
