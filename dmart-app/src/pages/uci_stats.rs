use leptos::prelude::*;
use crate::api;
use crate::api::{UciStatsResponse, GravedadStats, PromedioScores};

#[component]
pub fn UciStats() -> impl IntoView {
    let stats_resource = LocalResource::new(|| {
        async move {
            api::get_stats().await.unwrap_or_else(|_| UciStatsResponse {
                total_pacientes: 0,
                pacientes_activos: 0,
                por_gravedad: GravedadStats {
                    criticos: 0,
                    severos: 0,
                    moderados: 0,
                    bajos: 0,
                },
                promedios: PromedioScores {
                    apache_promedio: 0.0,
                    gcs_promedio: 0.0,
                    sofa_promedio: 0.0,
                    saps3_promedio: 0.0,
                    news2_promedio: 0.0,
                },
                reciente: vec![],
            })
        }
    });

    view! {
        <div class="page-enter">
            <div class="mb-6">
                <h1 class="text-2xl font-bold" style="color:var(--uci-text);">Estadisticas UCI</h1>
            </div>

            <Suspense fallback=move || view! { <div class="p-10 text-center rounded-lg" style="background:var(--uci-surface);">Cargando...</div> }>
                {move || stats_resource.get().map(|stats| {
                    view! {
                        <StatsContent stats=stats />
                    }
                })}
            </Suspense>
        </div>
    }
}

#[component]
pub fn StatsContent(stats: UciStatsResponse) -> impl IntoView {
    view! {
        <div class="space-y-6">
            <div class="grid grid-cols-2 md:grid-cols-4 gap-4">
                <StatCard title="Total Pacientes" value=stats.total_pacientes.to_string() _color="#3B82F6" />
                <StatCard title="Criticos" value=stats.por_gravedad.criticos.to_string() _color="#EF4444" />
                <StatCard title="Severos" value=stats.por_gravedad.severos.to_string() _color="#F97316" />
                <StatCard title="Estables" value=(stats.por_gravedad.moderados + stats.por_gravedad.bajos).to_string() _color="#10B981" />
            </div>

            <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
                <div class="p-6 rounded-xl" style="background:var(--uci-surface);">
                    <h3 class="text-sm font-bold uppercase mb-4" style="color:var(--uci-text);">Distribucion por Gravedad</h3>
                    <DonutChart data=stats.por_gravedad total=stats.total_pacientes />
                </div>

                <div class="p-6 rounded-xl" style="background:var(--uci-surface);">
                    <h3 class="text-sm font-bold uppercase mb-4" style="color:var(--uci-text);">Promedio de Scores</h3>
                    <div class="space-y-4">
                        <ScoreBar label="APACHE II" value=stats.promedios.apache_promedio max=71.0 />
                        <ScoreBar label="GCS" value=stats.promedios.gcs_promedio max=15.0 />
                        <ScoreBar label="SOFA" value=stats.promedios.sofa_promedio max=24.0 />
                        <ScoreBar label="SAPS III" value=stats.promedios.saps3_promedio max=104.0 />
                        <ScoreBar label="NEWS2" value=stats.promedios.news2_promedio max=20.0 />
                    </div>
                </div>
            </div>

            <div class="p-6 rounded-xl" style="background:var(--uci-surface);">
                <h3 class="text-sm font-bold uppercase mb-4" style="color:var(--uci-text);">{"Pacientes Recientes ("}{stats.reciente.len()}{")"}</h3>
                <div class="space-y-2">
                    {stats.reciente.iter().map(|p| {
                        let nivel = format!("{:?}", p.estado_gravedad);
                        view! {
                            <div class="p-3 rounded-lg" style="background:var(--uci-bg);">
                                <div class="font-semibold" style="color:var(--uci-text);">{p.nombre_completo.clone()}</div>
                                <div class="text-sm" style="color:var(--uci-muted);">{nivel}</div>
                            </div>
                        }
                    }).collect::<Vec<_>>()}
                </div>
            </div>
        </div>
    }
}

#[component]
pub fn StatCard(title: &'static str, value: String, _color: &'static str) -> impl IntoView {
    view! {
        <div class="p-4 text-center rounded-xl shadow-md" style="background:var(--uci-surface);">
            <div class="text-3xl font-black" style="color:var(--uci-text);">{value}</div>
            <div class="text-xs font-bold uppercase mt-1" style="color:var(--uci-muted);">{title}</div>
        </div>
    }
}

#[component]
pub fn DonutChart(data: GravedadStats, total: usize) -> impl IntoView {
    view! {
        <div class="text-center">
            <div class="text-4xl font-black mb-4" style="color:var(--uci-text);">{total}</div>
            <div class="text-sm mb-4" style="color:var(--uci-muted);">Total</div>
            <div class="space-y-2 text-left">
                <div class="flex justify-between">
                    <span style="color:#EF4444;">Criticos</span>
                    <span style="color:var(--uci-text);">{data.criticos}</span>
                </div>
                <div class="flex justify-between">
                    <span style="color:#F97316;">Severos</span>
                    <span style="color:var(--uci-text);">{data.severos}</span>
                </div>
                <div class="flex justify-between">
                    <span style="color:#F59E0B;">Moderados</span>
                    <span style="color:var(--uci-text);">{data.moderados}</span>
                </div>
                <div class="flex justify-between">
                    <span style="color:#10B981;">Estables</span>
                    <span style="color:var(--uci-text);">{data.bajos}</span>
                </div>
            </div>
        </div>
    }
}

#[component]
pub fn ScoreBar(label: &'static str, value: f32, max: f32) -> impl IntoView {
    let pct = if max > 0.0 { (value / max * 100.0).min(100.0) } else { 0.0 };
    let color = if pct >= 70.0 { "#EF4444" } else if pct >= 40.0 { "#F97316" } else { "#10B981" };

    view! {
        <div>
            <div class="flex justify-between text-sm">
                <span style="color:var(--uci-text);">{label}</span>
                <span style="color:var(--uci-muted);">{format!("{:.1}", value)}</span>
            </div>
            <div class="h-2 mt-1 rounded-full" style="background:var(--uci-border);">
                <div class="h-full rounded-full transition-all duration-500" style=format!("width:{}%; background:{};", pct, color)></div>
            </div>
        </div>
    }
}