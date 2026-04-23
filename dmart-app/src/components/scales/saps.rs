use leptos::prelude::*;
use dmart_shared::models::*;
use dmart_shared::scales::*;

#[component]
pub fn Saps3Scale(
    data: RwSignal<ApacheIIData>,
) -> impl IntoView {
    let breakdown = Memo::new(move |_| calculate_saps3_breakdown(&data.get()));
    let score = Memo::new(move |_| breakdown.get().total);
    let level = Memo::new(move |_| Saps3Level::from_score(score.get()));

    view! {
        <div class="glass-card p-6 sm:p-10 border-indigo-500/20 bg-white shadow-xl animate-fade-in">
            <div class="flex flex-col lg:flex-row gap-10">
                <div class="flex-1 space-y-8">
                    <div class="flex items-center gap-4 mb-6">
                        <div class="w-12 h-12 rounded-2xl bg-indigo-500/10 flex items-center justify-center text-indigo-600 text-2xl">
                            <i class="fa-solid fa-chart-line"></i>
                        </div>
                        <div>
                            <h3 class="text-2xl font-black text-uci-text tracking-tight uppercase">"SAPS III"</h3>
                            <p class="text-xs font-bold text-uci-muted tracking-widest uppercase">"Simplified Acute Physiology Score III"</p>
                        </div>
                    </div>

                    <div class="grid grid-cols-1 md:grid-cols-3 gap-6 bg-indigo-50/50 p-6 rounded-3xl border border-indigo-100/50 mb-8">
                        <div class="text-center">
                            <div class="text-[10px] font-black text-indigo-400 uppercase tracking-widest mb-1">"Caja 1"</div>
                            <div class="text-2xl font-black text-indigo-900">{move || breakdown.get().box1}</div>
                            <div class="text-[9px] text-indigo-400 font-bold uppercase">"Antecedentes"</div>
                        </div>
                        <div class="text-center border-x border-indigo-200/50 px-4">
                            <div class="text-[10px] font-black text-indigo-400 uppercase tracking-widest mb-1">"Caja 2"</div>
                            <div class="text-2xl font-black text-indigo-900">{move || breakdown.get().box2}</div>
                            <div class="text-[9px] text-indigo-400 font-bold uppercase">"Circunstancias"</div>
                        </div>
                        <div class="text-center">
                            <div class="text-[10px] font-black text-indigo-400 uppercase tracking-widest mb-1">"Caja 3"</div>
                            <div class="text-2xl font-black text-indigo-900">{move || breakdown.get().box3}</div>
                            <div class="text-[9px] text-indigo-400 font-bold uppercase">"Fisiología"</div>
                        </div>
                    </div>

                    <div class="grid grid-cols-1 md:grid-cols-2 gap-8">
                        <SapsMetric label="GCS" icon="fa-brain" 
                            value=Signal::derive(move || data.get().gcs_total as f32)
                            min=3.0 max=15.0 step=1.0
                            on_change=Callback::new(move |v| data.update(|d| d.gcs_total = v as u8)) />

                        <SapsMetric label="Frecuencia Cardíaca" icon="fa-heartbeat" 
                            value=Signal::derive(move || data.get().frecuencia_cardiaca)
                            min=30.0 max=200.0 step=1.0
                            on_change=Callback::new(move |v| data.update(|d| d.frecuencia_cardiaca = v)) />

                        <SapsMetric label="Presión Sistólica" icon="fa-gauge-high" 
                            value=Signal::derive(move || data.get().presion_sistolica)
                            min=40.0 max=250.0 step=1.0
                            on_change=Callback::new(move |v| data.update(|d| d.presion_sistolica = v)) />

                        <SapsMetric label="Temperatura" icon="fa-thermometer" 
                            value=Signal::derive(move || data.get().temperatura)
                            min=33.0 max=42.0 step=0.1
                            on_change=Callback::new(move |v| data.update(|d| d.temperatura = v)) />

                        <SapsMetric label="Creatinina" icon="fa-vial" 
                            value=Signal::derive(move || data.get().creatinina)
                            min=0.1 max=10.0 step=0.1
                            on_change=Callback::new(move |v| data.update(|d| d.creatinina = v)) />

                        <SapsMetric label="Bilirrubina" icon="fa-vial-circle-check" 
                            value=Signal::derive(move || data.get().bilirrubina)
                            min=0.1 max=20.0 step=0.1
                            on_change=Callback::new(move |v| data.update(|d| d.bilirrubina = v)) />
                    </div>
                </div>

                <div class="lg:w-80 flex flex-col gap-6">
                    <div class="bg-uci-bg rounded-3xl p-8 border-2 border-indigo-500/30 shadow-inner text-center space-y-4">
                        <div class="text-xs font-black text-indigo-500 uppercase tracking-[0.2em]">"Score SAPS III"</div>
                        <div class="text-7xl font-black text-uci-text font-mono leading-none tracking-tighter">
                            {move || score.get()}
                        </div>
                        <div class=move || format!("text-lg font-black uppercase tracking-widest {}", level.get().color_class())>
                            {move || level.get().label()}
                        </div>
                    </div>

                    <div class="p-6 border border-uci-border rounded-2xl bg-white space-y-4">
                        <h4 class="text-xs font-black text-uci-muted uppercase tracking-widest">"Nota del Estándar"</h4>
                        <p class="text-[10px] font-medium text-uci-muted leading-relaxed">
                            "SAPS III permite ajustar la probabilidad de mortalidad según la region geográfica. Utiliza las variables medidas en la primera hora de ingreso a UCI."
                        </p>
                    </div>
                </div>
            </div>
        </div>
    }
}

#[component]
fn SapsMetric(
    label: &'static str,
    icon: &'static str,
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
                <span class="text-xs font-black text-indigo-600 bg-indigo-50 px-2 py-0.5 rounded">
                    {move || format!("{:.1}", value.get())}
                </span>
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
