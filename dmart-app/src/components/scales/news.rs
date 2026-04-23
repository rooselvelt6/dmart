use leptos::prelude::*;
use dmart_shared::models::*;
use dmart_shared::scales::*;

#[component]
pub fn News2Scale(
    data: RwSignal<ApacheIIData>,
) -> impl IntoView {
    let breakdown = Memo::new(move |_| news2_breakdown(&data.get()));
    let score = Memo::new(move |_| breakdown.get().total);
    let level = Memo::new(move |_| News2Level::from_score(score.get()));

    view! {
        <div class="glass-card p-6 sm:p-10 border-amber-500/20 bg-white shadow-xl animate-fade-in">
            <div class="flex flex-col lg:flex-row gap-10">
                <div class="flex-1 space-y-8">
                    <div class="flex items-center gap-4 mb-6">
                        <div class="w-12 h-12 rounded-2xl bg-amber-500/10 flex items-center justify-center text-amber-600 text-2xl">
                            <i class="fa-solid fa-bell"></i>
                        </div>
                        <div>
                            <h3 class="text-2xl font-black text-uci-text tracking-tight uppercase">"NEWS2"</h3>
                            <p class="text-xs font-bold text-uci-muted tracking-widest uppercase">"National Early Warning Score 2"</p>
                        </div>
                    </div>

                    <div class="grid grid-cols-1 md:grid-cols-2 gap-8">
                        <NewsMetric label="Frecuencia Respiratoria" icon="fa-wind" pts=Signal::derive(move || breakdown.get().fr)
                            value=Signal::derive(move || data.get().frecuencia_respiratoria)
                            min=5.0 max=40.0 step=1.0
                            on_change=Callback::new(move |v| data.update(|d| d.frecuencia_respiratoria = v)) />

                        <NewsMetric label="Saturación de Oxígeno (SpO2)" icon="fa-droplet" pts=Signal::derive(move || breakdown.get().spo2)
                            value=Signal::derive(move || data.get().spo2)
                            min=70.0 max=100.0 step=1.0
                            on_change=Callback::new(move |v| data.update(|d| d.spo2 = v)) />

                        <NewsMetric label="Presión Sistólica" icon="fa-gauge-high" pts=Signal::derive(move || breakdown.get().pas)
                            value=Signal::derive(move || data.get().presion_sistolica)
                            min=70.0 max=250.0 step=1.0
                            on_change=Callback::new(move |v| data.update(|d| d.presion_sistolica = v)) />

                        <NewsMetric label="Frecuencia Cardíaca" icon="fa-heartbeat" pts=Signal::derive(move || breakdown.get().fc)
                            value=Signal::derive(move || data.get().frecuencia_cardiaca)
                            min=30.0 max=160.0 step=1.0
                            on_change=Callback::new(move |v| data.update(|d| d.frecuencia_cardiaca = v)) />

                        <NewsMetric label="Temperatura" icon="fa-thermometer" pts=Signal::derive(move || breakdown.get().temp)
                            value=Signal::derive(move || data.get().temperatura)
                            min=33.0 max=42.0 step=0.1
                            on_change=Callback::new(move |v| data.update(|d| d.temperatura = v)) />
                            
                        <div class="bg-amber-50/50 p-4 rounded-2xl border border-amber-200/50">
                            <label class="text-[10px] font-black text-uci-muted uppercase tracking-widest mb-3 block">"Oxígeno Suplementario"</label>
                            <div class="flex items-center gap-4">
                                <button 
                                    type="button"
                                    class=move || format!("px-4 py-2 rounded-xl font-bold transition-all {}", if data.get().o2_suplementario { "bg-amber-500 text-white shadow-md shadow-amber-500/30" } else { "bg-white border border-uci-border text-uci-muted" })
                                    on:click=move |_| data.update(|d| d.o2_suplementario = true)>
                                    "Sí (+2)"
                                </button>
                                <button 
                                    type="button"
                                    class=move || format!("px-4 py-2 rounded-xl font-bold transition-all {}", if !data.get().o2_suplementario { "bg-uci-muted text-white shadow-md shadow-slate-500/30" } else { "bg-white border border-uci-border text-uci-muted" })
                                    on:click=move |_| data.update(|d| d.o2_suplementario = false)>
                                    "Aire Ambiente"
                                </button>
                            </div>
                        </div>
                    </div>
                </div>

                <div class="lg:w-80 flex flex-col gap-6">
                    <div class="bg-uci-bg rounded-3xl p-8 border-2 border-amber-500/30 shadow-inner text-center space-y-4">
                        <div class="text-xs font-black text-amber-500 uppercase tracking-[0.2em]">"NEWS2 Score"</div>
                        <div class="text-7xl font-black text-uci-text font-mono leading-none tracking-tighter">
                            {move || score.get()}
                        </div>
                        <div class=move || format!("text-lg font-black uppercase tracking-widest {}", level.get().color_class())>
                            {move || level.get().label()}
                        </div>
                    </div>

                    <div class="bg-amber-600 text-white rounded-2xl p-6 shadow-lg shadow-amber-600/20 space-y-3">
                        <div class="flex items-center gap-2">
                            <i class="fa-solid fa-hand-holding-medical"></i>
                            <span class="text-xs font-black uppercase tracking-widest">"Respuesta Clínica"</span>
                        </div>
                        <p class="text-sm font-bold leading-relaxed">
                            {move || level.get().response()}
                        </p>
                    </div>
                </div>
            </div>
        </div>
    }
}

#[component]
fn NewsMetric(
    label: &'static str,
    icon: &'static str,
    pts: Signal<u32>,
    value: Signal<f32>,
    min: f32,
    max: f32,
    step: f32,
    on_change: Callback<f32>,
) -> impl IntoView {
    let val_pct = move || {
        let v = value.get();
        ((v - min) / (max - min) * 100.0).clamp(0.0, 100.0)
    };

    view! {
        <div class="space-y-3 group">
            <div class="flex justify-between items-center">
                <label class="text-[10px] font-black text-uci-muted uppercase tracking-widest flex items-center gap-2">
                    <i class=format!("fa-solid {}", icon)></i>
                    {label}
                </label>
                <div class="flex items-center gap-2">
                    <span class="text-xs font-bold text-uci-muted">
                        {move || format!("{:.1}", value.get())}
                    </span>
                    <span class="text-xs font-black bg-amber-500 text-white px-2 py-0.5 rounded-lg shadow-sm">
                        {move || format!("+{}", pts.get())}
                    </span>
                </div>
            </div>
            <div class="relative py-2">
                <input 
                    type="range" class="w-full"
                    min=min max=max step=step
                    prop:value=move || value.get()
                    on:input=move |ev| {
                        if let Ok(v) = event_target_value(&ev).parse::<f32>() {
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
