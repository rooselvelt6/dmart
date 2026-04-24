use dmart_shared::models::*;
use leptos::prelude::*;

#[derive(Clone, PartialEq)]
pub struct RadarData {
    pub label: &'static str,
    pub value: f32,
    pub max: f32,
    pub warning: f32,
    pub critical: f32,
}

impl RadarData {
    pub fn from_patient(m: &Measurement) -> Vec<RadarData> {
        vec![
            RadarData {
                label: "APACHE II",
                value: m.apache_score as f32,
                max: 71.0,
                warning: 20.0,
                critical: 30.0,
            },
            RadarData {
                label: "GCS",
                value: m.gcs_score as f32,
                max: 15.0,
                warning: 12.0,
                critical: 8.0,
            },
            RadarData {
                label: "SAPS III",
                value: m.saps3_score.unwrap_or(0) as f32,
                max: 104.0,
                warning: 50.0,
                critical: 70.0,
            },
            RadarData {
                label: "NEWS2",
                value: m.news2_score.unwrap_or(0) as f32,
                max: 64.0,
                warning: 7.0,
                critical: 14.0,
            },
            RadarData {
                label: "SOFA",
                value: m.sofa_score.unwrap_or(0) as f32,
                max: 24.0,
                warning: 12.0,
                critical: 18.0,
            },
        ]
    }

    pub fn normalized(&self) -> f32 {
        ((self.value / self.max) * 100.0).min(100.0)
    }

    pub fn color(&self) -> &'static str {
        if self.value >= self.critical {
            "#EF4444"
        } else if self.value >= self.warning {
            "#F97316"
        } else {
            "#10B981"
        }
    }
}

#[component]
pub fn RadarChart(data: Vec<RadarData>, size: i32) -> impl IntoView {
    let center = size as f32 / 2.0;
    let radius = (size - 60) as f32 / 2.0;
    let n = data.len();
    
    let angle_offset = -90.0_f32;

    let get_point = |i: usize, value: f32| -> (f32, f32) {
        let angle = (angle_offset + (i as f32 * 360.0 / n as f32)) * 3.14159 / 180.0;
        let x = center + (angle.cos() * radius * value / 100.0);
        let y = center + (angle.sin() * radius * value / 100.0);
        (x, y)
    };

    let get_label_point = |i: usize| -> (f32, f32) {
        let angle = (angle_offset + (i as f32 * 360.0 / n as f32)) * 3.14159 / 180.0;
        let x = center + (angle.cos() * radius) * 1.18;
        let y = center + (angle.sin() * radius) * 1.18;
        (x, y)
    };

    let polygon_points = data.iter()
        .enumerate()
        .map(|(i, d)| {
            let (x, y) = get_point(i, d.normalized());
            format!("{},{}", x, y)
        })
        .collect::<Vec<_>>()
        .join(" ");

    view! {
        <svg width={size} height={size} viewBox={format!("0 0 {} {}", size, size)} class="overflow-visible">
            <defs>
                <linearGradient id="radarGradient" x1="0%" y1="0%" x2="100%" y2="100%">
                    <stop offset="0%" style="stop-color:#3B82F6;stop-opacity:0.6" />
                    <stop offset="100%" style="stop-color:#8B5CF6;stop-opacity:0.3" />
                </linearGradient>
            </defs>

            // Círculos concéntricos de referencia
            {[0.0_f32, 0.33, 0.66, 1.0].iter().map(|pct| {
                let r = radius * pct;
                view! {
                    <circle cx={center} cy={center} r={r} 
                        fill="none" stroke="var(--uci-border)" stroke-width="1" opacity="0.4" 
                        stroke-dasharray={if *pct == 0.0 { "none" } else { "4 4" }} />
                }
            }).collect_view()}

            // Líneas de eje desde el centro
            {data.iter().enumerate().map(|(i, _)| {
                let (lx, ly) = get_label_point(i);
                view! {
                    <line x1={center} y1={center} x2={lx} y2={ly} 
                        stroke="var(--uci-border)" stroke-width="1" opacity="0.4" />
                }
            }).collect_view()}

            // Polígono de datos (área cerrada)
            <polygon points={polygon_points} 
                fill="url(#radarGradient)" 
                stroke="#3B82F6" 
                stroke-width="2.5"
                stroke-linejoin="round" />

            // Puntos de datos en los vértices
            {data.iter().enumerate().map(|(i, d)| {
                let (x, y) = get_point(i, d.normalized());
                view! {
                    <circle cx={x} cy={y} r="6" fill={d.color()} stroke="white" stroke-width="2" />
                    <circle cx={x} cy={y} r="10" fill={d.color()} opacity="0.2" />
                }
            }).collect_view()}

            // Labels
            {data.iter().enumerate().map(|(i, d)| {
                let (x, y) = get_label_point(i);
                let dy = if y > center { 18.0_f32 } else { -8.0_f32 };
                view! {
                    <text x={x} y={y + dy} text-anchor="middle" 
                        fill="var(--uci-text)" font-size="11" font-weight="600">
                        {d.label}
                    </text>
                    <text x={x} y={y + dy + 14.0} text-anchor="middle" 
                        fill={d.color()} font-size="10" font-weight="700">
                        {format!("{:.0}", d.value)}
                    </text>
                }
            }).collect_view()}

            // Centro
            <circle cx={center} cy={center} r="3" fill="var(--uci-accent)" />
        </svg>
    }
}

#[component]
pub fn OrganRadar(data: Vec<RadarData>, size: i32) -> impl IntoView {
    let center = size as f32 / 2.0;
    let radius = (size - 60) as f32 / 2.0;
    let n = data.len();
    let angle_offset = -90.0_f32;

    let get_point = |i: usize, value: f32| -> (f32, f32) {
        let angle = (angle_offset + (i as f32 * 360.0 / n as f32)) * 3.14159 / 180.0;
        let x = center + (angle.cos() * radius * value / 100.0);
        let y = center + (angle.sin() * radius * value / 100.0);
        (x, y)
    };

    let polygon_points = data.iter()
        .enumerate()
        .map(|(i, d)| {
            let (x, y) = get_point(i, d.normalized());
            format!("{},{}", x, y)
        })
        .collect::<Vec<_>>()
        .join(" ");

    view! {
        <svg width={size} height={size} viewBox={format!("0 0 {} {}", size, size)} class="overflow-visible">
            <defs>
                <linearGradient id="organGradient" x1="0%" y1="0%" x2="100%" y2="100%">
                    <stop offset="0%" style="stop-color:#EF4444;stop-opacity:0.5" />
                    <stop offset="100%" style="stop-color:#F97316;stop-opacity:0.3" />
                </linearGradient>
            </defs>

            {[0.33_f32, 0.66, 1.0].iter().map(|pct| {
                let r = radius * pct;
                view! {
                    <circle cx={center} cy={center} r={r} 
                        fill="none" stroke="var(--uci-border)" stroke-width="1" opacity="0.3" />
                }
            }).collect_view()}

            {data.iter().enumerate().map(|(i, _)| {
                let angle = (angle_offset + (i as f32 * 360.0 / n as f32)) * 3.14159 / 180.0;
                let lx = center + (angle.cos() * radius * 1.2);
                let ly = center + (angle.sin() * radius * 1.2);
                view! {
                    <line x1={center} y1={center} x2={lx} y2={ly} 
                        stroke="var(--uci-border)" stroke-width="1" opacity="0.3" />
                }
            }).collect_view()}

            <polygon points={polygon_points} 
                fill="url(#organGradient)" 
                stroke="#EF4444" 
                stroke-width="2"
                stroke-linejoin="round" />

            {data.iter().enumerate().map(|(i, d)| {
                let (x, y) = get_point(i, d.normalized());
                view! {
                    <circle cx={x} cy={y} r="5" fill={d.color()} stroke="white" stroke-width="2" />
                }
            }).collect_view()}

            <circle cx={center} cy={center} r="3" fill="var(--uci-accent)" />
        </svg>
    }
}

#[component]
pub fn MiniRadarChart(
    data: Vec<RadarData>,
    size: i32,
) -> impl IntoView {
    let center = size as f32 / 2.0;
    let radius = (size - 30) as f32 / 2.0;
    let n = data.len().max(1);
    let angle_offset = -90.0_f32;

    let get_point = |i: usize, value: f32| -> (f32, f32) {
        let angle = (angle_offset + (i as f32 * 360.0 / n as f32)) * 3.14159 / 180.0;
        let x = center + (angle.cos() * radius * value / 100.0);
        let y = center + (angle.sin() * radius * value / 100.0);
        (x, y)
    };

    let polygon_points = data.iter()
        .enumerate()
        .map(|(i, d)| {
            let (x, y) = get_point(i, d.normalized());
            format!("{},{}", x, y)
        })
        .collect::<Vec<_>>()
        .join(" ");

    view! {
        <svg width={size} height={size} viewBox={format!("0 0 {} {}", size, size)}>
            {[0.5_f32, 1.0].iter().map(|pct| {
                view! {
                    <circle cx={center} cy={center} r={radius * pct} 
                        fill="none" stroke="var(--uci-border)" stroke-width="0.5" opacity="0.5" />
                }
            }).collect_view()}

            <polygon points={polygon_points} 
                fill="var(--uci-accent)" 
                fill-opacity="0.3"
                stroke="var(--uci-accent)" 
                stroke-width="1.5"
                stroke-linejoin="round" />

            {data.iter().enumerate().map(|(i, d)| {
                let (x, y) = get_point(i, d.normalized());
                view! {
                    <circle cx={x} cy={y} r="2.5" fill={d.color()} />
                }
            }).collect_view()}
        </svg>
    }
}