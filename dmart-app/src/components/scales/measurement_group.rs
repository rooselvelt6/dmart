use leptos::prelude::*;
use dmart_shared::models::Measurement;

#[derive(Clone)]
struct RowData {
    fecha: String,
    score: String,
    notas: String,
}

#[component]
pub fn MeasurementGroup(
    title: String,
    icon: String,
    color: String,
    measurements: Vec<Measurement>,
) -> impl IntoView {
    let rows: Vec<RowData> = measurements.iter().map(|m| {
        let title_inner = title.clone();
        let ts = m.timestamp.clone();
        let fecha = if ts.len() >= 16 { ts[..16].to_string() } else { ts };
        let score = if title_inner.contains("APACHE") {
            m.apache_score.to_string()
        } else if title_inner.contains("GCS") {
            m.gcs_score.to_string()
        } else if title_inner.contains("SOFA") {
            m.sofa_score.map(|s| s.to_string()).unwrap_or_else(|| "—".to_string())
        } else if title_inner.contains("SAPS") {
            m.saps3_score.map(|s| s.to_string()).unwrap_or_else(|| "—".to_string())
        } else if title_inner.contains("NEWS") {
            m.news2_score.map(|s| s.to_string()).unwrap_or_else(|| "—".to_string())
        } else {
            "—".to_string()
        };
        let notas = m.notas.clone();
        RowData { fecha, score, notas }
    }).collect();

    let count = rows.len();
    let color_clone = color.clone();

    view! {
        <div class="mb-6 p-4 rounded-xl" style="background:var(--uci-surface); border:1px solid var(--uci-border);">
            <div class="flex items-center gap-3 mb-4">
                <i class={format!("fa-solid {} text-lg", icon)} style={format!("color:{};", color)}></i>
                <h4 class="font-bold text-uci-text">{title}</h4>
                <span class="ml-auto text-xs px-2 py-1 rounded-full" style={format!("background:{}; color:white;", color)}>{count}</span>
            </div>
            <div class="overflow-x-auto">
                <table class="w-full text-sm">
                    <thead>
                        <tr style="border-bottom:1px solid var(--uci-border);">
                            <th class="text-left py-2 px-2 font-medium" style="color:var(--uci-muted);">"Fecha"</th>
                            <th class="text-center py-2 px-2 font-medium" style="color:var(--uci-muted);">"Score"</th>
                            <th class="text-left py-2 px-2 font-medium" style="color:var(--uci-muted);">"Notas"</th>
                        </tr>
                    </thead>
                    <tbody>
                        {rows.into_iter().map(move |r| {
                            let notas = if r.notas.is_empty() { "—".to_string() } else { r.notas.clone() };
                            view! {
                                <tr class="border-b last:border-0" style="border-color:var(--uci-border);">
                                    <td class="py-2 px-2 text-uci-text whitespace-nowrap">{r.fecha}</td>
                                    <td class="py-2 px-2 text-center">
                                        <span class="font-bold" style={format!("color:{};", color_clone)}>{r.score}</span>
                                    </td>
                                    <td class="py-2 px-2 text-uci-muted truncate max-w-xs">{notas}</td>
                                </tr>
                            }
                        }).collect_view()}
                    </tbody>
                </table>
            </div>
        </div>
    }
}