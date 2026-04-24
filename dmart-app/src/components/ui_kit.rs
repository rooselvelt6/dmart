use leptos::prelude::*;

#[component]
pub fn Spinner() -> impl IntoView {
    view! {
        <div style="display:inline-block; width:24px; height:24px; border:2px solid #2A3547; border-top-color:#3B82F6; border-radius:50%; animation:spin 0.8s linear infinite;"></div>
    }
}

#[component]
pub fn Badge(label: &'static str) -> impl IntoView {
    view! {
        <span style="display:inline-block; padding:4px 10px; background:rgba(59,130,246,0.15); color:#3B82F6; border-radius:12px; font-size:12px; font-weight:600;">
            {label}
        </span>
    }
}

#[component]
pub fn Card(title: &'static str, content: impl IntoView) -> impl IntoView {
    view! {
        <div class="glass-card" style="padding:20px;">
            <h3 style="font-size:14px; font-weight:700; color:var(--uci-text); margin:0 0 12px;">{title}</h3>
            {content}
        </div>
    }
}
