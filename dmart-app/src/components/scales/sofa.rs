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

    view! {
        <div class="glass-card p-6 sm:p-10 border-emerald-500/20 bg-white shadow-xl animate-fade-in">
            <div class="flex flex-col lg:flex-row gap-10">
                <div class="flex-1 space-y-8">
                    <div class="flex items-center gap-4 mb-6">
                        <div class="w-12 h-12 rounded-2xl bg-emerald-500/10 flex items-center justify-center text-emerald-600 text-2xl">
                            <i class="fa-solid fa-lungs"></i>
                        </div>
                        <div>
                            <h3 class="text-2xl font-black text-uci-text tracking-tight uppercase">"SOFA"</h3>
                            <p class="text-xs font-bold text-uci-muted tracking-widest uppercase">"Sequential Organ Failure Assessment"</p>
                        </div>
                    </div>

                    <div class="grid grid-cols-1 md:grid-cols-2 gap-10">
                        <SofaMetric label="Respiratorio (PaO2/FiO2)" icon="fa-wind" pts=Signal::derive(move || breakdown.get().respiratorio)
                            value=Signal::derive(move || {
                                let d = data.get();
                                let pao2 = d.pao2.unwrap_or(80.0);
                                if d.fio2 > 0.0 { pao2 / d.fio2 } else { 400.0 }
                            })
                            min=50.0 max=500.0 step=10.0
                            on_change=Callback::new(move |v| data.update(|d| d.pao2 = Some(v * d.fio2))) />

                        <SofaMetric label="Cardiovascular (PAM)" icon="fa-heart-pulse" pts=Signal::derive(move || breakdown.get().cardiovascular)
                            value=Signal::derive(move || data.get().presion_arterial_media)
                            min=40.0 max=110.0 step=1.0
                            on_change=Callback::new(move |v| data.update(|d| d.presion_arterial_media = v)) />

                        <SofaMetric label="Hematológico (Plaquetas)" icon="fa-droplet" pts=Signal::derive(move || breakdown.get().coagulacion)
                            value=Signal::derive(move || data.get().plaquetas)
                            min=10.0 max=200.0 step=5.0
                            on_change=Callback::new(move |v| data.update(|d| d.plaquetas = v)) />

                        <SofaMetric label="Hepático (Bilirrubina)" icon="fa-vial" pts=Signal::derive(move || breakdown.get().hepatico)
                            value=Signal::derive(move || data.get().bilirrubina)
                            min=0.1 max=15.0 step=0.1
                            on_change=Callback::new(move |v| data.update(|d| d.bilirrubina = v)) />

                        <SofaMetric label="Neurológico (GCS)" icon="fa-brain" pts=Signal::derive(move || breakdown.get().neurologico)
                            value=Signal::derive(move || data.get().gcs_total as f32)
                            min=3.0 max=15.0 step=1.0
                            on_change=Callback::new(move |v| data.update(|d| d.gcs_total = v as u8)) />

                        <SofaMetric label="Renal (Creatinina)" icon="fa-kidneys" pts=Signal::derive(move || breakdown.get().renal)
                            value=Signal::derive(move || data.get().creatinina)
                            min=0.1 max=6.0 step=0.1
                            on_change=Callback::new(move |v| data.update(|d| d.creatinina = v)) />
                    </div>
                </div>

                <div class="lg:w-80 flex flex-col gap-6">
                    <div class="bg-uci-bg rounded-3xl p-8 border-2 border-emerald-500/30 shadow-inner text-center space-y-4">
                        <div class="text-xs font-black text-emerald-500 uppercase tracking-[0.2em]">"Puntaje SOFA"</div>
                        <div class="text-7xl font-black text-uci-text font-mono leading-none">
                            {move || score.get()}
                        </div>
                        <div class=move || format!("text-sm font-black uppercase tracking-widest {}", level.get().color_class())>
                            {move || level.get().label()}
                        </div>
                    </div>

                    <div class="bg-white rounded-2xl p-6 border border-uci-border shadow-sm">
                        <div class="flex justify-between items-center mb-2">
                            <span class="text-[10px] font-black text-uci-muted uppercase tracking-widest">"Mortalidad Estimada"</span>
                            <span class="text-2xl font-black text-emerald-600">
                                {move || format!("{:.1}%", mortality.get())}
                            </span>
                        </div>
                        <div class="w-full h-2 bg-uci-border rounded-full overflow-hidden">
                            <div class="h-full bg-emerald-500 transition-all duration-500" style=move || format!("width: {}%", mortality.get())></div>
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
                    <span class="text-xs font-black bg-emerald-500 text-white px-2 py-0.5 rounded-lg shadow-sm">
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
