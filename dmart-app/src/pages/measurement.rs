use leptos::prelude::*;
use leptos::either::Either;
use leptos::task::spawn_local;
use leptos_router::hooks::*;
use dmart_shared::models::*;
use dmart_shared::scales::{calculate_apache_ii_score, calculate_gcs_score, mortality_risk};
use crate::api;
use crate::components::{slider::ScaleSlider, severity_badge::SeverityBadge, toggle::Toggle};

#[component]
pub fn MeasurementPage() -> impl IntoView {
    let params = use_params_map();
    let id = move || params.get().get("id").map(|s| s.to_string()).unwrap_or_default();
    
    let patient_res = LocalResource::new(move || {
        let pid = id();
        async move { api::get_patient(&pid).await }
    });

    // Reactive physiology data
    let (apache, set_apache) = signal(ApacheIIData::default());
    let (gcs, set_gcs) = signal(GcsData::default());
    let (notas, set_notas) = signal(String::new());
    
    // Derived scores
    let current_apache_score = Memo::new(move |_| calculate_apache_ii_score(&apache.get()));
    let current_gcs_score = Memo::new(move |_| gcs.get().total());
    let current_severity = Memo::new(move |_| SeverityLevel::from_score(current_apache_score.get()));
    let current_mortality = Memo::new(move |_| mortality_risk(current_apache_score.get()));

    let saving = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let navigate = use_navigate();

    let on_submit = move |_: web_sys::MouseEvent| {
        saving.set(true);
        let pid = id();
        let a = apache.get();
        let g = gcs.get();
        let n = notas.get();
        let nav = navigate.clone();
        
        spawn_local(async move {
            match api::create_measurement(&pid, a, g, n).await {
                Ok(_) => {
                    saving.set(false);
                    nav(&format!("/patients/{}", pid), Default::default());
                }
                Err(e) => {
                    saving.set(false);
                    error.set(Some(e));
                }
            }
        });
    };

    let on_submit = StoredValue::new(on_submit);

    view! {
        <div class="page-enter max-w-6xl mx-auto">
            <Suspense fallback=move || view! { <div class="p-10 text-uci-muted">"Cargando calculadora..."</div> }>
                {move || patient_res.get().map(|res_wrapper| match &*res_wrapper {
                    Ok(ref p) => Either::Left(view! {
                        <div class="flex flex-col lg:flex-row gap-8">
                            // Left Column: Calculator
                            <div class="flex-1">
                                <header class="mb-8">
                                    <a href=format!("/patients/{}", p.patient_id) class="text-uci-muted text-xs font-bold uppercase mb-2 block hover:text-uci-accent transition-colors">
                                        "← Volver al Paciente"
                                    </a>
                                    <h1 class="text-3xl font-extrabold text-uci-text">"Nueva Medición"</h1>
                                    <p class="text-sm text-uci-muted">
                                        "Paciente: " <span class="text-uci-accent font-bold">{p.nombre_completo()}</span>
                                    </p>
                                </header>

                                {move || error.get().map(|e| view! {
                                    <div class="bg-uci-critical/10 border border-uci-critical/30 p-4 rounded-xl mb-6 text-uci-critical text-sm">
                                        "⚠ " {e}
                                    </div>
                                })}

                                <div class="space-y-10">
                                    // 1. Signos Vitales
                                    <section>
                                        <h2 class="text-uci-text font-bold text-lg mb-6 flex items-center gap-3">
                                            <span class="w-8 h-8 rounded-lg bg-uci-accent/20 flex items-center justify-center text-uci-accent">"💓"</span>
                                            "Signos Vitales"
                                        </h2>
                                        <div class="grid grid-cols-1 md:grid-cols-2 gap-x-12">
                                            <ScaleSlider 
                                                label="Temperatura Central" unit="°C" min=30.0 max=44.0 step=0.1
                                                value=Signal::derive(move || apache.get().temperatura)
                                                on_change=move |v| set_apache.update(|a| a.temperatura = v) />
                                            
                                            <ScaleSlider 
                                                label="Presión Arterial Media" unit="mmHg" min=0.0 max=200.0 step=1.0
                                                value=Signal::derive(move || apache.get().presion_arterial_media)
                                                on_change=move |v| set_apache.update(|a| a.presion_arterial_media = v) />

                                            <ScaleSlider 
                                                label="Frecuencia Cardíaca" unit="lpm" min=0.0 max=220.0 step=1.0
                                                value=Signal::derive(move || apache.get().frecuencia_cardiaca)
                                                on_change=move |v| set_apache.update(|a| a.frecuencia_cardiaca = v) />

                                            <ScaleSlider 
                                                label="Frecuencia Respiratoria" unit="rpm" min=0.0 max=60.0 step=1.0
                                                value=Signal::derive(move || apache.get().frecuencia_respiratoria)
                                                on_change=move |v| set_apache.update(|a| a.frecuencia_respiratoria = v) />
                                        </div>
                                    </section>

                                    // 2. Oxigenación y Gasometría
                                    <section>
                                        <h2 class="text-uci-text font-bold text-lg mb-6 flex items-center gap-3">
                                            <span class="w-8 h-8 rounded-lg bg-uci-accent/20 flex items-center justify-center text-uci-accent">"🩸"</span>
                                            "Oxigenación y Laboratorios"
                                        </h2>
                                        <div class="grid grid-cols-1 md:grid-cols-2 gap-x-12">
                                            <ScaleSlider 
                                                label="FiO2 (Fracción Insp. O2)" unit="" min=0.21 max=1.0 step=0.01
                                                value=Signal::derive(move || apache.get().fio2)
                                                on_change=move |v| set_apache.update(|a| a.fio2 = v) />

                                            <ScaleSlider 
                                                label="pH Arterial" unit="" min=7.0 max=7.8 step=0.01
                                                value=Signal::derive(move || apache.get().ph_arterial)
                                                on_change=move |v| set_apache.update(|a| a.ph_arterial = v) />

                                            <ScaleSlider 
                                                label="Sodio Sérico" unit="mEq/L" min=100.0 max=180.0 step=1.0
                                                value=Signal::derive(move || apache.get().sodio_serico)
                                                on_change=move |v| set_apache.update(|a| a.sodio_serico = v) />

                                            <ScaleSlider 
                                                label="Potasio Sérico" unit="mEq/L" min=1.0 max=8.5 step=0.1
                                                value=Signal::derive(move || apache.get().potasio_serico)
                                                on_change=move |v| set_apache.update(|a| a.potasio_serico = v) />
                                            
                                            <ScaleSlider 
                                                label="Creatinina" unit="mg/dL" min=0.1 max=12.0 step=0.1
                                                value=Signal::derive(move || apache.get().creatinina)
                                                on_change=move |v| set_apache.update(|a| a.creatinina = v) />

                                            <div class="mb-6 flex items-center justify-between p-4 bg-uci-surface rounded-xl border border-uci-border">
                                                <div>
                                                    <div class="text-[12px] font-bold text-uci-muted uppercase mb-1">"Falla Renal Aguda"</div>
                                                    <div class="text-[10px] text-uci-muted">"Si es true, duplica puntos de Creatinina"</div>
                                                </div>
                                                <Toggle 
                                                    value=Signal::derive(move || apache.get().falla_renal_aguda)
                                                    on_change=move |v| set_apache.update(|a| a.falla_renal_aguda = v) />
                                            </div>

                                            <ScaleSlider 
                                                label="Hematocrito" unit="%" min=10.0 max=60.0 step=1.0
                                                value=Signal::derive(move || apache.get().hematocrito)
                                                on_change=move |v| set_apache.update(|a| a.hematocrito = v) />

                                            <ScaleSlider 
                                                label="Leucocitos (WBC)" unit="x10³" min=0.5 max=60.0 step=0.1
                                                value=Signal::derive(move || apache.get().leucocitos)
                                                on_change=move |v| set_apache.update(|a| a.leucocitos = v) />
                                        </div>
                                    </section>

                                    // 3. Glasgow (GCS)
                                    <section>
                                        <h2 class="text-uci-text font-bold text-lg mb-6 flex items-center gap-3">
                                            <span class="w-8 h-8 rounded-lg bg-uci-accent/20 flex items-center justify-center text-uci-accent">"🧠"</span>
                                            "Glasgow Coma Scale"
                                        </h2>
                                        <div class="bg-uci-card/30 p-6 rounded-2xl border border-uci-border">
                                            <div class="grid grid-cols-1 gap-6">
                                                <div class="flex flex-col md:flex-row gap-4">
                                                    <div class="flex-1">
                                                        <label class="form-label">"Apertura Ocular"</label>
                                                        <select class="form-select" on:change=move |ev| {
                                                            let v = event_target_value(&ev).parse::<u8>().unwrap_or(4);
                                                            set_gcs.update(|g| g.apertura_ocular = v);
                                                            set_apache.update(|a| a.gcs_total = current_gcs_score.get());
                                                        }>
                                                            <option value="4">"Espontánea (4)"</option>
                                                            <option value="3">"A la voz (3)"</option>
                                                            <option value="2">"Al dolor (2)"</option>
                                                            <option value="1">"Ninguna (1)"</option>
                                                        </select>
                                                    </div>
                                                    <div class="flex-1">
                                                        <label class="form-label">"Respuesta Verbal"</label>
                                                        <select class="form-select" on:change=move |ev| {
                                                            let v = event_target_value(&ev).parse::<u8>().unwrap_or(5);
                                                            set_gcs.update(|g| g.respuesta_verbal = v);
                                                            set_apache.update(|a| a.gcs_total = current_gcs_score.get());
                                                        }>
                                                            <option value="5">"Orientado (5)"</option>
                                                            <option value="4">"Confuso (4)"</option>
                                                            <option value="3">"Palabras (3)"</option>
                                                            <option value="2">"Sonidos (2)"</option>
                                                            <option value="1">"Ninguna (1)"</option>
                                                        </select>
                                                    </div>
                                                    <div class="flex-1">
                                                        <label class="form-label">"Respuesta Motora"</label>
                                                        <select class="form-select" on:change=move |ev| {
                                                            let v = event_target_value(&ev).parse::<u8>().unwrap_or(6);
                                                            set_gcs.update(|g| g.respuesta_motora = v);
                                                            set_apache.update(|a| a.gcs_total = current_gcs_score.get());
                                                        }>
                                                            <option value="6">"Obedece (6)"</option>
                                                            <option value="5">"Localiza (5)"</option>
                                                            <option value="4">"Retira (4)"</option>
                                                            <option value="3">"Flexión (3)"</option>
                                                            <option value="2">"Extensión (2)"</option>
                                                            <option value="1">"Ninguna (1)"</option>
                                                        </select>
                                                    </div>
                                                </div>
                                                <div class="pt-4 border-t border-uci-border flex justify-between items-center">
                                                    <div class="text-sm font-medium text-uci-muted">"Interpretación: " <span class="text-uci-text">{move || GcsData::interpret(&gcs.get())}</span></div>
                                                    <div class="text-2xl font-black text-uci-accent font-mono">{move || current_gcs_score.get()} "/15"</div>
                                                </div>
                                            </div>
                                        </div>
                                    </section>

                                    // 4. Notas
                                    <section class="pb-20">
                                        <FormField label="Notas Clínicas / Observaciones">
                                            <textarea 
                                                class="form-input" placeholder="Observaciones sobre el estado del paciente en las últimas 24h..."
                                                on:input=move |ev| set_notas.set(event_target_value(&ev))
                                            ></textarea>
                                        </FormField>
                                    </section>
                                </div>
                            </div>

                            // Right Column: Summary Stick Card
                            <div class="lg:w-96">
                                <div class="lg:sticky lg:top-8">
                                    <div class="glass-card p-6 overflow-hidden">
                                        // Live Score Header
                                        <div class="text-center mb-8">
                                            <div class="text-[10px] font-bold text-uci-muted uppercase tracking-[0.2em] mb-4">"Live Apache II Score"</div>
                                            <div 
                                                class="score-number mb-2 transition-colors duration-500"
                                                style=move || format!("color: {}", crate::pages::dashboard::severity_color_hex(&current_severity.get()))
                                            >
                                                {move || current_apache_score.get()}
                                            </div>
                                            <div class="flex justify-center mb-6">
                                                <SeverityBadge level=current_severity.get() />
                                            </div>
                                            
                                            <div class="bg-uci-bg/50 rounded-2xl p-4 border border-uci-border/50">
                                                <div class="text-[10px] font-bold text-uci-muted uppercase mb-1">"Riesgo de Mortalidad"</div>
                                                <div class="text-2xl font-bold text-uci-severe">{move || format!("{:.1}%", current_mortality.get())}</div>
                                            </div>
                                        </div>

                                        // Summary Table
                                        <div class="space-y-3 mb-8">
                                            <h4 class="text-[11px] font-bold text-uci-accent uppercase mb-3">"Desglose de Puntos"</h4>
                                            {move || {
                                                let bk = dmart_shared::scales::apache_ii_breakdown(&apache.get());
                                                view! {
                                                    <div class="space-y-1">
                                                        <ScoreRow label="Fisiología (APS)" score=bk.aps_total />
                                                        <ScoreRow label="GCS Impacto" score=bk.gcs_pts />
                                                        <ScoreRow label="Edad Contribución" score=bk.edad_pts />
                                                        <ScoreRow label="Enf. Crónicas" score=bk.cronicas_pts />
                                                    </div>
                                                }
                                            }}
                                        </div>

                                        // Submit Button
                                        <button 
                                            class="btn-primary w-full py-4 text-base"
                                            on:click=move |ev| on_submit.with_value(|f| f(ev))
                                            disabled=saving
                                        >
                                            {move || if saving.get() { "Guardando..." } else { "Guardar Medición Diaria" }}
                                        </button>
                                        
                                        <p class="mt-4 text-[10px] text-uci-muted text-center leading-relaxed">
                                            "Al aplicar, se actualizará el estado de gravedad del paciente y se bloqueará por las próximas 24 horas."
                                        </p>
                                    </div>
                                </div>
                            </div>
                        </div>
                    }),
                    _ => Either::Right(view! { <div class="text-center p-20 text-uci-critical">"Error de conexión"</div> }),
                })}
            </Suspense>
        </div>
    }
}

#[component]
fn ScoreRow(label: &'static str, score: u32) -> impl IntoView {
    view! {
        <div class="flex justify-between text-xs py-1 border-b border-uci-border/20 last:border-0">
            <span class="text-uci-muted">{label}</span>
            <span class="text-uci-text font-bold">"+" {score}</span>
        </div>
    }
}

#[component]
fn FormField(label: &'static str, children: Children) -> impl IntoView {
    view! {
        <div class="mb-4">
            <label class="form-label">{label}</label>
            {children()}
        </div>
    }
}
