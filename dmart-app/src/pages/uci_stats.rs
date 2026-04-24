use leptos::prelude::*;
use leptos::either::Either;
use crate::api::{self, UciStatsResponse, PromedioScores, GravedadStats};
use crate::components::severity_badge::SeverityBadge;

#[component]
pub fn UciStats() -> impl IntoView {
    let stats = LocalResource::new(|| async move {
        api::get_stats().await
    });

    view! {
        <div class="page-enter">
            <div class="mb-6">
                <h1 class="text-2xl md:text-3xl font-black text-uci-text">"Estadísticas UCI"</h1>
                <p class="text-sm text-uci-muted">"Métricas en tiempo real de la Unidad de Cuidados Intensivos"</p>
            </div>

            <Suspense fallback=move || view! {
                <div class="glass-card p-10 text-center">
                    <i class="fa-solid fa-spinner fa-spin text-3xl text-uci-accent mb-4"></i>
                    <p class="text-uci-muted">"Cargando estadísticas..."</p>
                </div>
            }>
                {move || stats.get().map(|res| match &*res {
                    Ok(s) => Either::Left(view! { <StatsDashboard stats=s.clone() /> }),
                    Err(e) => Either::Right(view! {
                        <div class="glass-card p-10 text-center text-uci-critical">
                            <i class="fa-solid fa-triangle-exclamation text-3xl mb-4"></i>
                            <p>{format!("Error: {}", e)}</p>
                        </div>
                    }),
                })}
            </Suspense>
        </div>
    }
}

#[component]
pub fn StatsDashboard(stats: UciStatsResponse) -> impl IntoView {
    let criticos = stats.por_gravedad.criticos;
    let severos = stats.por_gravedad.severos;
    let moderados = stats.por_gravedad.moderados;
    let bajos = stats.por_gravedad.bajos;
    let estables = moderados + bajos;
    let reciente = stats.reciente.clone();
    let promedios = stats.promedios.clone();

    view! {
        <div class="space-y-6">
            <div class="grid grid-cols-2 md:grid-cols-4 gap-4">
                <StatCard title="Total Pacientes" value={stats.total_pacientes.to_string()} icon="fa-users" color="#3B82F6" />
                <StatCard title="Críticos" value={criticos.to_string()} icon="fa-skull" color="#EF4444" />
                <StatCard title="Severos" value={severos.to_string()} icon="fa-triangle-exclamation" color="#F97316" />
                <StatCard title="Estables" value={estables.to_string()} icon="fa-check-circle" color="#10B981" />
            </div>

            <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
                <div class="glass-card p-6">
                    <h3 class="text-sm font-bold text-uci-muted uppercase tracking-widest mb-4 flex items-center gap-2">
                        <i class="fa-solid fa-chart-pie text-uci-accent"></i>
                        "Distribución por Gravedad"
                    </h3>
                    <DonutChart criticos=criticos severos=severos moderados=moderados bajos=bajos />
                </div>

                <div class="glass-card p-6">
                    <h3 class="text-sm font-bold text-uci-muted uppercase tracking-widest mb-4 flex items-center gap-2">
                        <i class="fa-solid fa-scale-balanced text-uci-accent"></i>
                        "Promedio de Scores"
                    </h3>
                    <div class="space-y-4">
                        <ScoreBar label="APACHE II" value=promedios.apache_promedio max=71.0 />
                        <ScoreBar label="GCS" value=promedios.gcs_promedio max=15.0 />
                        <ScoreBar label="SOFA" value=promedios.sofa_promedio max=24.0 />
                        <ScoreBar label="SAPS III" value=promedios.saps3_promedio max=104.0 />
                        <ScoreBar label="NEWS2" value=promedios.news2_promedio max=64.0 />
                    </div>
                </div>
            </div>

            <div class="glass-card overflow-hidden">
                <div class="px-6 py-4 border-b border-uci-border flex items-center justify-between">
                    <h3 class="text-sm font-bold text-uci-muted uppercase tracking-widest flex items-center gap-2">
                        <i class="fa-solid fa-users text-uci-accent"></i>
                        "Pacientes Recientes"
                    </h3>
                    <span class="text-xs text-uci-muted">{reciente.len()} registros</span>
                </div>
                <div class="overflow-x-auto">
                    <table class="w-full text-left border-collapse">
                        <thead class="bg-uci-surface/50 text-[11px] font-bold text-uci-muted uppercase tracking-wider">
                            <tr>
                                <th class="px-5 py-3 border-b border-uci-border">"Paciente"</th>
                                <th class="px-5 py-3 border-b border-uci-border">"Cédula"</th>
                                <th class="px-5 py-3 border-b border-uci-border">"Gravedad"</th>
                                <th class="px-5 py-3 border-b border-uci-border">"Ingreso UCI"</th>
                            </tr>
                        </thead>
                        <tbody class="text-sm">
                            {reciente.into_iter().map(|p| {
                                view! {
                                    <tr class="hover:bg-uci-accent/5 border-b border-uci-border transition-colors">
                                        <td class="px-5 py-3 font-semibold text-uci-text">{p.nombre_completo}</td>
                                        <td class="px-5 py-3 font-mono text-xs text-uci-muted">{p.cedula}</td>
                                        <td class="px-5 py-3"><SeverityBadge level=p.estado_gravedad /></td>
                                        <td class="px-5 py-3 text-xs text-uci-muted">{p.fecha_ingreso_uci[..10].to_string()}</td>
                                    </tr>
                                }
                            }).collect_view()}
                        </tbody>
                    </table>
                </div>
            </div>
        </div>
    }
}

#[component]
pub fn StatCard(title: &'static str, value: String, icon: &'static str, color: &'static str) -> impl IntoView {
    view! {
        <div class="glass-card p-4 md:p-5 text-center" style=format!("border-top:3px solid {};", color)>
            <i class={format!("fa-solid {} text-xl mb-2", icon)} style=format!("color:{}", color)></i>
            <div class="text-2xl md:text-3xl font-black text-uci-text">{value}</div>
            <div class="text-xs font-bold text-uci-muted uppercase tracking-widest">{title}</div>
        </div>
    }
}

#[component]
pub fn DonutChart(criticos: usize, severos: usize, moderados: usize, bajos: usize) -> impl IntoView {
    let total = criticos + severos + moderados + bajos;
    
    view! {
        <div class="flex items-center justify-center">
            <div class="relative w-40 h-40 flex items-center justify-center">
                <svg viewBox="0 0 100 100" class="w-full h-full -rotate-90">
                    <circle cx="50" cy="50" r="40" fill="none" stroke="var(--uci-border)" stroke-width="12" />
                </svg>
                <div class="absolute inset-0 flex items-center justify-center">
                    <span class="text-xl font-black text-uci-text">{total}</span>
                </div>
            </div>
            <div class="ml-6 space-y-2">
                <div class="flex items-center gap-2 text-sm">
                    <span class="w-3 h-3 rounded-full bg-red-500"></span>
                    <span class="text-uci-text">{"Críticos: ".to_string() + &criticos.to_string()}</span>
                </div>
                <div class="flex items-center gap-2 text-sm">
                    <span class="w-3 h-3 rounded-full bg-orange-500"></span>
                    <span class="text-uci-text">{"Severos: ".to_string() + &severos.to_string()}</span>
                </div>
                <div class="flex items-center gap-2 text-sm">
                    <span class="w-3 h-3 rounded-full bg-amber-500"></span>
                    <span class="text-uci-text">{"Moderados: ".to_string() + &moderados.to_string()}</span>
                </div>
                <div class="flex items-center gap-2 text-sm">
                    <span class="w-3 h-3 rounded-full bg-emerald-500"></span>
                    <span class="text-uci-text">{"Estables: ".to_string() + &bajos.to_string()}</span>
                </div>
            </div>
        </div>
    }
}

#[component]
pub fn ScoreBar(label: &'static str, value: f32, max: f32) -> impl IntoView {
    let pct = (value / max * 100.0).min(100.0);
    let color = if pct >= 70.0 { "#EF4444" } else if pct >= 40.0 { "#F97316" } else { "#10B981" };
    
    view! {
        <div class="space-y-1">
            <div class="flex justify-between text-xs">
                <span class="font-semibold text-uci-text">{label}</span>
                <span class="text-uci-muted">{format!("{:.1}", value)}</span>
            </div>
            <div class="h-2 bg-uci-surface rounded-full overflow-hidden">
                <div class="h-full rounded-full transition-all" style=format!("width:{}%; background:{}", pct, color)></div>
            </div>
        </div>
    }
}