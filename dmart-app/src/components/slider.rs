use leptos::prelude::*;

#[component]
pub fn ScaleSlider<F>(
    label: &'static str,
    value: Signal<f32>,
    on_change: F,
    min: f32,
    max: f32,
    step: f32,
    unit: &'static str,
    #[prop(optional)] severity: Option<Signal<&'static str>>,
) -> impl IntoView
where
    F: Fn(f32) + 'static,
{
    let percentage = move || {
        let v = value.get();
        let p = ((v - min) / (max - min)) * 100.0;
        p.clamp(0.0, 100.0)
    };

    view! {
        <div class="mb-6">
            <div class="flex justify-between items-center mb-2">
                <label class="form-label mb-0">{label}</label>
                <div class="text-lg font-mono font-bold text-uci-accent">
                    {move || format!("{:.1} {}", value.get(), unit)}
                </div>
            </div>
            <input
                type="range"
                class="w-full"
                min=min
                max=max
                step=step
                prop:value=move || value.get().to_string()
                on:input=move |ev| {
                    let val = event_target_value(&ev).parse::<f32>().unwrap_or(0.0);
                    on_change(val);
                }
                data-sev=move || severity.map(|s| s.get()).unwrap_or("normal")
                style=move || format!("--val: {}%", percentage())
            />
            <div class="flex justify-between mt-1 text-[10px] text-uci-muted font-mono">
                <span>{min}</span>
                <span>{max}</span>
            </div>
        </div>
    }
}
