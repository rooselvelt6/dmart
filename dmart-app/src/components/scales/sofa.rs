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
        <div class="glass-card p-6 sm:p-10 border-emerald-500/20 shadow-xl animate-fade-in" style="background:var(--uci-card);">
            <div class="flex flex-col lg:flex-row gap-10">
                <div class="flex-1 space-y-8">
                    <div class="flex items-center gap-5 mb-8">
                        <div class="w-16 h-16 rounded-2xl flex items-center justify-center text-3xl shadow-lg" 
                             style="background:linear-gradient(135deg,#10B981,#059669); color:white;">
                            <i class="fa-solid fa-lungs"></i>
                        </div>
                        <div>
                            <h3 class="text-4xl font-black text-[var(--uci-text)] tracking-tight uppercase">"SOFA"</h3>
                            <p class="text-lg font-semibold" style="color:var(--uci-muted); letter-spacing:0.1em;">"Sequential Organ Failure Assessment"</p>
                        </div>
                    </div>

                    <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
                        <SofaMetric label="Respiratorio (PaO2/FiO2)" icon="fa-wind" color="#06B6D4" pts=Signal::derive(move || breakdown.get().respiratorio)
                            value=Signal::derive(move || {
                                let d = data.get();
                                let pao2 = d.pao2.unwrap_or(80.0);
                                if d.fio2 > 0.0 { pao2 / d.fio2 } else { 400.0 }
                            })
                            min=50.0 max=500.0 step=10.0
                            on_change=Callback::new(move |v| data.update(|d| d.pao2 = Some(v * d.fio2))) />

                        <SofaMetric label="Cardiovascular (PAM)" icon="fa-heart-pulse" color="#EC4899" pts=Signal::derive(move || breakdown.get().cardiovascular)
                            value=Signal::derive(move || data.get().presion_arterial_media)
                            min=40.0 max=110.0 step=1.0
                            on_change=Callback::new(move |v| data.update(|d| d.presion_arterial_media = v)) />

                        <SofaMetric label="Hematológico (Plaquetas)" icon="fa-droplet" color="#DC2626" pts=Signal::derive(move || breakdown.get().coagulacion)
                            value=Signal::derive(move || data.get().plaquetas)
                            min=10.0 max=200.0 step=5.0
                            on_change=Callback::new(move |v| data.update(|d| d.plaquetas = v)) />

                        <SofaMetric label="Hepático (Bilirrubina)" icon="fa-vial" color="#84CC16" pts=Signal::derive(move || breakdown.get().hepatico)
                            value=Signal::derive(move || data.get().bilirrubina)
                            min=0.1 max=15.0 step=0.1
                            on_change=Callback::new(move |v| data.update(|d| d.bilirrubina = v)) />

                        <SofaMetric label="Neurológico (GCS)" icon="fa-brain" color="#A855F7" pts=Signal::derive(move || breakdown.get().neurologico)
                            value=Signal::derive(move || data.get().gcs_total as f32)
                            min=3.0 max=15.0 step=1.0
                            on_change=Callback::new(move |v| data.update(|d| d.gcs_total = v as u8)) />

                        <SofaMetric label="Renal (Creatinina)" icon="fa-kidneys" color="#F59E0B" pts=Signal::derive(move || breakdown.get().renal)
                            value=Signal::derive(move || data.get().creatinina)
                            min=0.1 max=6.0 step=0.1
                            on_change=Callback::new(move |v| data.update(|d| d.creatinina = v)) />
                    </div>
                </div>

                <div class="lg:w-96 flex flex-col gap-6">
                    <div class="rounded-3xl p-8 border-2 shadow-inner text-center"
                         style="background:linear-gradient(135deg,rgba(16,185,129,0.1),rgba(5,150,105,0.05)); border-color:rgba(16,185,129,0.3);">
                        <div class="text-lg font-black uppercase tracking-[0.2em]" style="color:var(--uci-muted);">"Puntaje SOFA"</div>
                        <div class="text-8xl font-black font-mono leading-none" style="color:#10B981;">
                            {move || score.get()}
                        </div>
                        <div class="text-xl font-black uppercase tracking-widest mt-3" style=move || format!("color:{};", level_color())>
                            {move || level.get().label()}
                        </div>
                    </div>

                    <div class="rounded-2xl p-6 border" style="background:var(--uci-surface); border-color:rgba(16,185,129,0.2);">
                        <div class="flex justify-between items-center mb-4">
                            <div class="text-base font-black" style="color:var(--uci-muted);">"Mortalidad Estimada"</div>
                            <div class="text-3xl font-black" style="color:#10B981;">
                                {move || format!("{:.1}%", mortality.get())}
                            </div>
                        </div>
                        <div class="h-3 rounded-full overflow-hidden" style="background:var(--uci-border);">
                            <div class="h-full transition-all duration-500" 
                                 style=move || format!("width: {}%; background:linear-gradient(90deg,#10B981,#059669);", mortality.get())></div>
                        </div>
                    </div>

                    <div class="rounded-2xl p-5 border flex items-center gap-4"
                         style="background:color-mix(in srgb, #10B981 8%, transparent); border-color:color-mix(in srgb, #10B981 20%, transparent);">
                        <div class="w-12 h-12 rounded-full flex items-center justify-center text-2xl"
                             style="background:linear-gradient(135deg,#10B981,#059669); color:white;">
                            <i class="fa-solid fa-calculator"></i>
                        </div>
                        <div>
                            <div class="text-base font-black uppercase tracking-widest" style="color:var(--uci-muted);">"Cálculo"</div>
                            <div class="text-lg font-semibold" style="color:var(--uci-text);">"Automático en tiempo real"</div>
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
        <div class="space-y-3 p-4 rounded-xl border" style="background:var(--uci-surface); border-color:color-mix(in srgb, {} 20%, transparent);">
            <div class="flex justify-between items-center">
                <label class="text-lg font-bold flex items-center gap-3" style="color:var(--uci-muted);">
                    <i class=format!("fa-solid {}", icon) style=move || format!("color:{};", color)></i>
                    {label}
                </label>
                <div class="flex items-center gap-2">
                    <span class="text-2xl font-bold font-mono"
                          style=move || format!("color:{};", color)>
                        {move || if step < 1.0 { format!("{:.1}", value.get()) } else { format!("{:.0}", value.get()) }}
                    </span>
                    <span class="text-xs font-bold px-2 py-1 rounded" style="background:color-mix(in srgb, {} 15%, transparent); color:{};">"pts:" {pts}</span>
                </div>
            </div>
            <input 
                type="range" class="w-full"
                min=min max=max step=step
                prop:value=move || value.get()
                on:input=move |ev| {
                    if let Ok(v) = event_target_value(&ev).parse::<f32>() {
                        on_change.run(v);
                    }
                }
                style=move || format!("--val: {}%; --slider-color: {};", val_pct(), color)
                data-sev="normal"
            />
        </div>
    }
}