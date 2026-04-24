use leptos::prelude::*;
use dmart_shared::models::*;

#[component]
pub fn GcsScale(
    data: RwSignal<GcsData>,
) -> impl IntoView {
    let eye_val = Signal::derive(move || data.get().apertura_ocular);
    let eye_set = Callback::new(move |v| data.update(|d| d.apertura_ocular = v));
    
    let verbal_val = Signal::derive(move || data.get().respuesta_verbal);
    let verbal_set = Callback::new(move |v| data.update(|d| d.respuesta_verbal = v));
    
    let motor_val = Signal::derive(move || data.get().respuesta_motora);
    let motor_set = Callback::new(move |v| data.update(|d| d.respuesta_motora = v));
    
    let total = Signal::derive(move || data.get().apertura_ocular + data.get().respuesta_verbal + data.get().respuesta_motora);

    view! {
        <div class="glass-card p-4" style="background:var(--uci-card); border:1px solid var(--uci-border);">
            <div class="flex items-center gap-3 mb-4">
                <div class="w-10 h-10 rounded-lg flex items-center justify-center bg-purple-500">
                    <i class="fa-solid fa-brain text-white"></i>
                </div>
                <div>
                    <h3 class="text-xl font-black uppercase" style="color:var(--uci-text);">"Glasgow"</h3>
                    <p class="text-[10px] font-bold" style="color:var(--uci-muted);">"Escala de Coma"</p>
                </div>
            </div>

            <div class="space-y-4">
                <GcsSlider label="Ojos (E)" color="#10B981" min=1 max=4 value=eye_val set_value=eye_set />
                <GcsSlider label="Verbal (V)" color="#F59E0B" min=1 max=5 value=verbal_val set_value=verbal_set />
                <GcsSlider label="Motor (M)" color="#8B5CF6" min=1 max=6 value=motor_val set_value=motor_set />
            </div>

            <div class="mt-4 text-center p-4 rounded-xl" style="background:linear-gradient(135deg, #8B5CF6, #A855F7); color:white;">
                <div class="text-xs font-bold uppercase opacity-80">"Total GCS"</div>
                <div class="text-5xl font-black">{move || total.get()}</div>
                <div class="text-sm opacity-70">"/15"</div>
            </div>

            <div class="mt-4 p-3 rounded-lg" style="background:var(--uci-surface);">
                <div class="text-center font-bold text-sm" style="color:var(--uci-text);">
                    {move || match total.get() {
                        13..=15 => "Trauma Leve",
                        9..=12 => "Trauma Moderado",
                        _ => "Trauma Severo",
                    }}
                </div>
            </div>
        </div>
    }
}

#[component]
fn GcsSlider(
    label: &'static str,
    color: &'static str,
    min: u8,
    max: u8,
    value: Signal<u8>,
    set_value: Callback<u8>,
) -> impl IntoView {
    view! {
        <div class="p-3 rounded-lg" style="background:var(--uci-surface);">
            <div class="flex justify-between items-center mb-2">
                <label class="text-sm font-bold flex items-center gap-2" style="color:var(--uci-muted);">
                    <i class="fa-solid fa-brain" style=format!("color:{};", color)></i>
                    {label}
                </label>
                <span class="text-2xl font-black" style=format!("color:{};", color)>
                    {move || value.get()}
                </span>
            </div>
            <input 
                type="range" class="w-full h-2 rounded-lg appearance-none cursor-pointer"
                min=min max=max step=1
                prop:value=move || value.get()
                on:input=move |ev| {
                    if let Ok(v) = event_target_value(&ev).parse::<u8>() {
                        set_value.run(v);
                    }
                }
            />
        </div>
    }
}