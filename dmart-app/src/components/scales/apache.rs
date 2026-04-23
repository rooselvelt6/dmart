use leptos::prelude::*;
use dmart_shared::models::*;
use dmart_shared::scales::*;

#[component]
pub fn ApacheIIScale(
    data: RwSignal<ApacheIIData>,
) -> impl IntoView {
    let score = Memo::new(move |_| calculate_apache_ii_score(&data.get()));
    let severity = Memo::new(move |_| SeverityLevel::from_score(score.get()));
    let mortality = Memo::new(move |_| mortality_risk(score.get()));

    let breakdown = Memo::new(move |_| apache_ii_breakdown(&data.get()));

    view! {
        <div class="glass-card p-6 sm:p-10 border-uci-accent/20 bg-white shadow-xl animate-fade-in">
            <div class="flex flex-col lg:flex-row gap-10">
                <div class="flex-1 space-y-8">
                    <div class="flex items-center gap-4 mb-6">
                        <div class="w-12 h-12 rounded-2xl bg-uci-accent/10 flex items-center justify-center text-uci-accent text-2xl">
                            <i class="fa-solid fa-heart-pulse"></i>
                        </div>
                        <div>
                            <h3 class="text-2xl font-black text-uci-text tracking-tight uppercase">"APACHE II"</h3>
                            <p class="text-xs font-bold text-uci-muted tracking-widest uppercase">"Fisiología Aguda y Salud Crónica"</p>
                        </div>
                    </div>

                    <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
                        <ScaleSlider label="Temperatura (°C)" icon="fa-thermometer-half" min=30.0 max=45.0 step=0.1 
                            value=Signal::derive(move || data.get().temperatura)
                            on_change=Callback::new(move |v| data.update(|d| d.temperatura = v)) />
                    
                        <ScaleSlider label="PAM (mmHg)" icon="fa-gauge-high" min=20.0 max=200.0 step=1.0 
                            value=Signal::derive(move || data.get().presion_arterial_media)
                            on_change=Callback::new(move |v| data.update(|d| d.presion_arterial_media = v)) />

                        <ScaleSlider label="Frecuencia Cardíaca" icon="fa-heartbeat" min=20.0 max=220.0 step=1.0 
                            value=Signal::derive(move || data.get().frecuencia_cardiaca)
                            on_change=Callback::new(move |v| data.update(|d| d.frecuencia_cardiaca = v)) />

                        <ScaleSlider label="Frec. Respiratoria" icon="fa-wind" min=5.0 max=60.0 step=1.0 
                            value=Signal::derive(move || data.get().frecuencia_respiratoria)
                            on_change=Callback::new(move |v| data.update(|d| d.frecuencia_respiratoria = v)) />

                        <ScaleSlider label="FiO2" icon="fa-wind" min=0.21 max=1.0 step=0.01 
                            value=Signal::derive(move || data.get().fio2)
                            on_change=Callback::new(move |v| data.update(|d| d.fio2 = v)) />

                        <ScaleSlider label="PaO2 (mmHg)" icon="fa-lungs" min=40.0 max=500.0 step=1.0 
                            value=Signal::derive(move || data.get().pao2.unwrap_or(80.0))
                            on_change=Callback::new(move |v| data.update(|d| d.pao2 = Some(v))) />

                        <ScaleSlider label="pH Arterial" icon="fa-vial-circle-check" min=6.8 max=7.8 step=0.01 
                            value=Signal::derive(move || data.get().ph_arterial)
                            on_change=Callback::new(move |v| data.update(|d| d.ph_arterial = v)) />

                        <ScaleSlider label="Sodio (mEq/L)" icon="fa-flask-vial" min=110.0 max=180.0 step=1.0 
                            value=Signal::derive(move || data.get().sodio_serico)
                            on_change=Callback::new(move |v| data.update(|d| d.sodio_serico = v)) />

                        <ScaleSlider label="Potasio (mEq/L)" icon="fa-flask" min=1.0 max=10.0 step=0.1 
                            value=Signal::derive(move || data.get().potasio_serico)
                            on_change=Callback::new(move |v| data.update(|d| d.potasio_serico = v)) />

                        <ScaleSlider label="Creatinina (mg/dL)" icon="fa-kidneys" min=0.1 max=15.0 step=0.1 
                            value=Signal::derive(move || data.get().creatinina)
                            on_change=Callback::new(move |v| data.update(|d| d.creatinina = v)) />

                        <ScaleSlider label="Hematocrito (%)" icon="fa-droplet" min=10.0 max=65.0 step=1.0 
                            value=Signal::derive(move || data.get().hematocrito)
                            on_change=Callback::new(move |v| data.update(|d| d.hematocrito = v)) />

                        <ScaleSlider label="Leucocitos (10³/µL)" icon="fa-microscope" min=0.1 max=50.0 step=0.1 
                            value=Signal::derive(move || data.get().leucocitos)
                            on_change=Callback::new(move |v| data.update(|d| d.leucocitos = v)) />
                    </div>

                    <div class="pt-4 border-t border-uci-border">
                        <h4 class="text-xs font-bold text-uci-muted uppercase tracking-widest mb-4">"Enfermedades Crónicas"</h4>
                        <div class="grid grid-cols-2 md:grid-cols-3 gap-3">
                            <CheckBox label="Insuficiencia Hepática" 
                                checked=Signal::derive(move || data.get().insuficiencia_hepatica)
                                on_change=Callback::new(move |v| data.update(|d| d.insuficiencia_hepatica = v)) />
                            <CheckBox label="Cardiovascular Severa" 
                                checked=Signal::derive(move || data.get().cardiovascular_severa)
                                on_change=Callback::new(move |v| data.update(|d| d.cardiovascular_severa = v)) />
                            <CheckBox label="Insuficiencia Respiratoria" 
                                checked=Signal::derive(move || data.get().insuficiencia_respiratoria)
                                on_change=Callback::new(move |v| data.update(|d| d.insuficiencia_respiratoria = v)) />
                            <CheckBox label="Insuficiencia Renal" 
                                checked=Signal::derive(move || data.get().insuficiencia_renal)
                                on_change=Callback::new(move |v| data.update(|d| d.insuficiencia_renal = v)) />
                            <CheckBox label="Inmunocomprometido" 
                                checked=Signal::derive(move || data.get().inmunocomprometido)
                                on_change=Callback::new(move |v| data.update(|d| d.inmunocomprometido = v)) />
                            <CheckBox label="Cirugía No Operado" 
                                checked=Signal::derive(move || data.get().cirugia_no_operado)
                                on_change=Callback::new(move |v| data.update(|d| d.cirugia_no_operado = v)) />
                        </div>
                    </div>

                    <div class="pt-4 border-t border-uci-border">
                        <h4 class="text-xs font-bold text-uci-muted uppercase tracking-widest mb-4">"Falla Renal Aguda"</h4>
                        <div class="flex items-center gap-3">
                            <CheckBox label="Falla Renal Aguda (dobla puntuación creatinina)" 
                                checked=Signal::derive(move || data.get().falla_renal_aguda)
                                on_change=Callback::new(move |v| data.update(|d| d.falla_renal_aguda = v)) />
                        </div>
                    </div>
                </div>

                <div class="lg:w-80 flex flex-col gap-5">
                    <div class=move || format!("bg-uci-bg rounded-3xl p-6 border-2 shadow-inner text-center transition-all duration-300 {}", 
                        match severity.get() {
                            SeverityLevel::Low => "border-green-400 bg-green-50",
                            SeverityLevel::Moderate => "border-yellow-400 bg-yellow-50",
                            SeverityLevel::Severe => "border-orange-500 bg-orange-50",
                            SeverityLevel::Critical => "border-red-500 bg-red-50",
                        }
                    )>
                        <div class="text-xs font-black text-uci-accent uppercase tracking-[0.2em]">"Puntaje Total"</div>
                        <div class=move || format!("text-6xl font-black font-mono leading-none tracking-tighter transition-all duration-300 {}",
                            match severity.get() {
                                SeverityLevel::Low => "text-green-600",
                                SeverityLevel::Moderate => "text-yellow-600",
                                SeverityLevel::Severe => "text-orange-600",
                                SeverityLevel::Critical => "text-red-600",
                            }
                        )>
                            {move || score.get()}
                        </div>
                        <div class=move || format!("text-sm font-black uppercase tracking-widest transition-all duration-300 {}",
                            match severity.get() {
                                SeverityLevel::Low => "text-green-600",
                                SeverityLevel::Moderate => "text-yellow-600",
                                SeverityLevel::Severe => "text-orange-600",
                                SeverityLevel::Critical => "text-red-600",
                            }
                        )>
                            {move || severity.get().label()}
                        </div>
                    </div>

                    <div class=move || format!("rounded-2xl p-5 border transition-all duration-300 {}",
                        match severity.get() {
                            SeverityLevel::Low => "border-green-400/30 bg-green-50/50",
                            SeverityLevel::Moderate => "border-yellow-400/30 bg-yellow-50/50",
                            SeverityLevel::Severe => "border-orange-500/30 bg-orange-50/50",
                            SeverityLevel::Critical => "border-red-500/30 bg-red-50/50",
                        }
                    )>
                        <div class="flex justify-between items-center mb-3">
                            <span class="text-xs font-bold text-uci-muted uppercase tracking-wider">"Mortalidad Est."</span>
                            <div class=move || format!("text-2xl font-black transition-all duration-300 {}",
                                match severity.get() {
                                    SeverityLevel::Low => "text-green-600",
                                    SeverityLevel::Moderate => "text-yellow-600",
                                    SeverityLevel::Severe => "text-orange-600",
                                    SeverityLevel::Critical => "text-red-600",
                                }
                            )>
                                {move || format!("{:.1}%", mortality.get())}
                            </div>
                        </div>
                        <p class="text-[10px] text-uci-muted font-medium italic leading-relaxed">
                            "Curva original Knaus et al. (1985). Predice mortalidad hospitalaria en UCI."
                        </p>
                    </div>

                    <div class="bg-white rounded-2xl p-5 border border-uci-border shadow-sm">
                        <h4 class="text-xs font-bold text-uci-muted uppercase tracking-widest mb-3">"Desglose"</h4>
                        <div class="grid grid-cols-2 gap-2 text-xs">
                            <div class="flex justify-between"><span class="text-uci-muted">APS:</span><span class="font-bold">{move || breakdown.get().aps_total}</span></div>
                            <div class="flex justify-between"><span class="text-uci-muted">Edad:</span><span class="font-bold">{move || breakdown.get().edad_pts}</span></div>
                            <div class="flex justify-between"><span class="text-uci-muted">Crónicas:</span><span class="font-bold">{move || breakdown.get().cronicas_pts}</span></div>
                            <div class="flex justify-between col-span-2 border-t border-uci-border pt-2 mt-1">
                                <span class="font-bold">Total:</span><span class="font-bold text-uci-accent">{move || breakdown.get().total}</span>
                            </div>
                        </div>
                    </div>

                    <div class="p-5 bg-uci-accent/5 rounded-2xl border border-uci-accent/10 flex items-center gap-4">
                        <div class="w-10 h-10 rounded-full bg-uci-accent/10 flex items-center justify-center text-uci-accent">
                            <i class="fa-solid fa-calculator"></i>
                        </div>
                        <div>
                            <div class="text-[10px] font-black text-uci-muted uppercase tracking-widest">"Cálculo"</div>
                            <div class="text-xs font-bold text-uci-text">"Actualización en tiempo real"</div>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    }
}

#[component]
fn ScaleSlider(
    label: &'static str,
    icon: &'static str,
    min: f32,
    max: f32,
    step: f32,
    value: Signal<f32>,
    on_change: Callback<f32>,
) -> impl IntoView {
    let val_pct = move || {
        let v = value.get();
        ((v - min) / (max - min) * 100.0).clamp(0.0, 100.0)
    };

    view! {
        <div class="space-y-2 group">
            <div class="flex justify-between items-center">
                <label class="text-[10px] font-bold text-uci-muted uppercase tracking-widest flex items-center gap-1">
                    <i class=icon></i>
                    {label}
                </label>
                <span class="text-sm font-bold text-uci-accent font-mono bg-uci-accent/5 px-2 py-0.5 rounded">
                    {move || if step < 1.0 { format!("{:.2}", value.get()) } else { format!("{:.0}", value.get()) }}
                </span>
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
                style=move || format!("--val: {}%", val_pct())
                data-sev="normal"
            />
        </div>
    }
}

#[component]
fn CheckBox(
    label: &'static str,
    checked: Signal<bool>,
    on_change: Callback<bool>,
) -> impl IntoView {
    view! {
        <label class="flex items-center gap-2 cursor-pointer group">
            <input 
                type="checkbox" 
                class="w-4 h-4 rounded border-uci-border text-uci-accent focus:ring-uci-accent"
                prop:checked=checked.get()
                on:change=move |ev| {
                    let target = event_target::<web_sys::HtmlInputElement>(&ev);
                    on_change.run(target.checked());
                }
            />
            <span class="text-xs font-medium text-uci-text group-hover:text-uci-accent transition-colors">{label}</span>
        </label>
    }
}