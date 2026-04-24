use leptos::prelude::*;
use leptos::either::Either;
use leptos::task::spawn_local;
use leptos_router::hooks::*;
use dmart_shared::models::*;
use dmart_shared::scales::{calculate_apache_ii_score, calculate_sofa_score, mortality_risk};
use crate::api;
use crate::components::scales::*;
use crate::components::radar_chart::{RadarChart, RadarData};

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
enum EscalaMedir {
    #[default]
    ApacheII,
    GCS,
    SOFA,
    SAPS3,
    News2,
}

impl EscalaMedir {
    fn label(&self) -> &'static str {
        match self {
            EscalaMedir::ApacheII => "APACHE II",
            EscalaMedir::GCS => "GCS",
            EscalaMedir::SOFA => "SOFA",
            EscalaMedir::SAPS3 => "SAPS III",
            EscalaMedir::News2 => "NEWS2",
        }
    }
    
    fn from_param(s: &str) -> Self {
        match s {
            "gcs" => EscalaMedir::GCS,
            "saps3" => EscalaMedir::SAPS3,
            "news2" => EscalaMedir::News2,
            "sofa" => EscalaMedir::SOFA,
            _ => EscalaMedir::ApacheII,
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

    let apache_data = RwSignal::new(ApacheIIData::default());
    let gcs_data = RwSignal::new(GcsData::default());

    // Signals para scores calculados en tiempo real (para el radar)
    let apache_score = Memo::new(move |_| calculate_apache_ii_score(&apache_data.get()));
    let gcs_score = Memo::new(move |_| gcs_data.get().total() as u8);
    let apache_mortality = Memo::new(move |_| mortality_risk(apache_score.get()));
    let sofa_score = Memo::new(move |_| calculate_sofa_score(&apache_data.get()) as u8);

    let notas = RwSignal::new(String::new());
    let guardando = RwSignal::new(false);
    let error_msg = RwSignal::new(Option::<String>::None);
    let navigate = use_navigate();

    let on_save = Callback::new(move |_| {
        guardando.set(true);
        let pid = id();
        let esc = escala.get();
        let n = Some(notas.get());
        let nav = navigate.clone();
        
        spawn_local(async move {
            let res = match esc {
                EscalaMedir::ApacheII => {
                    let d = apache_data.get();
                    api::calc_apache(&pid, d, n).await.map(|_| ())
                }
                EscalaMedir::GCS => {
                    let g = gcs_data.get();
                    api::calc_gcs(&pid, g.apertura_ocular, g.respuesta_verbal, g.respuesta_motora, n).await.map(|_| ())
                }
                EscalaMedir::SOFA => {
                    let d = apache_data.get();
                    let g = gcs_data.get();
                    api::calc_sofa(&pid, d.pao2.unwrap_or(80.0), d.fio2, d.plaquetas, d.bilirrubina, d.presion_arterial_media, d.vasopresores, d.dosis_vasopresor, g.total(), d.creatinina, d.diuresis_diaria, n).await.map(|_| ())
                }
                EscalaMedir::SAPS3 => {
                    let d = apache_data.get();
                    api::calc_saps3(&pid, d.edad as u8, 0, d.tipo_admision.clone(), d.fuente_admision.clone(), n).await.map(|_| ())
                }
                EscalaMedir::News2 => {
                    let d = apache_data.get();
                    api::calc_news2(&pid, d.frecuencia_respiratoria, d.spo2, d.o2_suplementario, d.presion_sistolica, d.frecuencia_cardiaca, d.temperatura, d.alerta, n).await.map(|_| ())
                }
            };

            match res {
                Ok(_) => {
                    guardando.set(false);
                    nav(&format!("/patients/{}", pid), Default::default());
                }
                Err(e) => {
                    guardando.set(false);
                    error_msg.set(Some(e));
                }
            }
        });
    });

    view! {
        <div class="w-full min-h-screen" style="background:var(--uci-bg);">
            <Suspense fallback=move || view! { 
                <div class="flex items-center justify-center min-h-[60vh]">
                    <div class="flex flex-col items-center gap-4">
                        <div class="w-12 h-12 border-4 rounded-full animate-spin" style="border-color:var(--uci-accent); border-top-color:transparent;"></div>
                        <span class="text-sm font-bold uppercase tracking-widest" style="color:var(--uci-muted);">"Cargando..."</span>
                    </div>
                </div> 
            }>
                {move || patient_res.get().map(|res_wrapper| match res_wrapper {
                    Ok(ref p) => Either::Left(view! {
                        <div class="max-w-[1800px] mx-auto px-4 py-6">
                            // Header
                            <div class="mb-6">
                                <a href=format!("/patients/{}", p.patient_id) 
                                   class="text-sm font-bold flex items-center gap-2 mb-3 uppercase tracking-widest transition-all no-underline"
                                   style="color:var(--uci-muted);" 
                                   onmouseenter="this.style.color='var(--uci-accent)'" 
                                   onmouseleave="this.style.color='var(--uci-muted)'">
                                    <i class="fa-solid fa-arrow-left"></i>
                                    "Volver al Paciente"
                                </a>
                                <h1 class="text-2xl md:text-3xl font-black" style="color:var(--uci-text);">
                                    {p.nombre_completo()}
                                </h1>
                                <div class="flex items-center gap-3 text-sm font-bold mt-1" style="color:var(--uci-muted);">
                                    <span class="px-3 py-0.5 rounded-full text-xs" style="background:rgba(59,130,246,0.1); color:var(--uci-accent);">{p.cedula.clone()}</span>
                                    <span>"•"</span>
                                    <span>"Nueva Medición"</span>
                                </div>
                            </div>

                            // Tabs para seleccionar escala a registrar
                            <div class="mb-6">
                                <div class="flex gap-2 p-1 rounded-xl overflow-x-auto" 
                                     style="background:var(--uci-surface); border:1px solid var(--uci-border);">
                                    <ScaleTab active=Signal::derive(move || escala.get() == EscalaMedir::ApacheII) 
                                        on_click=Callback::new(move |_| set_escala.set(EscalaMedir::ApacheII)) 
                                        label="APACHE II" icon="fa-heart-pulse" />
                                    <ScaleTab active=Signal::derive(move || escala.get() == EscalaMedir::GCS) 
                                        on_click=Callback::new(move |_| set_escala.set(EscalaMedir::GCS)) 
                                        label="GCS" icon="fa-brain" />
                                    <ScaleTab active=Signal::derive(move || escala.get() == EscalaMedir::SOFA) 
                                        on_click=Callback::new(move |_| set_escala.set(EscalaMedir::SOFA)) 
                                        label="SOFA" icon="fa-lungs" />
                                    <ScaleTab active=Signal::derive(move || escala.get() == EscalaMedir::SAPS3) 
                                        on_click=Callback::new(move |_| set_escala.set(EscalaMedir::SAPS3)) 
                                        label="SAPS III" icon="fa-chart-line" />
                                    <ScaleTab active=Signal::derive(move || escala.get() == EscalaMedir::News2) 
                                        on_click=Callback::new(move |_| set_escala.set(EscalaMedir::News2)) 
                                        label="NEWS2" icon="fa-bell" />
                                </div>
                            </div>

                            // Contenido principal
                            <div class="grid grid-cols-1 xl:grid-cols-[1fr_320px] gap-6">
                                // Escala activa (formulario)
                                <div class="min-w-0">
                                    <div class="animate-fade-in">
                                        {move || match escala.get() {
                                            EscalaMedir::ApacheII => view! { <ApacheIIScale data=apache_data /> }.into_any(),
                                            EscalaMedir::GCS      => view! { <GcsScale data=gcs_data /> }.into_any(),
                                            EscalaMedir::SOFA     => view! { <SofaScale data=apache_data /> }.into_any(),
                                            EscalaMedir::SAPS3    => view! { <Saps3Scale data=apache_data /> }.into_any(),
                                            EscalaMedir::News2    => view! { <News2Scale data=apache_data /> }.into_any(),
                                        }}
                                    </div>
                                </div>

                                // Sidebar con scores en tiempo real
                                <div class="xl:sticky xl:top-6 xl:self-start flex flex-col gap-4">
                                    // Radar Chart
                                    <div class="rounded-2xl p-5" style="background:var(--uci-surface); border:1px solid var(--uci-border);">
                                        <h3 class="text-xs font-black uppercase tracking-[0.2em] mb-4 text-center" style="color:var(--uci-muted);">
                                            <i class="fa-solid fa-chart-polar mr-2" style="color:var(--uci-accent);"></i>
                                            "Scores en Tiempo Real"
                                        </h3>
                                        <div class="flex justify-center">
                                            <RadarChart data={vec![
                                                RadarData { label: "APACHE II", value: apache_score.get() as f32, max: 71.0, warning: 20.0, critical: 30.0 },
                                                RadarData { label: "GCS", value: gcs_score.get() as f32, max: 15.0, warning: 12.0, critical: 8.0 },
                                                RadarData { label: "SOFA", value: sofa_score.get() as f32, max: 24.0, warning: 12.0, critical: 18.0 },
                                            ]} size=260 />
                                        </div>
                                    </div>

                                    // Stats cards
                                    <div class="grid grid-cols-3 gap-3">
                                        <div class="rounded-xl p-3 text-center" style="background:var(--uci-surface); border:1px solid var(--uci-border);">
                                            <div class="text-2xl font-black" style="color:#10B981;">{move || apache_score.get()}</div>
                                            <div class="text-[10px] font-bold uppercase" style="color:var(--uci-muted);">"APACHE"</div>
                                        </div>
                                        <div class="rounded-xl p-3 text-center" style="background:var(--uci-surface); border:1px solid var(--uci-border);">
                                            <div class="text-2xl font-black" style="color:#8B5CF6;">{move || gcs_score.get()}</div>
                                            <div class="text-[10px] font-bold uppercase" style="color:var(--uci-muted);">"GCS"</div>
                                        </div>
                                        <div class="rounded-xl p-3 text-center" style="background:var(--uci-surface); border:1px solid var(--uci-border);">
                                            <div class="text-2xl font-black" style="color:#06B6D4;">{move || sofa_score.get()}</div>
                                            <div class="text-[10px] font-bold uppercase" style="color:var(--uci-muted);">"SOFA"</div>
                                        </div>
                                    </div>

                                    // Mortalidad
                                    <div class="rounded-2xl p-4" style="background:linear-gradient(135deg, var(--uci-surface), var(--uci-card));">
                                        <div class="flex items-center justify-between mb-2">
                                            <span class="text-xs font-bold uppercase tracking-wider" style="color:var(--uci-muted);">"Riesgo Mortalidad"</span>
                                            <i class="fa-solid fa-skull text-sm" style="color:var(--uci-muted);"></i>
                                        </div>
                                        <div class="text-4xl font-black text-center" style="color:var(--uci-critical);">
                                            {move || format!("{:.1}%", apache_mortality.get())}
                                        </div>
                                    </div>

                                    // Observaciones y guardar
                                    <div class="rounded-2xl p-4 space-y-4" style="background:var(--uci-surface); border:1px solid var(--uci-border);">
                                        <div>
                                            <label class="text-xs font-black uppercase tracking-[0.2em] mb-2 block" style="color:var(--uci-muted);">"Observaciones"</label>
                                            <textarea 
                                                class="w-full p-3 rounded-xl text-sm resize-none"
                                                style="background:var(--uci-card); border:1px solid var(--uci-border); color:var(--uci-text);"
                                                rows="2"
                                                placeholder="Notas clínicas..."
                                                prop:value=move || notas.get()
                                                on:input=move |ev| notas.set(event_target_value(&ev))
                                            ></textarea>
                                        </div>

                                        {move || error_msg.get().map(|e| view! {
                                            <div class="p-3 rounded-xl text-xs font-bold" 
                                                 style="background:rgba(239,68,68,0.1); border:1px solid rgba(239,68,68,0.3); color:#DC2626;">
                                                <i class="fa-solid fa-circle-exclamation mr-1"></i>
                                                {e}
                                            </div>
                                        })}
                                        
                                        <button 
                                            class="w-full py-3 rounded-xl font-black text-sm flex items-center justify-center gap-2 transition-all"
                                            style="background:var(--uci-accent); color:white;"
                                            on:click={
                                                let on_save = on_save.clone();
                                                move |_| on_save.run(())
                                            }
                                            disabled=move || guardando.get()
                                        >
                                            {move || if guardando.get() {
                                                Either::Left(view! { <i class="fa-solid fa-circle-notch animate-spin"></i> })
                                            } else {
                                                Either::Right(view! { 
                                                    <><i class="fa-solid fa-check"></i> "Registrar " {escala.get().label()}</>
                                                })
                                            }}
                                        </button>
                                    </div>
                                </div>
                            </div>
                        </div>
                    }),
                    _ => Either::Right(view! { 
                        <div class="p-20 text-center">
                            <div class="text-4xl font-black mb-4" style="color:var(--uci-critical);">"Error"</div>
                            <p style="color:var(--uci-muted);">"No se pudo cargar el expediente del paciente"</p>
                        </div> 
                    })
                })}
            </Suspense>
        </div>
    }
}

#[component]
fn ScaleTab(
    active: Signal<bool>,
    on_click: Callback<web_sys::MouseEvent>,
    label: &'static str,
    icon: &'static str,
) -> impl IntoView {
    view! {
        <button 
            class=move || format!("flex items-center gap-2 px-4 py-2 rounded-lg font-bold text-xs whitespace-nowrap transition-all {}", 
                if active.get() { "scale-105" } else { "opacity-70 hover:opacity-100" })
            style=move || format!(
                "background:{}; color:{}; border:1px solid {};",
                if active.get() { "var(--uci-accent)" } else { "transparent" },
                if active.get() { "white" } else { "var(--uci-text)" },
                if active.get() { "var(--uci-accent)" } else { "transparent" }
            )
            on:click=move |ev| on_click.run(ev)
        >
            <i class=format!("fa-solid {}", icon)></i>
            <span>{label}</span>
        </button>
    }
}