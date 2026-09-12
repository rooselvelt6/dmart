use leptos::prelude::*;

#[component]
pub fn Spinner() -> impl IntoView {
    view! {
        <div style="display:inline-block; width:24px; height:24px; border:2px solid #2A3547; border-top-color:#3B82F6; border-radius:50%; animation:spin 0.8s linear infinite;"></div>
    }
}

/// Estado de carga reutilizable (accesible).
#[component]
pub fn LoadingState(label: &'static str) -> impl IntoView {
    view! {
        <div
            class="flex items-center justify-center gap-3 p-10"
            style="color:var(--uci-muted);"
            role="status"
            aria-live="polite"
            aria-label=move || label
        >
            <Spinner />
            <span style="font-size:14px;">{label}</span>
        </div>
    }
}

/// Estado de error reutilizable con botón de reintento.
#[component]
pub fn ErrorState(message: String, on_retry: Option<Callback<()>>) -> impl IntoView {
    view! {
        <div
            class="flex flex-col items-center justify-center gap-3 p-10 text-center rounded-lg"
            style="background:var(--uci-surface); color:var(--uci-text);"
            role="alert"
        >
            <div style="font-size:28px;" aria-hidden="true">"⚠"</div>
            <p style="font-size:14px; margin:0; max-width:420px; color:var(--uci-muted);">{move || message.clone()}</p>
            {move || on_retry.map(|cb| {
                view! {
                    <button class="btn-primary" on:click=move |_| cb.run(())>"Reintentar"</button>
                }
            })}
        </div>
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
