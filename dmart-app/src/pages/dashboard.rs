use leptos::prelude::*;
use leptos::either::Either;
use leptos_router::hooks::*;
use dmart_shared::models::*;
use crate::api;
use crate::components::{severity_badge::SeverityBadge, chart::EvolutionChart};

#[component]
pub fn DashboardPage() -> impl IntoView {
    let patients = LocalResource::new(|| async move {
        api::list_patients(None).await.unwrap_or_default()
    });

    let measurements_cache: Vec<(String, Vec<Measurement>)> = vec![];

    view! {
        <div class="page-enter">
            // Header
            <div style="margin-bottom:28px;">
                <h1 style="font-size:26px; font-weight:800; color:#E2E8F0; margin:0 0 4px;">"Panel de Monitoreo"</h1>
                <p style="color:#475569; font-size:14px; margin:0;">"Pacientes activos en UCI — visión general y evolución"</p>
            </div>

            <Suspense fallback=move || view! {
                <div style="text-align:center; padding:60px; color:#475569;">
                    <div style="font-size:32px; margin-bottom:8px;">"⌛"</div>
                    "Cargando pacientes..."
                </div>
            }>
                {move || patients.get().map(|list_wrapper| {
                    let list = &*list_wrapper;
                    // Stats row
                    let total = list.len();
                    let criticos = list.iter().filter(|p| matches!(p.estado_gravedad, SeverityLevel::Critico)).count();
                    let severos  = list.iter().filter(|p| matches!(p.estado_gravedad, SeverityLevel::Severo)).count();
                    let moderados= list.iter().filter(|p| matches!(p.estado_gravedad, SeverityLevel::Moderado)).count();
                    let bajos    = list.iter().filter(|p| matches!(p.estado_gravedad, SeverityLevel::Bajo)).count();

                    view! {
                        <div>
                            // Stats grid
                            <div style="display:grid; grid-template-columns:repeat(4,1fr); gap:16px; margin-bottom:28px;">
                                {stat_card("Total Pacientes", &total.to_string(), "#3B82F6", "👥")}
                                {stat_card("Críticos", &criticos.to_string(), "#EF4444", "🚨")}
                                {stat_card("Severos", &severos.to_string(), "#F97316", "⚠️")}
                                {stat_card("Mod. / Bajos", &format!("{} / {}", moderados, bajos), "#10B981", "✅")}
                            </div>

                            // Patient monitoring cards
                            <div style="display:grid; grid-template-columns:repeat(auto-fill,minmax(380px,1fr)); gap:20px;">
                                {list.into_iter().map(|patient| view! {
                                    <PatientMonitorCard patient=patient.clone() />
                                }).collect_view()}
                            </div>

                            // Empty state
                            {if total == 0 {
                                Either::Left(view! {
                                    <div style="text-align:center; padding:80px 40px; color:#475569;">
                                        <div style="font-size:64px; margin-bottom:16px; opacity:0.5;">"🏥"</div>
                                        <div style="font-size:20px; font-weight:600; color:#94A3B8; margin-bottom:8px;">"No hay pacientes registrados"</div>
                                        <p style="font-size:14px; margin:0 0 20px;">"Comience registrando el primer paciente"</p>
                                        <a href="/patients/new" class="btn-primary" style="display:inline-block; text-decoration:none;">"+ Nuevo Paciente"</a>
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
fn PatientMonitorCard(patient: PatientListItem) -> impl IntoView {
    let id = patient.id.clone();
    let id_for_chart = patient.id.clone();

    // Load measurements for the chart
    let measurements = LocalResource::new(move || {
        let pid = id_for_chart.clone();
        async move {
            api::get_measurements(&pid).await.unwrap_or_default()
        }
    });

    let severity_color = severity_color_hex(&patient.estado_gravedad);
    let severity_glow  = severity_glow(&patient.estado_gravedad);

    view! {
        <div class="glass-card"
             style=format!("padding:20px; border-left:3px solid {}; transition:all 0.3s;", severity_color)
             onmouseenter="this.style.transform='translateY(-2px)'; this.style.boxShadow='0 8px 32px rgba(0,0,0,0.3)'"
             onmouseleave="this.style.transform=''; this.style.boxShadow=''">

            // Header row: name + severity
            <div style="display:flex; justify-content:space-between; align-items:flex-start; margin-bottom:14px;">
                <div>
                    <div style="font-weight:700; font-size:16px; color:#E2E8F0; margin-bottom:2px;">
                        {patient.nombre_completo.clone()}
                    </div>
                    <div style="font-size:12px; color:#64748B;">
                        "HC: " {patient.historia_clinica.clone()} " · CI: " {patient.cedula.clone()}
                    </div>
                </div>
                <SeverityBadge level=patient.estado_gravedad.clone() />
            </div>

            // Scores row
            <div style="display:flex; gap:12px; margin-bottom:14px;">
                <div style=format!("flex:1; background:rgba(10,14,26,0.6); border-radius:10px; padding:10px 12px; border:1px solid {};", severity_color)>
                    <div style="font-size:10px; color:#64748B; text-transform:uppercase; letter-spacing:0.5px; margin-bottom:2px;">"Apache II"</div>
                    <div style=format!("font-size:28px; font-weight:800; color:{}; font-family:'JetBrains Mono',monospace; line-height:1;", severity_color)>
                        {patient.ultimo_apache_score.map(|s| s.to_string()).unwrap_or_else(|| "—".into())}
                    </div>
                </div>
                <div style="flex:1; background:rgba(10,14,26,0.6); border-radius:10px; padding:10px 12px; border:1px solid #2A3547;">
                    <div style="font-size:10px; color:#64748B; text-transform:uppercase; letter-spacing:0.5px; margin-bottom:2px;">"Glasgow (GCS)"</div>
                    <div style="font-size:28px; font-weight:800; color:#3B82F6; font-family:'JetBrains Mono',monospace; line-height:1;">
                        {patient.ultimo_gcs_score.map(|s| s.to_string()).unwrap_or_else(|| "—".into())}
                        <span style="font-size:14px; color:#475569; font-weight:400;">"/15"</span>
                    </div>
                </div>
                <div style="flex:1; background:rgba(10,14,26,0.6); border-radius:10px; padding:10px 12px; border:1px solid #2A3547;">
                    <div style="font-size:10px; color:#64748B; text-transform:uppercase; letter-spacing:0.5px; margin-bottom:2px;">"Edad / Sexo"</div>
                    <div style="font-size:18px; font-weight:700; color:#94A3B8; line-height:1; margin-bottom:2px;">
                        {patient.edad} " años"
                    </div>
                    <div style="font-size:12px; color:#475569;">{format!("{:?}", patient.sexo)}</div>
                </div>
            </div>

            // Evolution chart (mini)
            <Suspense fallback=move || view! {
                <div style="height:60px; background:rgba(10,14,26,0.4); border-radius:8px; display:flex; align-items:center; justify-content:center; color:#334155; font-size:12px;">"Cargando gráfica..."</div>
            }>
                {move || measurements.get().map(|ms_wrapper| {
                    let ms = &*ms_wrapper;
                    if ms.is_empty() {
                        Either::Left(view! {
                            <div style="height:64px; background:rgba(10,14,26,0.4); border-radius:8px; display:flex; align-items:center; justify-content:center; gap:6px; color:#334155; font-size:12px;">
                                "Sin mediciones registradas"
                            </div>
                        })
                    } else {
                        Either::Right(view! {
                            <EvolutionChart measurements=ms.clone() height=64 compact=true />
                        })
                    }
                })}
            </Suspense>

            // Actions
            <div style="display:flex; gap:8px; margin-top:14px;">
                <a href=format!("/patients/{}", id.clone())
                   style="flex:1; display:block; text-align:center; padding:8px; border-radius:8px; font-size:13px; font-weight:600; background:rgba(59,130,246,0.1); color:#3B82F6; border:1px solid rgba(59,130,246,0.2); text-decoration:none; transition:all 0.2s;"
                   onmouseenter="this.style.background='rgba(59,130,246,0.2)'"
                   onmouseleave="this.style.background='rgba(59,130,246,0.1)'">"Ver Detalle"</a>
                <a href=format!("/patients/{}/measure", id.clone())
                   style="flex:1; display:block; text-align:center; padding:8px; border-radius:8px; font-size:13px; font-weight:600; background:rgba(16,185,129,0.1); color:#10B981; border:1px solid rgba(16,185,129,0.2); text-decoration:none; transition:all 0.2s;"
                   onmouseenter="this.style.background='rgba(16,185,129,0.2)'"
                   onmouseleave="this.style.background='rgba(16,185,129,0.1)'">"Nueva Medición"</a>
            </div>
        </div>
    }
}

fn stat_card(title: &str, value: &str, color: &str, icon: &str) -> impl IntoView {
    let title = title.to_string();
    let value = value.to_string();
    let color = color.to_string();
    let icon = icon.to_string();
    view! {
        <div class="glass-card" style=format!("padding:20px; border-top:3px solid {};", color)>
            <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:8px;">
                <span style="font-size:10px; color:#64748B; text-transform:uppercase; letter-spacing:1px; font-weight:600;">{title}</span>
                <span style="font-size:20px;">{icon}</span>
            </div>
            <div style=format!("font-size:36px; font-weight:800; color:{}; font-family:'JetBrains Mono',monospace; line-height:1;", color)>{value}</div>
        </div>
    }
}

pub fn severity_color_hex(level: &SeverityLevel) -> String {
    match level {
        SeverityLevel::Bajo     => "#10B981".to_string(),
        SeverityLevel::Moderado => "#F59E0B".to_string(),
        SeverityLevel::Severo   => "#F97316".to_string(),
        SeverityLevel::Critico  => "#EF4444".to_string(),
    }
}

fn severity_glow(level: &SeverityLevel) -> String {
    match level {
        SeverityLevel::Bajo     => "0 0 12px rgba(16,185,129,0.4)".to_string(),
        SeverityLevel::Moderado => "0 0 12px rgba(245,158,11,0.4)".to_string(),
        SeverityLevel::Severo   => "0 0 12px rgba(249,115,22,0.4)".to_string(),
        SeverityLevel::Critico  => "0 0 12px rgba(239,68,68,0.4)".to_string(),
    }
}
