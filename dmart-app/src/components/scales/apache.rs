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

    let severity_color = move || {
        match severity.get() {
            SeverityLevel::Bajo => "var(--uci-low)",
            SeverityLevel::Moderado => "var(--uci-moderate)",
            SeverityLevel::Severo => "var(--uci-severe)",
            SeverityLevel::Critico => "var(--uci-critical)",
        }
    };

    view! {
        <div class="glass-card p-6 sm:p-10 border-[var(--uci-border)] bg-[var(--uci-card)] shadow-xl animate-fade-in">
            <div class="flex flex-col lg:flex-row gap-10">
                <div class="flex-1 space-y-8">
                    <div class="flex items-center gap-5 mb-8">
                        <div class="w-16 h-16 rounded-2xl flex items-center justify-center text-3xl shadow-lg" 
                             style="background:linear-gradient(135deg, var(--uci-accent), var(--uci-accent2)); color:white;">
                            <i class="fa-solid fa-heart-pulse"></i>
                        </div>
                        <div>
                            <h3 class="text-4xl font-black text-[var(--uci-text)] tracking-tight uppercase">"APACHE II"</h3>
                            <p class="text-lg font-semibold" style="color:var(--uci-muted); letter-spacing:0.1em;">"Fisiología Aguda y Salud Crítica"</p>
                        </div>
                    </div>

                    <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
                        <ScaleSlider label="Temperatura (°C)" icon="fa-temperature-half" min=30.0 max=45.0 step=0.1 
                            color="#EF4444"
                            value=Signal::derive(move || data.get().temperatura)
                            on_change=Callback::new(move |v| data.update(|d| d.temperatura = v)) />
                
                        <ScaleSlider label="PAM (mmHg)" icon="fa-gauge-high" min=20.0 max=200.0 step=1.0 
                            color="#F97316"
                            value=Signal::derive(move || data.get().presion_arterial_media)
                            on_change=Callback::new(move |v| data.update(|d| d.presion_arterial_media = v)) />

                        <ScaleSlider label="Frecuencia Cardíaca" icon="fa-heartbeat" min=20.0 max=220.0 step=1.0 
                            color="#EC4899"
                            value=Signal::derive(move || data.get().frecuencia_cardiaca)
                            on_change=Callback::new(move |v| data.update(|d| d.frecuencia_cardiaca = v)) />

                        <ScaleSlider label="Frec. Respiratoria" icon="fa-wind" min=5.0 max=60.0 step=1.0 
                            color="#8B5CF6"
                            value=Signal::derive(move || data.get().frecuencia_respiratoria)
                            on_change=Callback::new(move |v| data.update(|d| d.frecuencia_respiratoria = v)) />

                        <ScaleSlider label="FiO2" icon="fa-wind" min=0.21 max=1.0 step=0.01 
                            color="#06B6D4"
                            value=Signal::derive(move || data.get().fio2)
                            on_change=Callback::new(move |v| data.update(|d| d.fio2 = v)) />

                        <ScaleSlider label="PaO2 (mmHg)" icon="fa-lungs" min=40.0 max=500.0 step=1.0 
                            color="#10B981"
                            value=Signal::derive(move || data.get().pao2.unwrap_or(80.0))
                            on_change=Callback::new(move |v| data.update(|d| d.pao2 = Some(v))) />

                        <ScaleSlider label="pH Arterial" icon="fa-vial-circle-check" min=6.8 max=7.8 step=0.01 
                            color="#14B8A6"
                            value=Signal::derive(move || data.get().ph_arterial)
                            on_change=Callback::new(move |v| data.update(|d| d.ph_arterial = v)) />

                        <ScaleSlider label="Sodio (mEq/L)" icon="fa-flask-vial" min=110.0 max=180.0 step=1.0 
                            color="#F59E0B"
                            value=Signal::derive(move || data.get().sodio_serico)
                            on_change=Callback::new(move |v| data.update(|d| d.sodio_serico = v)) />

                        <ScaleSlider label="Potasio (mEq/L)" icon="fa-flask" min=1.0 max=10.0 step=0.1 
                            color="#8B5CF6"
                            value=Signal::derive(move || data.get().potasio_serico)
                            on_change=Callback::new(move |v| data.update(|d| d.potasio_serico = v)) />

                        <ScaleSlider label="Creatinina (mg/dL)" icon="fa-kidneys" min=0.1 max=15.0 step=0.1 
                            color="#DC2626"
                            value=Signal::derive(move || data.get().creatinina)
                            on_change=Callback::new(move |v| data.update(|d| d.creatinina = v)) />

                        <ScaleSlider label="Hematocrito (%)" icon="fa-droplet" min=10.0 max=65.0 step=1.0 
                            color="#DC2626"
                            value=Signal::derive(move || data.get().hematocrito)
                            on_change=Callback::new(move |v| data.update(|d| d.hematocrito = v)) />

                        <ScaleSlider label="Leucocitos (10³/µL)" icon="fa-microscope" min=0.1 max=50.0 step=0.1 
                            color="#6366F1"
                            value=Signal::derive(move || data.get().leucocitos)
                            on_change=Callback::new(move |v| data.update(|d| d.leucocitos = v)) />
                    </div>

                    <div class="pt-6 border-t border-[var(--uci-border)]">
                        <h4 class="text-lg font-bold uppercase tracking-widest mb-5" style="color:var(--uci-muted);">"Enfermedades Crónicas"</h4>
                        <div class="grid grid-cols-2 md:grid-cols-3 gap-4">
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

                    <div class="pt-6 border-t border-[var(--uci-border)]">
                        <h4 class="text-lg font-bold uppercase tracking-widest mb-5 flex items-center gap-3" style="color:var(--uci-muted);">
                            <i class="fa-solid fa-brain text-xl" style="color:var(--uci-accent);"></i> " Glasgow (GCS) 3-15"
                        </h4>
                        <div class="grid grid-cols-3 gap-5">
                            <div class="bg-[var(--uci-surface)] p-5 rounded-2xl border-2 border-[var(--uci-border)] text-center">
                                <label class="text-base font-bold block mb-3" style="color:var(--uci-muted);">"Ojos (E)"</label>
                                <input type="range" min="1" max="4" step="1" 
                                    value={Signal::derive(move || data.get().gcs_ojos)}
                                    on:input=move |ev| {
                                        if let Ok(v) = event_target_value(&ev).parse::<u8>() {
                                            data.update(|d| { d.gcs_ojos = v; d.gcs_total = d.gcs_ojos + d.gcs_verbal + d.gcs_motor; });
                                        }
                                    }
                                    class="w-full mb-3"
                                    style="--slider-color:#10B981;"
                                />
                                <div class="text-4xl font-black" style="color:#10B981;">{move || data.get().gcs_ojos}</div>
                            </div>
                            <div class="bg-[var(--uci-surface)] p-5 rounded-2xl border-2 border-[var(--uci-border)] text-center">
                                <label class="text-base font-bold block mb-3" style="color:var(--uci-muted);">"Verbal (V)"</label>
                                <input type="range" min="1" max="5" step="1" 
                                    value={Signal::derive(move || data.get().gcs_verbal)}
                                    on:input=move |ev| {
                                        if let Ok(v) = event_target_value(&ev).parse::<u8>() {
                                            data.update(|d| { d.gcs_verbal = v; d.gcs_total = d.gcs_ojos + d.gcs_verbal + d.gcs_motor; });
                                        }
                                    }
                                    class="w-full mb-3"
                                    style="--slider-color:#F59E0B;"
                                />
                                <div class="text-4xl font-black" style="color:#F59E0B;">{move || data.get().gcs_verbal}</div>
                            </div>
                            <div class="bg-[var(--uci-surface)] p-5 rounded-2xl border-2 border-[var(--uci-border)] text-center">
                                <label class="text-base font-bold block mb-3" style="color:var(--uci-muted);">"Motor (M)"</label>
                                <input type="range" min="1" max="6" step="1" 
                                    value={Signal::derive(move || data.get().gcs_motor)}
                                    on:input=move |ev| {
                                        if let Ok(v) = event_target_value(&ev).parse::<u8>() {
                                            data.update(|d| { d.gcs_motor = v; d.gcs_total = d.gcs_ojos + d.gcs_verbal + d.gcs_motor; });
                                        }
                                    }
                                    class="w-full mb-3"
                                    style="--slider-color:#8B5CF6;"
                                />
                                <div class="text-4xl font-black" style="color:#8B5CF6;">{move || data.get().gcs_motor}</div>
                            </div>
                        </div>
                        <div class="mt-6 text-center p-6 rounded-2xl" style="background:linear-gradient(135deg, var(--uci-accent), var(--uci-accent2));">
                            <span class="text-xl font-bold text-white">GCS Total: </span>
                            <span class="text-5xl font-black text-white">{move || data.get().gcs_total}</span>
                            <span class="text-2xl text-white/70">/15</span>
                        </div>
                    </div>
                </div>

                <div class="lg:w-96 flex flex-col gap-6">
                    <div class="rounded-3xl p-8 border-2 shadow-inner text-center transition-all duration-300"
                         style=move || format!("border-color:{}; background:color-mix(in srgb, {} 10%, var(--uci-surface));", severity_color(), severity_color())>
                        <div class="text-lg font-black uppercase tracking-[0.2em]" style="color:var(--uci-muted);">"Puntaje Total"</div>
                        <div class="text-8xl font-black font-mono leading-none tracking-tighter transition-all duration-300"
                             style=move || format!("color:{};", severity_color())>
                            {move || score.get()}
                        </div>
                        <div class="text-xl font-black uppercase tracking-widest mt-3 transition-all duration-300"
                             style=move || format!("color:{};", severity_color())>
                            {move || severity.get().label()}
                        </div>
                    </div>

                    <div class="rounded-2xl p-6 border transition-all duration-300"
                         style=move || format!("border-color:color-mix(in srgb, {} 30%, transparent); background:color-mix(in srgb, {} 8%, var(--uci-surface));", severity_color(), severity_color())>
                        <div class="flex justify-between items-center mb-4">
                            <span class="text-lg font-bold" style="color:var(--uci-muted);">"Mortalidad Est."</span>
                            <div class="text-4xl font-black transition-all duration-300"
                                 style=move || format!("color:{};", severity_color())>
                                {move || format!("{:.1}%", mortality.get())}
                            </div>
                        </div>
                        <p class="text-base font-medium italic" style="color:var(--uci-muted);">
                            "Curva original Knaus et al. (1985)."
                        </p>
                    </div>

                    <div class="bg-[var(--uci-surface)] rounded-2xl p-6 border border-[var(--uci-border)] shadow-sm">
                        <h4 class="text-lg font-bold uppercase tracking-widest mb-5" style="color:var(--uci-muted);">"Desglose"</h4>
                        <div class="grid grid-cols-2 gap-4 text-lg">
                            <div class="flex justify-between"><span style="color:var(--uci-muted);">"APS:"</span><span class="font-bold">{move || breakdown.get().aps_total}</span></div>
                            <div class="flex justify-between"><span style="color:var(--uci-muted);">"Edad:"</span><span class="font-bold">{move || breakdown.get().edad_pts}</span></div>
                            <div class="flex justify-between"><span style="color:var(--uci-muted);">"Crónicas:"</span><span class="font-bold">{move || breakdown.get().cronicas_pts}</span></div>
                            <div class="flex justify-between col-span-2 border-t border-[var(--uci-border)] pt-4 mt-2">
                                <span class="font-bold">"Total:"</span><span class="font-bold text-2xl" style="color:var(--uci-accent);">{move || breakdown.get().total}</span>
                            </div>
                        </div>
                    </div>

                    <div class="p-6 rounded-2xl border flex items-center gap-5"
                         style="background:color-mix(in srgb, var(--uci-accent) 8%, transparent); border-color:color-mix(in srgb, var(--uci-accent) 20%, transparent);">
                        <div class="w-14 h-14 rounded-full flex items-center justify-center text-2xl"
                             style="background:linear-gradient(135deg, var(--uci-accent), var(--uci-accent2)); color:white;">
                            <i class="fa-solid fa-calculator"></i>
                        </div>
                        <div>
                            <div class="text-base font-black uppercase tracking-widest" style="color:var(--uci-muted);">"Cálculo"</div>
                            <div class="text-lg font-semibold" style="color:var(--uci-text);">"Actualización en tiempo real"</div>
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
    color: &'static str,
    value: Signal<f32>,
    on_change: Callback<f32>,
) -> impl IntoView {
    let val_pct = move || {
        let v = value.get();
        ((v - min) / (max - min) * 100.0).clamp(0.0, 100.0)
    };

    view! {
        <div class="space-y-3">
            <div class="flex justify-between items-center">
                <label class="text-lg font-bold flex items-center gap-3" style="color:var(--uci-muted);">
                    <i class=format!("fa-solid {}", icon) style=move || format!("color:{};", color)></i>
                    {label}
                </label>
                <span class="text-2xl font-bold font-mono px-4 py-2 rounded-xl"
                      style=move || format!("background:color-mix(in srgb, {} 15%, transparent); color:{};", color, color)>
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
                style=move || format!("--val: {}%; --slider-color: {};", val_pct(), color)
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
        <label class="flex items-center gap-4 cursor-pointer group p-3 rounded-xl hover:bg-[var(--uci-surface)] transition-all">
            <input 
                type="checkbox" 
                class="w-6 h-6 rounded border-2"
                style="border-color:var(--uci-muted); color:var(--uci-accent);"
                prop:checked=checked.get()
                on:change=move |ev| {
                    let target = event_target::<web_sys::HtmlInputElement>(&ev);
                    on_change.run(target.checked());
                }
            />
            <span class="text-lg font-semibold" style="color:var(--uci-text);">{label}</span>
        </label>
    }
}