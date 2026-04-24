use leptos::prelude::*;
use leptos::either::Either;
use dmart_shared::models::*;
use crate::api;
use crate::components::{severity_badge::SeverityBadge, chart::EvolutionChart};

#[component]
pub fn DashboardPage() -> impl IntoView {
    let patients = LocalResource::new(|| async move {
        api::list_patients(None).await.unwrap_or_default()
    });

    view! {
        <div class="page-enter">
            <div class="mb-5 md:mb-7">
                <h1 class="text-xl md:text-2xl lg:text-3xl font-extrabold" style="color:var(--uci-text); margin:0 0 4px;">"Panel de Monitoreo"</h1>
                <p style="color:var(--uci-muted); font-size:13px; margin:0;">"Pacientes activos en UCI — visión general y evolución"</p>
            </div>

            <Suspense fallback=move || view! {
                <div style="text-align:center; padding:60px; color:var(--uci-muted);">
                    <div style="font-size:32px; margin-bottom:8px;"><i class="fa-solid fa-spinner fa-spin"></i></div>
                    "Cargando pacientes..."
                </div>
            }>
                {move || patients.get().map(|list_wrapper| {
                    let list = &*list_wrapper;
                    let total = list.len();
                    let criticos = list.iter().filter(|p| matches!(p.estado_gravedad, SeverityLevel::Critico)).count();
                    let severos  = list.iter().filter(|p| matches!(p.estado_gravedad, SeverityLevel::Severo)).count();
                    let moderados= list.iter().filter(|p| matches!(p.estado_gravedad, SeverityLevel::Moderado)).count();
                    let bajos    = list.iter().filter(|p| matches!(p.estado_gravedad, SeverityLevel::Bajo)).count();

                    view! {
                        <div>
                            <div class="grid grid-cols-2 lg:grid-cols-4 gap-3 md:gap-4 mb-6 md:mb-7">
                                {stat_card("Total Pacientes", &total.to_string(), "#3B82F6", "fa-users")}
                                {stat_card("Criticos", &criticos.to_string(), "#EF4444", "fa-skull")}
                                {stat_card("Severos", &severos.to_string(), "#F97316", "fa-triangle-exclamation")}
                                {stat_card("Estables", &format!("{}", moderados + bajos), "#10B981", "fa-check-circle")}
                            </div>

                            <div class="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
                                {list.into_iter().map(|patient| view! {
                                    <PatientPokemonCard patient=patient.clone() />
                                }).collect_view()}
                            </div>

                            {if total == 0 {
                                Either::Left(view! {
                                    <div style="text-align:center; padding:80px 40px; color:var(--uci-muted);">
                                        <div style="font-size:48px; margin-bottom:16px; opacity:0.3;"><i class="fa-solid fa-hospital"></i></div>
                                        <div style="font-size:20px; font-weight:600; margin-bottom:8px;" class="text-uci-text">"No hay pacientes registrados"</div>
                                        <p style="font-size:14px; margin:0 0 20px;">"Comience registrando el primer paciente"</p>
                                        <a href="/patients/new" class="btn-primary" style="display:inline-block; text-decoration:none;"><i class="fa-solid fa-plus mr-2"></i>"Nuevo Paciente"</a>
                                    </div>
                                })
                            } else {
                                Either::Right(view! { <span></span> })
                            }}
                        </div>
                    }
                })}
            </Suspense>
        </div>
    }
}

#[component]
fn PatientPokemonCard(patient: PatientListItem) -> impl IntoView {
    let id = patient.id.clone();
    let id_for_chart = patient.id.clone();

    let measurements = LocalResource::new(move || {
        let pid = id_for_chart.clone();
        async move {
            api::get_measurements(&pid).await.unwrap_or_default()
        }
    });

    let severity_config = match &patient.estado_gravedad {
        SeverityLevel::Critico => ("#EF4444", "rgba(239,68,68,0.1)"),
        SeverityLevel::Severo => ("#F97316", "rgba(249,115,22,0.1)"),
        SeverityLevel::Moderado => ("#F59E0B", "rgba(245,158,11,0.1)"),
        SeverityLevel::Bajo => ("#10B981", "rgba(16,185,129,0.1)"),
    };

    let sex_icon = match patient.sexo {
        Sexo::Masculino => ("fa-mars", "#3B82F6"),
        Sexo::Femenino => ("fa-venus", "#EC4899"),
    };

    view! {
        <div class="glass-card p-0 overflow-hidden"
             style=format!("border-top:4px solid {}; border-radius:16px;", severity_config.0)
             onmouseenter="this.style.transform='translateY(-4px)'; this.style.boxShadow='0 12px 40px rgba(0,0,0,0.2)'"
             onmouseleave="this.style.transform=''; this.style.boxShadow=''">

            // Header
            <div class="p-4 flex items-center justify-between" style="background:linear-gradient(135deg,rgba(30,41,59,0.05),transparent); border-bottom:1px solid var(--uci-border);">
                <div class="flex items-center gap-3">
                    <div class="w-12 h-12 rounded-full flex items-center justify-center text-lg font-bold" style="background:{}; color:white;">
                        {patient.nombre_completo.chars().next().unwrap_or('P')}
                    </div>
                    <div>
                        <div class="font-bold text-base" style="color:var(--uci-text);">{patient.nombre_completo.clone()}</div>
                        <div class="flex items-center gap-2 text-xs" style="color:var(--uci-muted);">
                            <span><i class="fa-solid fa-id-card mr-1"></i>{patient.cedula.clone()}</span>
                            <span><i class="fa-solid fa-folder mr-1"></i>{patient.historia_clinica.clone()}</span>
                        </div>
                    </div>
                </div>
                <SeverityBadge level=patient.estado_gravedad.clone() />
            </div>

            // Scores circles
            <div class="p-4 grid grid-cols-4 gap-2">
                <ScoreCircle 
                    value=patient.ultimo_apache_score.map(|s| s.to_string()).unwrap_or_else(|| "-".into())
                    icon="fa-heart-pulse"
                    color="#EF4444".to_string()
                    label="APACHE"
                />
                <ScoreCircle 
                    value={format!("{}/15", patient.ultimo_gcs_score.map(|s| s.to_string()).unwrap_or_else(|| "-".into()))}
                    icon="fa-brain"
                    color="#8B5CF6".to_string()
                    label="GCS"
                />
                <ScoreCircle 
                    value=patient.ultimo_sofa_score.map(|s| s.to_string()).unwrap_or_else(|| "-".into())
                    icon="fa-lungs"
                    color="#10B981".to_string()
                    label="SOFA"
                />
                <ScoreCircle 
                    value=patient.ultimo_news2_score.map(|s| s.to_string()).unwrap_or_else(|| "-".into())
                    icon="fa-chart-line"
                    color="#F59E0B".to_string()
                    label="NEWS2"
                />
            </div>

            // Info row
            <div class="px-4 pb-3 flex flex-wrap gap-2">
                <InfoBadge icon="fa-syringe" value=patient.ultimo_saps3_score.map(|s| s.to_string()).unwrap_or_else(|| "-".into()) color="#3B82F6".to_string() />
                <InfoBadge icon="fa-skull" value={patient.mortality_risk.map(|m| format!("{:.0}%", m)).unwrap_or_else(|| "-".into())} color="#EF4444".to_string() />
                <InfoBadge icon="fa-user" value={format!("{} anios", patient.edad)} color="#64748B".to_string() />
                <InfoBadge icon=sex_icon.0 value="".to_string() color=sex_icon.1.to_string() />
            </div>

            // Chart
            <Suspense fallback=move || view! {
                <div style="height:50px; background:var(--uci-surface); display:flex; align-items:center; justify-content:center; color:var(--uci-muted); font-size:11px;">
                    <i class="fa-solid fa-spinner fa-spin mr-2"></i>"Cargando..."
                </div>
            }>
                {move || measurements.get().map(|ms| {
                    let ms = &*ms;
                    if ms.is_empty() {
                        Either::Left(view! {
                            <div style="height:50px; background:var(--uci-surface); display:flex; align-items:center; justify-content:center; font-size:11px; color:var(--uci-muted);">
                                <i class="fa-solid fa-chart-line mr-2 opacity-50"></i>"Sin datos de evolucion"
                            </div>
                        })
                    } else {
                        Either::Right(view! {
                            <EvolutionChart measurements=ms.to_vec() height=50 compact=true />
                        })
                    }
                })}
            </Suspense>

            // Actions
            <div class="flex gap-2 p-3" style="border-top:1px solid var(--uci-border);">
                <a href=format!("/patients/{}", id.clone())
                   class="flex-1 text-center py-2 rounded-lg font-semibold text-sm"
                   style="background:var(--uci-accent); color:white; text-decoration:none;">
                    <i class="fa-solid fa-address-card mr-1"></i>"Perfil"
                </a>
                <a href=format!("/patients/{}/measure", id.clone())
                   class="flex-1 text-center py-2 rounded-lg font-semibold text-sm"
                   style="background:var(--uci-low); color:white; text-decoration:none;">
                    <i class="fa-solid fa-plus mr-1"></i>"Medir"
                </a>
            </div>
        </div>
    }
}

#[component]
fn ScoreCircle(value: String, icon: &'static str, color: String, label: &'static str) -> impl IntoView {
    view! {
        <div class="flex flex-col items-center">
            <div class="w-14 h-14 rounded-full flex items-center justify-center" style={format!("background:{}; color:white; border:2px solid {};", color, color)}>
                <i class={format!("fa-solid {}", icon)}></i>
            </div>
            <div class="text-lg font-bold mt-1" style={format!("color:{}", color)}>{value}</div>
            <div class="text-[10px] uppercase font-semibold" style="color:var(--uci-muted);">{label}</div>
        </div>
    }
}

#[component]
fn InfoBadge(icon: &'static str, value: String, color: String) -> impl IntoView {
    view! {
        <div class="flex items-center gap-1 px-2 py-1 rounded-lg text-xs font-semibold" style={format!("background:{}; color:white;", color)}>
            <i class={format!("fa-solid {}", icon)}></i>
            {value}
        </div>
    }
}

fn stat_card(title: &str, value: &str, color: &str, icon: &str) -> impl IntoView {
    let title = title.to_string();
    let value = value.to_string();
    let color = color.to_string();
    view! {
        <div class="glass-card p-3 md:p-4 lg:p-5" style=format!("border-top:3px solid {};", color)>
            <div class="flex justify-between items-center mb-2 md:mb-3">
                <span class="text-[10px] md:text-xs uppercase font-bold" style="color:var(--uci-muted);">{title}</span>
                <i class=format!("fa-solid {} text-base md:text-lg", icon) style="color:{};"></i>
            </div>
            <div class="text-2xl md:text-3xl lg:text-4xl font-extrabold" style="color:{color}; font-family:'JetBrains Mono',monospace; line-height:1;">{value}</div>
        </div>
    }
}