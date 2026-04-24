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
        <div class="glass-card p-3 sm:p-4 lg:p-6 border-indigo-500/20 shadow-xl" style="background:var(--uci-card); overflow:hidden;">
            <div class="flex flex-col xl:flex-row gap-4 xl:gap-8">
                <div class="flex-1 min-w-0">
                    <div class="flex items-center gap-3 sm:gap-5 mb-4 sm:mb-6">
                        <div class="w-10 h-10 sm:w-12 sm:h-12 rounded-xl lg:rounded-2xl flex items-center justify-center text-xl sm:text-2xl" 
                             style="background:linear-gradient(135deg,#6366F1,#8B5CF6); color:white;">
                            <i class="fa-solid fa-chart-line"></i>
                        </div>
                        <div>
                            <h3 class="text-2xl sm:text-3xl lg:text-4xl font-black text-[var(--uci-text)] tracking-tight uppercase">"SAPS III"</h3>
                            <p class="text-xs sm:text-sm lg:text-lg font-semibold" style="color:var(--uci-muted);">"Simplified Acute Physiology"</p>
                        </div>
                    </div>

                    <div class="grid grid-cols-3 gap-2 sm:gap-4 p-3 sm:p-4 rounded-xl lg:rounded-2xl mb-4 sm:mb-6" style="background:var(--uci-surface);">
                        <div class="text-center">
                            <div class="text-xs sm:text-sm font-black" style="color:#6366F1;">"Box 1"</div>
                            <div class="text-xl sm:text-2xl lg:text-3xl font-black" style="color:var(--uci-text);">{move || breakdown.get().box1}</div>
                            <div class="text-[10px] sm:text-xs" style="color:var(--uci-muted);">"Antecedentes"</div>
                        </div>
                        <div class="text-center border-x" style="border-color:var(--uci-border);">
                            <div class="text-xs sm:text-sm font-black" style="color:#6366F1;">"Box 2"</div>
                            <div class="text-xl sm:text-2xl lg:text-3xl font-black" style="color:var(--uci-text);">{move || breakdown.get().box2}</div>
                            <div class="text-[10px] sm:text-xs" style="color:var(--uci-muted);">"Circunstancias"</div>
                        </div>
                        <div class="text-center">
                            <div class="text-xs sm:text-sm font-black" style="color:#6366F1;">"Box 3"</div>
                            <div class="text-xl sm:text-2xl lg:text-3xl font-black" style="color:var(--uci-text);">{move || breakdown.get().box3}</div>
                            <div class="text-[10px] sm:text-xs" style="color:var(--uci-muted);">"Fisiología"</div>
                        </div>
                    </div>

                    <div class="grid grid-cols-1 sm:grid-cols-2 gap-3 sm:gap-4 lg:gap-6">
                        <SapsMetric label="GCS" icon="fa-brain" 
                            value=Signal::derive(move || data.get().gcs_total as f32)
                            min=3.0 max=15.0 step=1.0
                            on_change=Callback::new(move |v| data.update(|d| d.gcs_total = v as u8)) />

                        <SapsMetric label="FC" icon="fa-heartbeat" 
                            value=Signal::derive(move || data.get().frecuencia_cardiaca)
                            min=30.0 max=200.0 step=1.0
                            on_change=Callback::new(move |v| data.update(|d| d.frecuencia_cardiaca = v)) />

                        <SapsMetric label="PAS" icon="fa-gauge-high" 
                            value=Signal::derive(move || data.get().presion_sistolica)
                            min=40.0 max=250.0 step=1.0
                            on_change=Callback::new(move |v| data.update(|d| d.presion_sistolica = v)) />

                        <SapsMetric label="Temp" icon="fa-thermometer" 
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

                <div class="xl:w-72 lg:w-full shrink-0">
                    <div class="rounded-2xl xl:rounded-3xl p-4 sm:p-6 lg:p-8 border-2 shadow-inner text-center"
                         style="background:linear-gradient(135deg,rgba(99,102,241,0.1),rgba(139,92,246,0.05)); border-color:rgba(99,102,241,0.3);">
                        <div class="text-xs sm:text-sm lg:text-lg font-black uppercase tracking-wider" style="color:#6366F1;">"SAPS III"</div>
                        <div class="text-5xl sm:text-6xl lg:text-7xl font-black font-mono leading-none tracking-tighter" style="color:var(--uci-text);">
                            {move || score.get()}
                        </div>
                        <div class="text-sm sm:text-lg lg:text-xl font-black uppercase tracking-widest mt-2" style=move || format!("color:{};", level.get().color_class())>
                            {move || level.get().label()}
                        </div>
                    </div>

                    <div class="rounded-xl lg:rounded-2xl p-3 sm:p-4 mt-3 sm:mt-4 border" style="background:var(--uci-surface); border-color:var(--uci-border);">
                        <div class="text-xs sm:text-sm font-black uppercase tracking-wider" style="color:var(--uci-muted);">"Nota"</div>
                        <p class="text-xs sm:text-sm leading-relaxed" style="color:var(--uci-muted);">
                            "SAPS III ajusta mortalidad según región geográfica."
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
    view! {
        <div class="p-3 sm:p-4 rounded-xl border" style="background:var(--uci-surface); border-color:var(--uci-border);">
            <div class="flex justify-between items-center gap-2">
                <label class="text-xs sm:text-sm font-bold flex items-center gap-2" style="color:var(--uci-muted);">
                    <i class=format!("fa-solid {}", icon)></i>
                    {label}
                </label>
                <span class="text-sm sm:text-lg font-bold font-mono" style="color:#6366F1;">
                    {move || if step < 1.0 { format!("{:.1}", value.get()) } else { format!("{:.0}", value.get()) }}
                </span>
            </div>
            <input 
                type="range" class="w-full h-2"
                min=min max=max step=step
                prop:value=move || value.get()
                on:input=move |ev| {
                    if let Ok(v) = event_target_value(&ev).parse::<f32>() {
                        on_change.run(v);
                    }
                }
            />
        </div>
    }
}