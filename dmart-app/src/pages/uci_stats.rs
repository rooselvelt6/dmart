use leptos::prelude::*;

#[component]
pub fn UciStats() -> impl IntoView {
    view! {
        <div class="glass-card" style="padding:20px;">
            <h2 class="text-lg font-bold text-uci-text mb-4">Estadísticas UCI</h2>
            <StatsGrid />
        </div>
    }
}

#[component]
pub fn StatsGrid() -> impl IntoView {
    view! {
        <div class="grid grid-cols-4 gap-3">
            <StatBox label="Total" value="0" />
            <StatBox label="Críticos" value="0" />
            <StatBox label="Severos" value="0" />
            <StatBox label="Estables" value="0" />
        </div>
    }
}

#[component]
pub fn StatBox(label: &'static str, value: &'static str) -> impl IntoView {
    view! {
        <div class="glass-card p-3 text-center">
            <div class="text-xs text-uci-muted uppercase mb-1">{label}</div>
            <div class="text-xl font-bold text-uci-accent">{value}</div>
        </div>
    }
}
