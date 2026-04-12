use dmart_shared::models::*;
use leptos::prelude::*;

#[derive(Clone)]
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
    let radius = (size - 40) as f32 / 2.0;

    view! {
        <svg width={size} height={size} viewBox={format!("0 0 {} {}", size, size)}>
            <circle cx={center} cy={center} r={radius} fill="none" stroke="var(--uci-border)" stroke-width="1" opacity="0.3" />
            <circle cx={center} cy={center} r={radius * 0.66} fill="none" stroke="var(--uci-border)" stroke-width="1" opacity="0.3" />
            <circle cx={center} cy={center} r={radius * 0.33} fill="none" stroke="var(--uci-border)" stroke-width="1" opacity="0.3" />

            {data.iter().enumerate().map(|(i, d)| {
                let angle = (i as f32 * 72.0 - 90.0) * 3.14159 / 180.0;
                let x = center + (angle.cos() * radius);
                let y = center + (angle.sin() * radius);
                let vx = center + (angle.cos() * radius * d.normalized() / 100.0);
                let vy = center + (angle.sin() * radius * d.normalized() / 100.0);

                view! {
                    // Línea del eje
                    <line x1={center} y1={center} x2={x} y2={y} stroke="var(--uci-border)" stroke-width="1" opacity="0.3" />
                    // Línea de datos
                    <line x1={vx} y1={vy} x2={vx} y2={vy} stroke={d.color()} stroke-width="3" stroke-linecap="round" />
                    // Punto
                    <circle cx={vx} cy={vy} r="4" fill={d.color()} />
                    // Label
                    <text x={x} y={y + 15.0} text-anchor="middle" fill="var(--uci-muted)" font-size="10">{d.label}</text>
                }
            }).collect_view()}

            // Título central
            <text x={center} y={center + 4.0} text-anchor="middle" fill="var(--uci-text)" font-size="14" font-weight="700">"Scores"</text>
        </svg>
    }
}

#[component]
pub fn OrganRadar(data: Vec<RadarData>, size: i32) -> impl IntoView {
    let center = size as f32 / 2.0;
    let radius = (size - 40) as f32 / 2.0;

    view! {
        <svg width={size} height={size} viewBox={format!("0 0 {} {}", size, size)}>
            <circle cx={center} cy={center} r={radius} fill="none" stroke="var(--uci-border)" stroke-width="1" opacity="0.3" />
            <circle cx={center} cy={center} r={radius * 0.66} fill="none" stroke="var(--uci-border)" stroke-width="1" opacity="0.3" />
            <circle cx={center} cy={center} r={radius * 0.33} fill="none" stroke="var(--uci-border)" stroke-width="1" opacity="0.3" />

            {data.iter().enumerate().map(|(i, d)| {
                let angle = (i as f32 * 72.0 - 90.0) * 3.14159 / 180.0;
                let x = center + (angle.cos() * radius);
                let y = center + (angle.sin() * radius);
                let vx = center + (angle.cos() * radius * d.normalized() / 100.0);
                let vy = center + (angle.sin() * radius * d.normalized() / 100.0);

                view! {
                    <line x1={center} y1={center} x2={x} y2={y} stroke="var(--uci-border)" stroke-width="1" opacity="0.3" />
                    <line x1={vx} y1={vy} x2={vx} y2={vy} stroke={d.color()} stroke-width="3" />
                    <circle cx={vx} cy={vy} r="4" fill={d.color()} />
                    <text x={x} y={y + 15.0} text-anchor="middle" fill="var(--uci-muted)" font-size="9">{d.label}</text>
                }
            }).collect_view()}

            <text x={center} y={center + 4.0} text-anchor="middle" fill="var(--uci-text)" font-size="12" font-weight="700">"Órganos"</text>
        </svg>
    }
}
