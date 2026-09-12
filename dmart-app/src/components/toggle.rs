use leptos::prelude::*;

#[component]
pub fn Toggle<F>(value: Signal<bool>, on_change: F) -> impl IntoView
where
    F: Fn(bool) + 'static,
{
    view! {
        <button
            type="button"
            role="switch"
            aria-checked=move || value.get()
            class=move || format!("toggle {}", if value.get() { "on" } else { "" })
            on:click=move |_| on_change(!value.get())
        ></button>
    }
}
