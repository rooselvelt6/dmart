use leptos::prelude::*;
use leptos::either::Either;
use leptos::task::spawn_local;
use leptos_router::hooks::*;
use dmart_shared::models::*;
use dmart_shared::scales::{calculate_apache_ii_score, calculate_gcs_score, mortality_risk};
use crate::api;
use crate::components::{slider::ScaleSlider, toggle::Toggle};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum EscalaMedir {
    #[default]
    ApacheII,
    GCS,
    SAPS3,
    News2,
    Sofa,
}

impl EscalaMedir {
    fn from_param(s: &str) -> Self {
        match s {
            "gcs" => EscalaMedir::GCS,
            "saps3" => EscalaMedir::SAPS3,
            "news2" => EscalaMedir::News2,
            "sofa" => EscalaMedir::Sofa,
            _ => EscalaMedir::ApacheII,
        }
    }
    
    fn label(&self) -> &'static str {
        match self {
            EscalaMedir::ApacheII => "APACHE II",
            EscalaMedir::GCS => "GCS",
            EscalaMedir::SAPS3 => "SAPS III",
            EscalaMedir::News2 => "NEWS 2",
            EscalaMedir::Sofa => "SOFA",
        }
    }
    
    fn icon(&self) -> &'static str {
        match self {
            EscalaMedir::ApacheII => "🏥",
            EscalaMedir::GCS => "🧠",
            EscalaMedir::SAPS3 => "📊",
            EscalaMedir::News2 => "🚨",
            EscalaMedir::Sofa => "🫀",
        }
    }
}

#[component]
pub fn MeasurementPage() -> impl IntoView {
    let params = use_params_map();
    let id = move || params.get().get("id").map(|s| s.to_string()).unwrap_or_default();
    
    let query = use_query_map();
    let (escala, set_escala) = signal(EscalaMedir::default());
    let _ = query.get().get("escala").map(|s| set_escala.set(EscalaMedir::from_param(&s)));
    
    let patient_res = LocalResource::new(move || {
        let pid = id();
        async move { api::get_patient(&pid).await }
    });

    let (apache, set_apache) = signal(ApacheIIData::default());
    let (gcs, set_gcs) = signal(GcsData::default());
    let (notas, set_notas) = signal(String::new());
    let (guardando, set_guardando) = signal(false);
    let (error_msg, set_error_msg) = signal(Option::<String>::None);
    
    let current_apache_score = Memo::new(move |_| calculate_apache_ii_score(&apache.get()));
    let current_gcs_score = Memo::new(move |_| gcs.get().total());
    let current_mortality = Memo::new(move |_| mortality_risk(current_apache_score.get()));

    let navigate = use_navigate();

    let on_submit = move |_: web_sys::MouseEvent| {
        set_guardando.set(true);
        let pid = id();
        let a = apache.get();
        let g = gcs.get();
        let n = notas.get();
        let nav = navigate.clone();
        
        spawn_local(async move {
            match api::create_measurement(&pid, a, g, n).await {
                Ok(_) => {
                    set_guardando.set(false);
                    nav(&format!("/patients/{}", pid), Default::default());
                }
                Err(e) => {
                    set_guardando.set(false);
                    set_error_msg.set(Some(e));
                }
            }
        });
    };

    let on_submit = StoredValue::new(on_submit);

    view! {
        <div class="page-enter max-w-6xl mx-auto">
            <Suspense fallback=move || view! { <div class="p-10 text-uci-muted">"Cargando..."</div> }>
                {move || patient_res.get().map(|res_wrapper| match &*res_wrapper {
                    Ok(ref p) => Either::Left(view! {
                        <div class="flex flex-col lg:flex-row gap-6">
                            <div class="w-40 flex-shrink-0">
                                <header class="mb-6">
                                    <a href=format!("/patients/{}", p.patient_id) class="text-uci-muted text-xs hover:text-uci-accent block mb-2">
                                        "← Volver"
                                    </a>
                                    <h1 class="text-xl font-extrabold text-uci-text">
                                        {escala.get().icon()} " " {escala.get().label()}
                                    </h1>
                                    <p class="text-sm text-uci-muted">{p.nombre_completo()}</p>
                                </header>
                                
                                <div class="p-3 bg-uci-surface rounded-xl border border-uci-border">
                                    <div class="text-xs font-bold text-uci-muted uppercase mb-3">"Escala Clínica"</div>
                                    <div class="space-y-1">
                                        <a href=format!("/patients/{}/measure?escala=apache", p.patient_id) class="block w-full px-2 py-2 rounded text-xs font-bold text-uci-muted hover:bg-uci-card">"🏥 APACHE II"</a>
                                        <a href=format!("/patients/{}/measure?escala=gcs", p.patient_id) class="block w-full px-2 py-2 rounded text-xs font-bold text-uci-muted hover:bg-uci-card">"🧠 GCS"</a>
                                        <a href=format!("/patients/{}/measure?escala=saps3", p.patient_id) class="block w-full px-2 py-2 rounded text-xs font-bold text-uci-muted hover:bg-uci-card">"📊 SAPS III"</a>
                                        <a href=format!("/patients/{}/measure?escala=news2", p.patient_id) class="block w-full px-2 py-2 rounded text-xs font-bold text-uci-muted hover:bg-uci-card">"🚨 NEWS 2"</a>
                                        <a href=format!("/patients/{}/measure?escala=sofa", p.patient_id) class="block w-full px-2 py-2 rounded text-xs font-bold text-uci-muted hover:bg-uci-card">"🫀 SOFA"</a>
                                    </div>
                                </div>
                                
                                <div class="mt-4 p-4 bg-uci-card/30 rounded-xl border border-uci-border">
                                    <div class="text-xs text-uci-muted mb-1">"Score Actual"</div>
                                    <div class="text-3xl font-black text-uci-accent">
                                        {move || match escala.get() {
                                            EscalaMedir::GCS => current_gcs_score.get().to_string(),
                                            _ => current_apache_score.get().to_string(),
                                        }}
                                    </div>
                                    <div class="text-xs text-uci-muted mt-1">
                                        {move || format!("Riesgo: {:.1}%", current_mortality.get())}
                                    </div>
                                </div>
                            </div>
                            
                            <div class="flex-1">
                                {move || error_msg.get().map(|e| view! {
                                    <div class="bg-uci-critical/10 border border-uci-critical/30 p-4 rounded-xl mb-4 text-uci-critical text-sm">
                                        "⚠ " {e}
                                    </div>
                                })}
                                
                                <Show when=move || escala.get() == EscalaMedir::ApacheII fallback=|| ()>
                                    <FormularioCompleto escala=EscalaMedir::ApacheII />
                                </Show>
                                <Show when=move || escala.get() == EscalaMedir::GCS fallback=|| ()>
                                    <FormularioGCSOnly />
                                </Show>
                                <Show when=move || escala.get() == EscalaMedir::SAPS3 fallback=|| ()>
                                    <FormularioCompleto escala=EscalaMedir::SAPS3 />
                                </Show>
                                <Show when=move || escala.get() == EscalaMedir::News2 fallback=|| ()>
                                    <FormularioCompleto escala=EscalaMedir::News2 />
                                </Show>
                                <Show when=move || escala.get() == EscalaMedir::Sofa fallback=|| ()>
                                    <FormularioCompleto escala=EscalaMedir::Sofa />
                                </Show>
                                
                                <div class="mt-6">
                                    <label class="form-label">"Notas"</label>
                                    <textarea class="form-input" placeholder="Observaciones..." on:input=move |ev| set_notas.set(event_target_value(&ev))></textarea>
                                </div>
                                
                                <button class="btn-primary w-full py-3 mt-4" on:click=move |ev| on_submit.with_value(|f| f(ev)) disabled=guardando>
                                    {move || if guardando.get() { "Guardando..." } else { "Guardar Medición" }}
                                </button>
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
fn FormularioCompleto(escala: EscalaMedir) -> impl IntoView {
    let (apache, set_apache) = signal(ApacheIIData::default());
    
    let titulo = match escala {
        EscalaMedir::ApacheII => "🏥 APACHE II",
        EscalaMedir::SAPS3 => "📊 SAPS III",
        EscalaMedir::News2 => "🚨 NEWS 2",
        EscalaMedir::Sofa => "🫀 SOFA",
        _ => "???",
    };
    
    view! {
        <section class="bg-uci-card/30 p-4 rounded-xl border border-uci-border">
            <h2 class="text-uci-text font-bold text-lg mb-4">{titulo}</h2>
            <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                <ScaleSlider label="Temperatura" unit="°C" min=30.0 max=44.0 step=0.1 value=Signal::derive(move || apache.get().temperatura) on_change=move |v| set_apache.update(|a| a.temperatura = v) />
                <ScaleSlider label="PAM" unit="mmHg" min=0.0 max=200.0 step=1.0 value=Signal::derive(move || apache.get().presion_arterial_media) on_change=move |v| set_apache.update(|a| a.presion_arterial_media = v) />
                <ScaleSlider label="FC" unit="lpm" min=0.0 max=220.0 step=1.0 value=Signal::derive(move || apache.get().frecuencia_cardiaca) on_change=move |v| set_apache.update(|a| a.frecuencia_cardiaca = v) />
                <ScaleSlider label="FR" unit="rpm" min=0.0 max=60.0 step=1.0 value=Signal::derive(move || apache.get().frecuencia_respiratoria) on_change=move |v| set_apache.update(|a| a.frecuencia_respiratoria = v) />
                <ScaleSlider label="FiO2" unit="" min=0.21 max=1.0 step=0.01 value=Signal::derive(move || apache.get().fio2) on_change=move |v| set_apache.update(|a| a.fio2 = v) />
                <ScaleSlider label="PaO2" unit="mmHg" min=0.0 max=500.0 step=1.0 value=Signal::derive(move || apache.get().pao2.unwrap_or(80.0)) on_change=move |v| set_apache.update(|a| a.pao2 = Some(v)) />
                <ScaleSlider label="pH" unit="" min=7.0 max=7.8 step=0.01 value=Signal::derive(move || apache.get().ph_arterial) on_change=move |v| set_apache.update(|a| a.ph_arterial = v) />
                <ScaleSlider label="Sodio" unit="mEq/L" min=100.0 max=180.0 step=1.0 value=Signal::derive(move || apache.get().sodio_serico) on_change=move |v| set_apache.update(|a| a.sodio_serico = v) />
                <ScaleSlider label="Potasio" unit="mEq/L" min=1.0 max=8.5 step=0.1 value=Signal::derive(move || apache.get().potasio_serico) on_change=move |v| set_apache.update(|a| a.potasio_serico = v) />
                <ScaleSlider label="Creatinina" unit="mg/dL" min=0.1 max=12.0 step=0.1 value=Signal::derive(move || apache.get().creatinina) on_change=move |v| set_apache.update(|a| a.creatinina = v) />
                <ScaleSlider label="Leucocitos" unit="x10³" min=0.5 max=60.0 step=0.1 value=Signal::derive(move || apache.get().leucocitos) on_change=move |v| set_apache.update(|a| a.leucocitos = v) />
                <ScaleSlider label="Edad" unit="años" min=0.0 max=120.0 step=1.0 value=Signal::derive(move || apache.get().edad as f32) on_change=move |v| set_apache.update(|a| a.edad = v as u8) />
            </div>
        </section>
    }
}

#[component]
fn FormularioGCSOnly() -> impl IntoView {
    let (gcs, set_gcs) = signal(GcsData::default());
    let score = Memo::new(move |_| gcs.get().total());
    
    view! {
        <section class="bg-uci-card/30 p-4 rounded-xl border border-uci-border">
            <h2 class="text-uci-text font-bold text-lg mb-4">"🧠 Glasgow Coma Scale"</h2>
            <div class="text-center mb-4">
                <div class="text-4xl font-black text-uci-accent">{score.get()}</div>
                <div class="text-sm text-uci-muted">"/ 15"</div>
            </div>
            <div class="grid grid-cols-1 md:grid-cols-3 gap-3">
                <div>
                    <label class="form-label">"Apertura Ocular"</label>
                    <select class="form-select" on:change=move |ev| { let v = event_target_value(&ev).parse::<u8>().unwrap_or(4); set_gcs.update(|g| g.apertura_ocular = v); }>
                        <option value="4">"Espontánea (4)"</option><option value="3">"A la voz (3)"</option><option value="2">"Al dolor (2)"</option><option value="1">"Ninguna (1)"</option>
                    </select>
                </div>
                <div>
                    <label class="form-label">"Respuesta Verbal"</label>
                    <select class="form-select" on:change=move |ev| { let v = event_target_value(&ev).parse::<u8>().unwrap_or(5); set_gcs.update(|g| g.respuesta_verbal = v); }>
                        <option value="5">"Orientado (5)"</option><option value="4">"Confuso (4)"</option><option value="3">"Palabras (3)"</option><option value="2">"Sonidos (2)"</option><option value="1">"Ninguna (1)"</option>
                    </select>
                </div>
                <div>
                    <label class="form-label">"Respuesta Motora"</label>
                    <select class="form-select" on:change=move |ev| { let v = event_target_value(&ev).parse::<u8>().unwrap_or(6); set_gcs.update(|g| g.respuesta_motora = v); }>
                        <option value="6">"Obedece (6)"</option><option value="5">"Localiza (5)"</option><option value="4">"Retira (4)"</option><option value="3">"Flexión (3)"</option><option value="2">"Extensión (2)"</option><option value="1">"Ninguna (1)"</option>
                    </select>
                </div>
            </div>
        </section>
    }
}