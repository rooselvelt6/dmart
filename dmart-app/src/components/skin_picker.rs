use dmart_shared::models::ColorPiel;
use leptos::prelude::*;

#[component]
pub fn SkinPicker<F>(value: Signal<ColorPiel>, on_change: F) -> impl IntoView
where
    F: Fn(ColorPiel) + Send + Sync + 'static,
{
    let options = vec![
        ColorPiel::Tipo1,
        ColorPiel::Tipo2,
        ColorPiel::Tipo3,
        ColorPiel::Tipo4,
        ColorPiel::Tipo5,
        ColorPiel::Tipo6,
    ];

    let on_change = StoredValue::new(on_change);

    view! {
        <div class="flex gap-3 mt-2">
            {options.into_iter().map(|opt| {
                let opt_cloned = opt.clone();
                let opt_for_selected = opt_cloned.clone();
                let is_selected = move || value.get() == opt_for_selected;
                let bg_color = opt.hex_color();
                let label = opt.label();

                view! {
                    <div
                        class=move || format!("skin-swatch {}", if is_selected() { "selected" } else { "" })
                        style=format!("background-color: {}", bg_color)
                        title=label
                        on:click=move |_| on_change.with_value(|f| f(opt_cloned.clone()))
                    ></div>
                }
            }).collect_view()}
        </div>
    }
}
