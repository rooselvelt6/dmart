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
    
    // SOFA requiere datos adicionales - por ahora calculamos con datos disponibles
    let sofa_score = Memo::new(move |_| {
        let d = apache_data.get();
        calculate_sofa_score(&d) as u8
    });

    // Datos del radar basados en la escala actual
    let radar_data = Memo::new(move |_| -> Vec<RadarData> {
        let esc = escala.get();
        match esc {
            EscalaMedir::ApacheII => {
                let score = apache_score.get();
                vec![
                    RadarData {
                        label: "APACHE II",
                        value: score as f32,
                        max: 71.0,
                        warning: 20.0,
                        critical: 30.0,
                    },
                ]
            }
            EscalaMedir::GCS => {
                vec![
                    RadarData {
                        label: "GCS",
                        value: gcs_score.get() as f32,
                        max: 15.0,
                        warning: 12.0,
                        critical: 8.0,
                    },
                ]
            }
            _ => {
                // Para otras escalas, mostrar preview de todas
                vec![
                    RadarData {
                        label: "APACHE II",
                        value: apache_score.get() as f32,
                        max: 71.0,
                        warning: 20.0,
                        critical: 30.0,
                    },
                    RadarData {
                        label: "GCS",
                        value: gcs_score.get() as f32,
                        max: 15.0,
                        warning: 12.0,
                        critical: 8.0,
                    },
                    RadarData {
                        label: "SOFA",
                        value: sofa_score.get() as f32,
                        max: 24.0,
                        warning: 12.0,
                        critical: 18.0,
                    },
                ]
            }
        }
    });

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
        <div class="page-enter w-full max-w-7xl mx-auto px-4 py-5 md:py-6 lg:py-8">
            <Suspense fallback=move || view! { 
                <div class="flex items-center justify-center min-h-[300px] md:min-h-[400px]">
                    <div class="flex flex-col items-center gap-4">
                        <div class="w-10 h-10 md:w-12 md:h-12 border-2 md:border-4 rounded-full animate-spin" style="border-color:var(--uci-accent); border-top-color:transparent;"></div>
                        <span class="text-xs md:text-sm font-bold uppercase tracking-widest" style="color:var(--uci-muted);">"Cargando Expediente..."</span>
                    </div>
                </div> 
            }>
                {move || patient_res.get().map(|res_wrapper| match &*res_wrapper {
                    Ok(ref p) => Either::Left(view! {
                        <div class="space-y-5 md:space-y-6 lg:space-y-8">
                            // Header
                            <div class="flex flex-col md:flex-row justify-between items-start md:items-center gap-4 md:gap-6">
                                <div>
                                    <a href=format!("/patients/{}", p.patient_id) class="text-xs font-bold flex items-center gap-2 mb-3 md:mb-4 uppercase tracking-widest transition-all no-underline" style="color:var(--uci-muted);" onmouseenter="this.style.color='var(--uci-accent)'" onmouseleave="this.style.color='var(--uci-muted)'">
                                        <i class="fa-solid fa-arrow-left-long"></i>
                                        "Volver al Perfil del Paciente"
                                    </a>
                                    <h1 class="text-xl md:text-2xl lg:text-3xl lg:text-4xl font-black tracking-tight" style="color:var(--uci-text);">
                                        {p.nombre_completo()}
                                    </h1>
                                    <div class="flex items-center gap-2 md:gap-3 text-sm font-bold mt-2" style="color:var(--uci-muted);">
                                        <span class="px-2 py-0.5 md:px-3 rounded-full text-xs" style="background:rgba(59,130,246,0.1); color:var(--uci-accent);">{p.cedula.clone()}</span>
                                        <span>"•"</span>
                                        <span class="text-xs md:text-sm">"UCI - Monitorización"</span>
                                    </div>
                                </div>

                                <div class="flex gap-1 md:gap-2 p-1 rounded-xl shadow-sm overflow-x-auto" style="background:var(--uci-surface); border:1px solid var(--uci-border);">
                                    <ScaleTab active=Signal::derive(move || escala.get() == EscalaMedir::ApacheII) on_click=Callback::new(move |_| set_escala.set(EscalaMedir::ApacheII)) label="APACHE II" icon="fa-heart-pulse" />
                                    <ScaleTab active=Signal::derive(move || escala.get() == EscalaMedir::GCS) on_click=Callback::new(move |_| set_escala.set(EscalaMedir::GCS)) label="GCS" icon="fa-brain" />
                                    <ScaleTab active=Signal::derive(move || escala.get() == EscalaMedir::SOFA) on_click=Callback::new(move |_| set_escala.set(EscalaMedir::SOFA)) label="SOFA" icon="fa-lungs" />
                                    <ScaleTab active=Signal::derive(move || escala.get() == EscalaMedir::SAPS3) on_click=Callback::new(move |_| set_escala.set(EscalaMedir::SAPS3)) label="SAPS III" icon="fa-chart-line" />
                                    <ScaleTab active=Signal::derive(move || escala.get() == EscalaMedir::News2) on_click=Callback::new(move |_| set_escala.set(EscalaMedir::News2)) label="NEWS2" icon="fa-bell" />
                                </div>
                            </div>

                            // Content with integrated radar chart
                            <div class="flex flex-col lg:flex-row gap-5 md:gap-6">
                                // Scale Content
                                <div class="flex-1 animate-fade-in">
                                    {move || match escala.get() {
                                        EscalaMedir::ApacheII => view! { <ApacheIIScale data=apache_data /> }.into_any(),
                                        EscalaMedir::GCS      => view! { <GcsScale data=gcs_data /> }.into_any(),
                                        EscalaMedir::SOFA     => view! { <SofaScale data=apache_data /> }.into_any(),
                                        EscalaMedir::SAPS3    => view! { <Saps3Scale data=apache_data /> }.into_any(),
                                        EscalaMedir::News2    => view! { <News2Scale data=apache_data /> }.into_any(),
                                    }}
                                </div>

                                // Sidebar con Radar Chart y Stats
                                <div class="lg:w-80 xl:w-96 flex flex-col gap-4">
                                    // Radar Chart Card
                                    <div class="glass-card p-5 flex flex-col items-center" style="background:var(--uci-surface);">
                                        <h3 class="text-xs font-black uppercase tracking-[0.2em] mb-4" style="color:var(--uci-muted);">
                                            <i class="fa-solid fa-chart-polar fa-lg mr-2" style="color:var(--uci-accent);"></i>
                                            "Vista Rápida de Scores"
                                        </h3>
                                        <div class="flex justify-center">
                                            <RadarChart data={radar_data.get()} size=280 />
                                        </div>
                                        <div class="mt-4 grid grid-cols-3 gap-2 w-full text-center">
                                            <div class="p-2 rounded-lg" style="background:var(--uci-card);">
                                                <div class="text-lg font-black" style="color:#10B981;">{move || format!("{:.0}", apache_score.get())}</div>
                                                <div class="text-[10px] font-bold uppercase" style="color:var(--uci-muted);">"APACHE"</div>
                                            </div>
                                            <div class="p-2 rounded-lg" style="background:var(--uci-card);">
                                                <div class="text-lg font-black" style="color:#3B82F6;">{move || gcs_score.get()}</div>
                                                <div class="text-[10px] font-bold uppercase" style="color:var(--uci-muted);">"GCS"</div>
                                            </div>
                                            <div class="p-2 rounded-lg" style="background:var(--uci-card);">
                                                <div class="text-lg font-black" style="color:#8B5CF6;">{move || sofa_score.get()}</div>
                                                <div class="text-[10px] font-bold uppercase" style="color:var(--uci-muted);">"SOFA"</div>
                                            </div>
                                        </div>
                                    </div>

                                    // Mortality Risk Card
                                    <div class="glass-card p-5" style="background:linear-gradient(135deg, var(--uci-surface), var(--uci-card));">
                                        <div class="flex items-center justify-between mb-4">
                                            <span class="text-sm font-bold uppercase tracking-wider" style="color:var(--uci-muted);">"Riesgo de Mortalidad"</span>
                                            <i class="fa-solid fa-skull-crossbones" style="color:var(--uci-muted);"></i>
                                        </div>
                                        <div class="text-5xl font-black text-center mb-3" style="color:var(--uci-critical);">
                                            {move || format!("{:.1}%", apache_mortality.get())}
                                        </div>
                                        <div class="h-3 rounded-full overflow-hidden" style="background:var(--uci-border);">
                                            <div class="h-full rounded-full transition-all duration-500" 
                                                 style=move || format!("width:{}%; background:linear-gradient(90deg, #10B981, #F59E0B, #EF4444);", apache_mortality.get().min(100.0))>
                                            </div>
                                        </div>
                                        <p class="text-xs text-center mt-3 italic" style="color:var(--uci-muted);">"Basado en ecuación de Knaus et al. (1985)"</p>
                                    </div>

                                    // Severity Badge
                                    <div class="glass-card p-5 text-center" style="background:var(--uci-surface);">
                                        <div class="text-xs font-bold uppercase tracking-widest mb-3" style="color:var(--uci-muted);">"Severidad APACHE II"</div>
                                        <div class="text-3xl font-black uppercase tracking-wider" style="color:var(--uci-accent);">
                                            {move || SeverityLevel::from_score(apache_score.get()).label()}
                                        </div>
                                    </div>
                                </div>
                            </div>

                            // Shared Actions & Notes
                            <div class="grid grid-cols-1 lg:grid-cols-3 gap-4 md:gap-6">
                                <div class="lg:col-span-2 glass-card p-4 md:p-6 lg:p-8" style="background:var(--uci-surface);">
                                    <label class="text-xs font-black uppercase tracking-[0.2em] mb-3 md:mb-4 block" style="color:var(--uci-muted);">"Observaciones Clínicas (Opcional)"</label>
                                    <textarea 
                                        class="form-input min-h-[80px] md:min-h-[100px] lg:min-h-[120px] text-sm md:text-base"
                                        placeholder="Describa hallazgos adicionales..."
                                        prop:value=move || notas.get()
                                        on:input=move |ev| notas.set(event_target_value(&ev))
                                    ></textarea>
                                </div>
                                
                                <div class="flex flex-col justify-end gap-3 md:gap-4">
                                    {move || error_msg.get().map(|e| view! {
                                        <div class="p-3 md:p-4 rounded-xl text-xs font-bold flex items-center gap-2" style="background:rgba(239,68,68,0.1); border:1px solid rgba(239,68,68,0.3); color:#DC2626;">
                                            <i class="fa-solid fa-circle-exclamation text-base"></i>
                                            {e}
                                        </div>
                                    })}
                                    
                                    <button 
                                        class="btn-primary h-12 md:h-14 lg:h-16 text-base md:text-lg font-black flex items-center justify-center gap-2 md:gap-3"
                                        on:click={
                                            let on_save = on_save.clone();
                                            move |_| on_save.run(())
                                        }
                                        disabled=move || guardando.get()
                                    >
                                        {move || if guardando.get() {
                                            Either::Left(view! { <i class="fa-solid fa-circle-notch animate-spin text-lg md:text-xl"></i> })
                                        } else {
                                            Either::Right(view! { 
                                                <>
                                                    <i class="fa-solid fa-clipboard-check text-lg md:text-xl"></i>
                                                    {move || match escala.get() {
                                                        EscalaMedir::ApacheII => "Registrar APACHE II",
                                                        EscalaMedir::GCS      => "Registrar Glasgow",
                                                        EscalaMedir::SOFA     => "Registrar SOFA",
                                                        EscalaMedir::SAPS3    => "Registrar SAPS III",
                                                        EscalaMedir::News2    => "Registrar NEWS2",
                                                    }}
                                                </>
                                            })
                                        }}
                                    </button>
                                    <p class="text-[10px] md:text-xs font-bold text-center leading-relaxed" style="color:var(--uci-muted);">
                                        "Se guardará únicamente la escala seleccionada en el historial del paciente"
                                    </p>
                                </div>
                            </div>
                        </div>
                    }),
                    _ => Either::Right(view! { <div class="p-20 text-center font-black text-rose-500">"Error al cargar expediente"</div> })
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
            class=move || format!("flex items-center gap-1 md:gap-2 px-3 md:px-4 lg:px-5 py-2 md:py-3 rounded-xl font-black text-[10px] md:text-xs whitespace-nowrap transition-all {}", 
                if active.get() { "scale-105 z-10" } else { "hover:opacity-80" })
            style=move || format!(
                "background:{}; color:{}; box-shadow:{}; border:1px solid {};",
                if active.get() { "var(--uci-accent)" } else { "var(--uci-surface)" },
                if active.get() { "white" } else { "var(--uci-text)" },
                if active.get() { "0 4px 12px rgba(37,99,235,0.3)" } else { "none" },
                if active.get() { "var(--uci-accent)" } else { "var(--uci-border)" }
            )
            on:click=move |ev| on_click.run(ev)
        >
            <i class=format!("fa-solid {}", icon)></i>
            <span class="hidden md:inline">{label}</span>
        </button>
    }
}