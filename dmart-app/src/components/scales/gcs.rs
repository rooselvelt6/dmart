use leptos::prelude::*;
use dmart_shared::models::*;

#[component]
pub fn GcsScale(
    data: RwSignal<GcsData>,
) -> impl IntoView {
    let eye = move || data.get().apertura_ocular;
    let verbal = move || data.get().respuesta_verbal;
    let motor = move || data.get().respuesta_motora;
    let total = move || data.get().total();

    view! {
        <div class="glass-card p-6 sm:p-10 border-purple-500/20 bg-white shadow-xl animate-fade-in">
            <div class="flex flex-col lg:flex-row gap-10">
                <div class="flex-1 space-y-12">
                    <div class="flex items-center gap-4 mb-6">
                        <div class="w-12 h-12 rounded-2xl bg-purple-500/10 flex items-center justify-center text-purple-600 text-2xl">
                            <i class="fa-solid fa-brain"></i>
                        </div>
                        <div>
                            <h3 class="text-2xl font-black text-uci-text tracking-tight uppercase">"Glasgow"</h3>
                            <p class="text-xs font-bold text-uci-muted tracking-widest uppercase">"GCS — Escala de Coma de Glasgow"</p>
                        </div>
                    </div>

                    <div class="space-y-10">
                        <GcsSlider label="Respuesta Ocular (E)" icon="fa-eye" min=1 max=4 
                            value=Signal::derive(move || eye())
                            on_change=Callback::new(move |v| data.update(|d| d.apertura_ocular = v))
                            labels=&["Ninguna", "Al dolor", "A la voz", "Espontánea"] />

                        <GcsSlider label="Respuesta Verbal (V)" icon="fa-comment" min=1 max=5 
                            value=Signal::derive(move || verbal())
                            on_change=Callback::new(move |v| data.update(|d| d.respuesta_verbal = v))
                            labels=&["Ninguna", "Sonidos incomprensibles", "Palabras inapropiadas", "Confuso", "Orientado"] />

                        <GcsSlider label="Respuesta Motora (M)" icon="fa-hand-pointer" min=1 max=6 
                            value=Signal::derive(move || motor())
                            on_change=Callback::new(move |v| data.update(|d| d.respuesta_motora = v))
                            labels=&["Ninguna", "Extensión", "Flexión anormal", "Retirada al dolor", "Localiza", "Obedece"] />
                    </div>
                </div>

                <div class="lg:w-80 flex flex-col gap-6">
                    <div class="bg-uci-bg rounded-3xl p-8 border-2 border-purple-500/30 shadow-inner text-center space-y-4">
                        <div class="text-xs font-black text-purple-400 uppercase tracking-[0.2em]">"Total GCS"</div>
                        <div class="text-8xl font-black text-uci-text font-mono leading-none tracking-tighter">
                            {move || total()}
                            <span class="text-2xl text-uci-muted">"/15"</span>
                        </div>
                        <div class="flex justify-center gap-1">
                            <span class="text-xs font-black bg-purple-500/10 text-purple-600 px-2 py-0.5 rounded">"E" {eye}</span>
                            <span class="text-xs font-black bg-purple-500/10 text-purple-600 px-2 py-0.5 rounded">"V" {verbal}</span>
                            <span class="text-xs font-black bg-purple-500/10 text-purple-600 px-2 py-0.5 rounded">"M" {motor}</span>
                        </div>
                    </div>

                    <div class="p-6 bg-purple-600 text-white rounded-2xl shadow-lg shadow-purple-600/20">
                        <div class="text-[10px] font-black uppercase tracking-widest mb-2 opacity-80">"Interpretación"</div>
                        <div class="text-lg font-black">
                            {move || match total() {
                                13..=15 => "Trauma Leve",
                                9..=12 => "Trauma Moderado",
                                _ => "Trauma Severo (Coma)",
                            }}
                        </div>
                    </div>
                </div>
            </div>
        </div>
    }
}

#[component]
fn GcsSlider(
    label: &'static str,
    icon: &'static str,
    min: u8,
    max: u8,
    value: Signal<u8>,
    on_change: Callback<u8>,
    labels: &'static [&'static str],
) -> impl IntoView {
    let val_pct = move || {
        let v = value.get();
        ((v - min) as f32 / (max - min) as f32 * 100.0).clamp(0.0, 100.0)
    };

    view! {
        <div class="space-y-4">
            <div class="flex justify-between items-center">
                <label class="text-[11px] font-black text-uci-muted uppercase tracking-[0.1em] flex items-center gap-2">
                    <i class=format!("fa-solid {}", icon)></i>
                    {label}
                </label>
                <span class="text-sm font-black text-purple-600 bg-purple-50 px-3 py-1 rounded-lg">
                    {move || format!("{} - {}", value.get(), labels.get((value.get() - min) as usize).unwrap_or(&""))}
                </span>
            </div>
            <div class="relative px-2">
                <div class="absolute inset-0 flex justify-between px-2 pointer-events-none">
                    {(min..=max).map(|_| view! { <div class="w-1 h-1 rounded-full bg-uci-border mt-3"></div> }).collect_view()}
                </div>
                <input 
                    type="range" class="w-full relative z-10"
                    min=min max=max step=1
                    prop:value=move || value.get()
                    on:input=move |ev| {
                        if let Ok(v) = event_target_value(&ev).parse::<u8>() {
                            on_change.run(v);
                        }
                    }
                    style=move || format!("--val: {}%", val_pct())
                    data-sev="normal"
                />
            </div>
        </div>
    }
}
