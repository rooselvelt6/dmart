use crate::api::{GravedadStats, PromedioScores};
use leptos::prelude::*;

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

#[component]
pub fn PromedioScoresCard(promedios: PromedioScores) -> impl IntoView {
    view! {
        <div class="p-6 rounded-xl" style="background:var(--uci-surface);">
            <h3 class="text-sm font-bold uppercase mb-4" style="color:var(--uci-text);">Promedio de Scores</h3>
            <div class="space-y-4">
                <ScoreBar label="APACHE II" value=promedios.apache_promedio max=71.0 />
                <ScoreBar label="GCS" value=promedios.gcs_promedio max=15.0 />
                <ScoreBar label="SOFA" value=promedios.sofa_promedio max=24.0 />
                <ScoreBar label="SAPS III" value=promedios.saps3_promedio max=104.0 />
                <ScoreBar label="NEWS2" value=promedios.news2_promedio max=20.0 />
            </div>
        </div>
    }
}
