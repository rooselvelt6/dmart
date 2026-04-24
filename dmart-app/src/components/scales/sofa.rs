use leptos::prelude::*;
use dmart_shared::models::*;
use dmart_shared::scales::*;

#[component]
pub fn SofaScale(
    data: RwSignal<ApacheIIData>,
) -> impl IntoView {
    let breakdown = Memo::new(move |_| sofa_breakdown(&data.get()));
    let score = Memo::new(move |_| breakdown.get().total);
    let level = Memo::new(move |_| SofaLevel::from_score(score.get()));
    let mortality = Memo::new(move |_| sofa_mortality_estimate(score.get()));

    let level_color = move || {
        match level.get() {
            SofaLevel::Normal => "#10B981",
            SofaLevel::Disfuncion => "#F59E0B",
            SofaLevel::Falla => "#EF4444",
            SofaLevel::FallaMultiorganica => "#DC2626",
        }
    };

    view! {
        <div class="glass-card p-3 sm:p-4 lg:p-6 border-emerald-500/20 shadow-xl" style="background:var(--uci-card); overflow:hidden;">
            <div class="flex flex-col xl:flex-row gap-4 xl:gap-8">
                <div class="flex-1 min-w-0">
                    <div class="flex items-center gap-3 sm:gap-5 mb-4 sm:mb-6">
                        <div class="w-10 h-10 sm:w-14 sm:h-14 rounded-xl lg:rounded-2xl flex items-center justify-center text-xl lg:text-3xl shadow-lg" 
                             style="background:linear-gradient(135deg,#10B981,#059669); color:white;">
                            <i class="fa-solid fa-lungs"></i>
                        </div>
                        <div>
                            <h3 class="text-2xl sm:text-3xl lg:text-4xl font-black text-[var(--uci-text)] tracking-tight uppercase">"SOFA"</h3>
                            <p class="text-xs sm:text-sm lg:text-lg font-semibold" style="color:var(--uci-muted);">"Sequential Organ Failure"</p>
                        </div>
                    </div>

                    <div class="grid grid-cols-1 sm:grid-cols-2 gap-3 sm:gap-4 lg:gap-6">
                        <SofaMetric label="Respiratorio (PaO2/FiO2)" icon="fa-wind" color="#06B6D4" _pts=Signal::derive(move || breakdown.get().respiratorio)
                            value=Signal::derive(move || {
                                let d = data.get();
                                let pao2 = d.pao2.unwrap_or(80.0);
                                if d.fio2 > 0.0 { pao2 / d.fio2 } else { 400.0 }
                            })
                            min=50.0 max=500.0 step=10.0
                            on_change=Callback::new(move |v| data.update(|d| d.pao2 = Some(v * d.fio2))) />

                        <SofaMetric label="Cardiovascular (PAM)" icon="fa-heart-pulse" color="#EC4899" _pts=Signal::derive(move || breakdown.get().cardiovascular)
                            value=Signal::derive(move || data.get().presion_arterial_media)
                            min=40.0 max=110.0 step=1.0
                            on_change=Callback::new(move |v| data.update(|d| d.presion_arterial_media = v)) />

                        <SofaMetric label="Plaquetas" icon="fa-droplet" color="#DC2626" _pts=Signal::derive(move || breakdown.get().coagulacion)
                            value=Signal::derive(move || data.get().plaquetas)
                            min=10.0 max=200.0 step=5.0
                            on_change=Callback::new(move |v| data.update(|d| d.plaquetas = v)) />

                        <SofaMetric label="Bilirrubina" icon="fa-vial" color="#84CC16" _pts=Signal::derive(move || breakdown.get().hepatico)
                            value=Signal::derive(move || data.get().bilirrubina)
                            min=0.1 max=15.0 step=0.1
                            on_change=Callback::new(move |v| data.update(|d| d.bilirrubina = v)) />

                        <SofaMetric label="Neurológico (GCS)" icon="fa-brain" color="#A855F7" _pts=Signal::derive(move || breakdown.get().neurologico)
                            value=Signal::derive(move || data.get().gcs_total as f32)
                            min=3.0 max=15.0 step=1.0
                            on_change=Callback::new(move |v| data.update(|d| d.gcs_total = v as u8)) />

                        <SofaMetric label="Renal (Creatinina)" icon="fa-kidneys" color="#F59E0B" _pts=Signal::derive(move || breakdown.get().renal)
                            value=Signal::derive(move || data.get().creatinina)
                            min=0.1 max=6.0 step=0.1
                            on_change=Callback::new(move |v| data.update(|d| d.creatinina = v)) />
                    </div>
                </div>

                <div class="xl:w-80 lg:w-full shrink-0">
                    <div class="rounded-2xl xl:rounded-3xl p-4 sm:p-6 lg:p-8 border-2 shadow-inner text-center"
                         style="background:linear-gradient(135deg,rgba(16,185,129,0.1),rgba(5,150,105,0.05)); border-color:rgba(16,185,129,0.3);">
                        <div class="text-xs sm:text-sm lg:text-lg font-black uppercase tracking-wider" style="color:var(--uci-muted);">"SOFA"</div>
                        <div class="text-5xl sm:text-6xl lg:text-8xl font-black font-mono leading-none" style="color:#10B981;">
                            {move || score.get()}
                        </div>
                        <div class="text-sm sm:text-lg lg:text-xl font-black uppercase tracking-widest mt-2" style=move || format!("color:{};", level_color())>
                            {move || level.get().label()}
                        </div>
                    </div>

                    <div class="rounded-xl lg:rounded-2xl p-4 sm:p-6 mt-3 sm:mt-4 border" style="background:var(--uci-surface); border-color:rgba(16,185,129,0.2);">
                        <div class="flex justify-between items-center mb-3">
                            <div class="text-sm font-black" style="color:var(--uci-muted);">"Mortalidad"</div>
                            <div class="text-2xl sm:text-3xl font-black" style="color:#10B981;">
                                {move || format!("{:.1}%", mortality.get())}
                            </div>
                        </div>
                        <div class="h-2 sm:h-3 rounded-full overflow-hidden" style="background:var(--uci-border);">
                            <div class="h-full transition-all duration-500" 
                                 style=move || format!("width: {}%; background:linear-gradient(90deg,#10B981,#059669);", mortality.get())></div>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    }
}

#[component]
fn SofaMetric(
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