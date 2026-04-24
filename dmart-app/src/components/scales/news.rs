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

    let level_color = move || {
        match level.get() {
            News2Level::Bajo => "#10B981",
            News2Level::Medio => "#F59E0B",
            News2Level::Alto => "#EF4444",
            News2Level::Emergent => "#DC2626",
        }
    };

    view! {
        <div class="glass-card p-3 sm:p-4 lg:p-6 border-amber-500/20 shadow-xl" style="background:var(--uci-card); overflow:hidden;">
            <div class="flex flex-col xl:flex-row gap-4 xl:gap-8">
                <div class="flex-1 min-w-0">
                    <div class="flex items-center gap-3 sm:gap-5 mb-4 sm:mb-6">
                        <div class="w-10 h-10 sm:w-14 sm:h-14 rounded-xl lg:rounded-2xl flex items-center justify-center text-xl lg:text-3xl shadow-lg" 
                             style="background:linear-gradient(135deg,#F59E0B,#D97706); color:white;">
                            <i class="fa-solid fa-bell"></i>
                        </div>
                        <div>
                            <h3 class="text-2xl sm:text-3xl lg:text-4xl font-black text-[var(--uci-text)] tracking-tight uppercase">"NEWS2"</h3>
                            <p class="text-xs sm:text-sm lg:text-lg font-semibold" style="color:var(--uci-muted);">"National Early Warning Score"</p>
                        </div>
                    </div>

                    <div class="grid grid-cols-1 sm:grid-cols-2 gap-3 sm:gap-4 lg:gap-6">
                        <NewsMetric label="FR" icon="fa-wind" color="#06B6D4" _pts=Signal::derive(move || breakdown.get().fr)
                            value=Signal::derive(move || data.get().frecuencia_respiratoria)
                            min=5.0 max=40.0 step=1.0
                            on_change=Callback::new(move |v| data.update(|d| d.frecuencia_respiratoria = v)) />

                        <NewsMetric label="SpO2" icon="fa-droplet" color="#3B82F6" _pts=Signal::derive(move || breakdown.get().spo2)
                            value=Signal::derive(move || data.get().spo2)
                            min=70.0 max=100.0 step=1.0
                            on_change=Callback::new(move |v| data.update(|d| d.spo2 = v)) />

                        <NewsMetric label="PAS" icon="fa-gauge-high" color="#F97316" _pts=Signal::derive(move || breakdown.get().pas)
                            value=Signal::derive(move || data.get().presion_sistolica)
                            min=70.0 max=250.0 step=1.0
                            on_change=Callback::new(move |v| data.update(|d| d.presion_sistolica = v)) />

                        <NewsMetric label="FC" icon="fa-heartbeat" color="#EC4899" _pts=Signal::derive(move || breakdown.get().fc)
                            value=Signal::derive(move || data.get().frecuencia_cardiaca)
                            min=30.0 max=160.0 step=1.0
                            on_change=Callback::new(move |v| data.update(|d| d.frecuencia_cardiaca = v)) />

                        <NewsMetric label="Temp" icon="fa-temperature-half" color="#EF4444" _pts=Signal::derive(move || breakdown.get().temp)
                            value=Signal::derive(move || data.get().temperatura)
                            min=33.0 max=42.0 step=0.1
                            on_change=Callback::new(move |v| data.update(|d| d.temperatura = v)) />
                        
                        <div class="p-3 sm:p-4 rounded-xl border col-span-1 sm:col-span-2" style="background:var(--uci-surface); border-color:rgba(245,158,11,0.2);">
                            <label class="text-sm sm:block" style="color:var(--uci-muted);">"Oxígeno Suplementario"</label>
                            <div class="flex gap-2 sm:gap-3 mt-2">
                                <button 
                                    type="button"
                                    class="flex-1 py-2 sm:py-3 px-2 sm:px-4 rounded-lg sm:rounded-xl font-bold text-xs sm:text-sm transition-all"
                                    style=move || if data.get().o2_suplementario { 
                                        "background:#F59E0B; color:white; box-shadow:0 4px 12px rgba(245,158,11,0.3);" 
                                    } else { 
                                        "background:var(--uci-surface); border:1px solid var(--uci-border); color:var(--uci-text);" 
                                    }
                                    on:click=move |_| data.update(|d| d.o2_suplementario = true)>
                                    <i class="fa-solid fa-check-circle mr-1 sm:mr-2"></i>"Sí (+2)"
                                </button>
                                <button 
                                    type="button"
                                    class="flex-1 py-2 sm:py-3 px-2 sm:px-4 rounded-lg sm:rounded-xl font-bold text-xs sm:text-sm transition-all"
                                    style=move || if !data.get().o2_suplementario { 
                                        "background:var(--uci-muted); color:white; box-shadow:0 4px 12px rgba(100,116,139,0.3);" 
                                    } else { 
                                        "background:var(--uci-surface); border:1px solid var(--uci-border); color:var(--uci-text);" 
                                    }
                                    on:click=move |_| data.update(|d| d.o2_suplementario = false)>
                                    "Aire Ambiente"
                                </button>
                            </div>
                        </div>
                    </div>
                </div>

                <div class="xl:w-80 lg:w-full shrink-0">
                    <div class="rounded-2xl xl:rounded-3xl p-4 sm:p-6 lg:p-8 border-2 shadow-inner text-center"
                         style="background:linear-gradient(135deg,rgba(245,158,11,0.1),rgba(217,119,6,0.05)); border-color:rgba(245,158,11,0.3);">
                        <div class="text-xs sm:text-sm lg:text-lg font-black uppercase tracking-wider" style="color:var(--uci-muted);">"NEWS2"</div>
                        <div class="text-5xl sm:text-6xl lg:text-8xl font-black font-mono leading-none tracking-tighter" style="color:#F59E0B;">
                            {move || score.get()}
                        </div>
                        <div class="text-sm sm:text-lg lg:text-xl font-black uppercase tracking-widest mt-2" style=move || format!("color:{};", level_color())>
                            {move || level.get().label()}
                        </div>
                    </div>

                    <div class="rounded-xl lg:rounded-2xl p-4 sm:p-6 mt-3 sm:mt-4 shadow-lg"
                         style="background:linear-gradient(135deg,#F59E0B,#D97706);">
                        <div class="flex items-center gap-2 sm:gap-3 mb-2">
                            <i class="fa-solid fa-hand-holding-medical text-lg sm:text-xl"></i>
                            <div class="text-xs sm:text-base font-black uppercase tracking-wider text-white/80">"Respuesta"</div>
                        </div>
                        <p class="text-sm sm:text-lg lg:text-xl font-bold leading-relaxed text-white">
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
    color: &'static str,
    _pts: Signal<u32>,
    value: Signal<f32>,
    min: f32,
    max: f32,
    step: f32,
    on_change: Callback<f32>,
) -> impl IntoView {
    view! {
        <div class="p-3 sm:p-4 rounded-xl border" style="background:var(--uci-surface); border-color:color-mix(in srgb, {} 20%, transparent);">
            <div class="flex justify-between items-center gap-2">
                <label class="text-sm sm:text-lg font-bold flex items-center gap-2" style="color:var(--uci-muted);">
                    <i class=format!("fa-solid {}", icon) style=move || format!("color:{};", color)></i>
                    {label}
                </label>
                <div class="flex items-center gap-2">
                    <span class="text-lg sm:text-2xl font-bold font-mono"
                          style=move || format!("color:{};", color)>
                        {move || if step < 1.0 { format!("{:.1}", value.get()) } else { format!("{:.0}", value.get()) }}
                    </span>
                </div>
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