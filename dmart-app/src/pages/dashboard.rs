use leptos::prelude::*;
use leptos::either::Either;
use dmart_shared::models::*;
use crate::api;
use crate::api::{UciStatsResponse, GravedadStats, PromedioScores};
use crate::components::dashboard_kit::{DonutChart, PromedioScoresCard};
use crate::components::severity_badge::SeverityBadge;
use crate::components::chart::EvolutionChart;

#[component]
pub fn DashboardPage() -> impl IntoView {
    let stats = LocalResource::new(|| async move {
        api::get_stats().await.unwrap_or_else(|_| UciStatsResponse {
            total_pacientes: 0,
            pacientes_activos: 0,
            por_gravedad: GravedadStats { criticos: 0, severos: 0, moderados: 0, bajos: 0 },
            promedios: PromedioScores { apache_promedio: 0.0, gcs_promedio: 0.0, sofa_promedio: 0.0, saps3_promedio: 0.0, news2_promedio: 0.0 },
            reciente: vec![],
        })
    });

    let patients = LocalResource::new(|| async move {
        api::list_patients(None).await.unwrap_or_default()
    });

    let admin_stats = LocalResource::new(|| async move {
        api::get_admin_stats().await.ok()
    });

    view! {
        <div class="page-enter">
            <div class="mb-5 md:mb-7">
                <h1 class="text-xl md:text-2xl lg:text-3xl font-extrabold" style="color:var(--uci-text); margin:0 0 4px;">"Panel de Monitoreo UCI"</h1>
                <p style="color:var(--uci-muted); font-size:13px; margin:0;">"Pacientes activos, scores, recursos — vision general"</p>
            </div>

            <Suspense fallback=move || view! {
                <div style="text-align:center; padding:60px; color:var(--uci-muted);">
                    <div style="font-size:32px; margin-bottom:8px;"><i class="fa-solid fa-spinner fa-spin"></i></div>
                    "Cargando..."
                </div>
            }>
                {move || {
                    stats.get().map(|s| {
                        let pacientes = patients.get().unwrap_or_default();
                        let admin = admin_stats.get().flatten();

                        view! {
                            <div>
                                <SummaryCards stats=s.clone() />
                                <StatsSection stats=s.clone() />
                                <AdminStatsSection admin=admin.clone() />
                                <ActivePatientsSection patients=pacientes.clone() />
                                <RecentPatientsSection reciente=s.reciente.clone() />
                            </div>
                        }
                    })
                }}
            </Suspense>
        </div>
    }
}

#[component]
fn SummaryCards(stats: UciStatsResponse) -> impl IntoView {
    view! {
        <div class="grid grid-cols-2 lg:grid-cols-4 gap-3 md:gap-4 mb-6 md:mb-7">
            {stat_card("Total Pacientes", &stats.total_pacientes.to_string(), "#3B82F6", "fa-users")}
            {stat_card("Criticos", &stats.por_gravedad.criticos.to_string(), "#EF4444", "fa-skull")}
            {stat_card("Severos", &stats.por_gravedad.severos.to_string(), "#F97316", "fa-triangle-exclamation")}
            {stat_card("Estables", &format!("{}", stats.por_gravedad.moderados + stats.por_gravedad.bajos), "#10B981", "fa-check-circle")}
        </div>
    }
}

#[component]
fn StatsSection(stats: UciStatsResponse) -> impl IntoView {
    view! {
        <div class="grid grid-cols-1 md:grid-cols-2 gap-4 mb-6 md:mb-7">
            <PromedioScoresCard promedios=stats.promedios.clone() />
            <div class="p-6 rounded-xl" style="background:var(--uci-surface);">
                <h3 class="text-sm font-bold uppercase mb-4" style="color:var(--uci-text);">"Distribucion por Gravedad"</h3>
                <DonutChart data=stats.por_gravedad.clone() total=stats.total_pacientes />
            </div>
        </div>
    }
}

#[component]
fn AdminStatsSection(admin: Option<AdminStats>) -> impl IntoView {
    match admin {
        Some(a) => Either::Left(view! {
            <div class="mb-6 md:mb-7">
                <h3 class="text-sm font-bold uppercase mb-3" style="color:var(--uci-muted);">
                    <i class="fa-solid fa-cube mr-2"></i>"Recursos de la Unidad"
                </h3>
                <div class="grid grid-cols-3 md:grid-cols-6 gap-3">
                    {resource_card("Camas Totales", &a.total_camas.to_string(), "#6366F1", "fa-bed")}
                    {resource_card("Libres", &a.camas_libres.to_string(), "#10B981", "fa-bed-empty")}
                    {resource_card("Ocupadas", &a.camas_ocupadas.to_string(), "#EF4444", "fa-bed-occupied")}
                    {resource_card("Eq. Disponibles", &a.equipos_disponibles.to_string(), "#3B82F6", "fa-monitor-heart")}
                    {resource_card("Medicos", &a.medicos_activos.to_string(), "#8B5CF6", "fa-user-doctor")}
                    {resource_card("Enfermeros", &a.enfermeros_activos.to_string(), "#EC4899", "fa-user-nurse")}
                </div>
            </div>
        }),
        None => Either::Right(view! { <span></span> }),
    }
}

#[component]
fn ActivePatientsSection(patients: Vec<PatientListItem>) -> impl IntoView {
    if patients.is_empty() {
        return Either::Right(view! {
            <div style="text-align:center; padding:80px 40px; color:var(--uci-muted);">
                <div style="font-size:48px; margin-bottom:16px; opacity:0.3;"><i class="fa-solid fa-hospital"></i></div>
                <div style="font-size:20px; font-weight:600; margin-bottom:8px;" class="text-uci-text">"No hay pacientes registrados"</div>
                <p style="font-size:14px; margin:0 0 20px;">"Comience registrando el primer paciente"</p>
                <a href="/patients/new" class="btn-primary" style="display:inline-block; text-decoration:none;"><i class="fa-solid fa-plus mr-2"></i>"Nuevo Paciente"</a>
            </div>
        });
    }

    Either::Left(view! {
        <div class="mb-6 md:mb-7">
            <h3 class="text-sm font-bold uppercase mb-3" style="color:var(--uci-muted);">
                <i class="fa-solid fa-heart-pulse mr-2"></i>"Pacientes Activos"
            </h3>
            <div class="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
                {patients.iter().map(|patient| view! {
                    <PatientPokemonCard patient=patient.clone() />
                }).collect_view()}
            </div>
        </div>
    })
}

#[component]
fn RecentPatientsSection(reciente: Vec<PatientListItem>) -> impl IntoView {
    if reciente.is_empty() {
        return Either::Right(view! { <span></span> });
    }

    let mostrar = if reciente.len() > 10 { &reciente[..10] } else { &reciente[..] };

    let content = view! {
        <div class="mb-6 md:mb-7">
            <h3 class="text-sm font-bold uppercase mb-3" style="color:var(--uci-muted);">
                <i class="fa-solid fa-clock mr-2"></i>{"Pacientes Recientes ("}{mostrar.len()}{")"}
            </h3>
            <div class="rounded-xl overflow-hidden" style="background:var(--uci-surface); border:1px solid var(--uci-border);">
                <table class="w-full">
                    <thead style="background:var(--uci-bg);">
                        <tr>
                            <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Paciente"</th>
                            <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Cedula"</th>
                            <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Edad"</th>
                            <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"APACHE"</th>
                            <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Gravedad"</th>
                            <th class="px-4 py-3 text-right text-sm font-medium" style="color:var(--uci-muted);">"Accion"</th>
                        </tr>
                    </thead>
                    <tbody class="divide-y" style="border-color:var(--uci-border);">
                        {mostrar.iter().map(|p| {
                            let pid = p.id.clone();
                            view! {
                                <tr class="hover:bg-black/5">
                                    <td class="px-4 py-3 text-sm font-medium" style="color:var(--uci-text);">{p.nombre_completo.clone()}</td>
                                    <td class="px-4 py-3 text-sm font-mono" style="color:var(--uci-muted);">{p.cedula.clone()}</td>
                                    <td class="px-4 py-3 text-sm" style="color:var(--uci-muted);">{format!("{} anios", p.edad)}</td>
                                    <td class="px-4 py-3 text-sm" style="color:var(--uci-muted);">{p.ultimo_apache_score.map(|s| s.to_string()).unwrap_or_else(|| "-".into())}</td>
                                    <td class="px-4 py-3"><SeverityBadge level=p.estado_gravedad.clone() /></td>
                                    <td class="px-4 py-3 text-right">
                                        <a href=format!("/patients/{}", pid)
                                            class="text-xs px-3 py-1.5 rounded-lg font-semibold"
                                            style="background:var(--uci-accent); color:white; text-decoration:none;">
                                            <i class="fa-solid fa-address-card mr-1"></i>"Ver"
                                        </a>
                                    </td>
                                </tr>
                            }
                        }).collect_view()}
                    </tbody>
                </table>
            </div>
        </div>
    };
    Either::Left(content)
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

            <div class="p-4 flex items-center justify-between" style="background:linear-gradient(135deg,rgba(30,41,59,0.05),transparent); border-bottom:1px solid var(--uci-border);">
                <div class="flex items-center gap-3">
                    <div class="w-12 h-12 rounded-full flex items-center justify-center text-lg font-bold" style=format!("background:{}; color:white;", severity_config.1)>
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

            <div class="px-4 pb-3 flex flex-wrap gap-2">
                <InfoBadge icon="fa-syringe" value=patient.ultimo_saps3_score.map(|s| s.to_string()).unwrap_or_else(|| "-".into()) color="#3B82F6".to_string() />
                <InfoBadge icon="fa-skull" value={patient.mortality_risk.map(|m| format!("{:.0}%", m)).unwrap_or_else(|| "-".into())} color="#EF4444".to_string() />
                <InfoBadge icon="fa-user" value={format!("{} anios", patient.edad)} color="#64748B".to_string() />
                <InfoBadge icon=sex_icon.0 value="".to_string() color=sex_icon.1.to_string() />
            </div>

            <Suspense fallback=move || view! {
                <div style="height:50px; background:var(--uci-surface); display:flex; align-items:center; justify-content:center; color:var(--uci-muted); font-size:11px;">
                    <i class="fa-solid fa-spinner fa-spin mr-2"></i>"Cargando..."
                </div>
            }>
                {move || measurements.get().map(|ms| {
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
            <div class="w-14 h-14 rounded-full flex items-center justify-center" style=format!("background:{}; color:white; border:2px solid {};", color, color)>
                <i class=format!("fa-solid {}", icon)></i>
            </div>
            <div class="text-lg font-bold mt-1" style=format!("color:{}", color)>{value}</div>
            <div class="text-[10px] uppercase font-semibold" style="color:var(--uci-muted);">{label}</div>
        </div>
    }
}

#[component]
fn InfoBadge(icon: &'static str, value: String, color: String) -> impl IntoView {
    view! {
        <div class="flex items-center gap-1 px-2 py-1 rounded-lg text-xs font-semibold" style=format!("background:{}; color:white;", color)>
            <i class=format!("fa-solid {}", icon)></i>
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
                <i class=format!("fa-solid {} text-base md:text-lg", icon) style=format!("color:{};", color)></i>
            </div>
            <div class="text-2xl md:text-3xl lg:text-4xl font-extrabold" style=format!("color:{}; font-family:'JetBrains Mono',monospace; line-height:1;", color)>{value}</div>
        </div>
    }
}

fn resource_card(title: &str, value: &str, color: &str, icon: &str) -> impl IntoView {
    let title = title.to_string();
    let value = value.to_string();
    let color = color.to_string();
    view! {
        <div class="glass-card p-3" style=format!("border-top:2px solid {}; border-radius:12px;", color)>
            <div class="flex items-center gap-2 mb-1">
                <i class=format!("fa-solid {} text-sm", icon) style=format!("color:{};", color)></i>
                <span class="text-[10px] uppercase font-bold" style="color:var(--uci-muted);">{title}</span>
            </div>
            <div class="text-xl md:text-2xl font-extrabold" style=format!("color:{}; font-family:'JetBrains Mono',monospace; line-height:1;", color)>{value}</div>
        </div>
    }
}
