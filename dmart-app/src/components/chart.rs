use dmart_shared::models::Measurement;
use leptos::either::Either;
use leptos::prelude::*;

#[component]
pub fn EvolutionChart(
    measurements: Vec<Measurement>,
    #[prop(optional, default = 200)] height: i32,
    #[prop(optional, default = false)] compact: bool,
) -> impl IntoView {
    let width = 600;
    let padding = if compact { 5 } else { 40 };

    // Sort measurements by date
    let mut sorted = measurements.clone();
    sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

    let max_apache = 71.0;

    let get_x = move |index: usize, total: usize| {
        if total <= 1 {
            return (width / 2) as f32;
        }
        let available_width = (width - padding * 2) as f32;
        padding as f32 + (index as f32 * (available_width / (total - 1) as f32))
    };

    let get_y = move |value: u32| {
        let available_height = (height - padding * 2) as f32;
        (height as f32 - padding as f32) - ((value as f32 / max_apache) * available_height)
    };

    let points = {
        let total = sorted.len();
        sorted
            .iter()
            .enumerate()
            .map(|(i, m)| (get_x(i, total), get_y(m.apache_score)))
            .collect::<Vec<_>>()
    };

    let polyline_points = points
        .iter()
        .map(|(x, y)| format!("{},{}", x, y))
        .collect::<Vec<_>>()
        .join(" ");

    view! {
        <div class="chart-container">
            <svg
                viewBox=format!("0 0 {} {}", width, height)
                class="chart-svg w-full h-auto"
                preserveAspectRatio="xMidYMid meet"
            >
                // Threshold lines (Apache II severity zones)
                {move || if !compact {
                    Either::Left(view! {
                        <g opacity="0.1">
                            <line x1=padding y1={get_y(10)} x2={width-padding} y2={get_y(10)} stroke="#F59E0B" stroke-dasharray="4" />
                            <line x1=padding y1={get_y(20)} x2={width-padding} y2={get_y(20)} stroke="#F97316" stroke-dasharray="4" />
                            <line x1=padding y1={get_y(30)} x2={width-padding} y2={get_y(30)} stroke="#EF4444" stroke-dasharray="4" />
                        </g>
                    })
                } else { Either::Right(()) }}

                // The line
                <polyline
                    fill="none"
                    stroke="#3B82F6"
                    stroke-width=if compact { "2" } else { "3" }
                    points=polyline_points
                    stroke-linejoin="round"
                />

                // Data points
                {points.into_iter().enumerate().map(|(i, (x, y))| {
                    let m = &sorted[i];
                    let color = match m.apache_score {
                        0..=9 => "#10B981",
                        10..=19 => "#F59E0B",
                        20..=29 => "#F97316",
                        _ => "#EF4444",
                    };

                    view! {
                        <circle
                            cx=x cy=y
                            r=if compact { "3" } else { "5" }
                            fill=color
                            stroke="#1E2537"
                            stroke-width="2"
                        >
                            <title>{format!("Score: {} ({})", m.apache_score, m.timestamp)}</title>
                        </circle>
                    }
                }).collect_view()}
            </svg>
        </div>
    }
}
