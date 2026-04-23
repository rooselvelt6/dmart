use leptos::prelude::*;
use leptos::either::Either;
use leptos::task::spawn_local;
use leptos_router::hooks::*;
use dmart_shared::models::*;
use crate::api;
use crate::components::scales::*;

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

                            // Independent Scale Content
                            <div class="animate-fade-in">
                                {move || match escala.get() {
                                    EscalaMedir::ApacheII => view! { <ApacheIIScale data=apache_data /> }.into_any(),
                                    EscalaMedir::GCS      => view! { <GcsScale data=gcs_data /> }.into_any(),
                                    EscalaMedir::SOFA     => view! { <SofaScale data=apache_data /> }.into_any(),
                                    EscalaMedir::SAPS3    => view! { <Saps3Scale data=apache_data /> }.into_any(),
                                    EscalaMedir::News2    => view! { <News2Scale data=apache_data /> }.into_any(),
                                }}
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