use dmart_shared::models::*;
use leptos::prelude::*;

#[derive(Clone, Debug)]
pub struct ClinicalAlert {
    pub level: AlertLevel,
    pub message: String,
    pub recommendation: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AlertLevel {
    Info,
    Warning,
    Critical,
}

impl AlertLevel {
    pub fn color(&self) -> &str {
        match self {
            AlertLevel::Info => "#3B82F6",
            AlertLevel::Warning => "#F97316",
            AlertLevel::Critical => "#EF4444",
        }
    }
}

pub fn analyze_patient(m: &Measurement) -> Vec<ClinicalAlert> {
    let mut alerts = Vec::new();

    // APACHE II alerts
    if m.apache_score >= 25 {
        alerts.push(ClinicalAlert {
            level: AlertLevel::Critical,
            message: format!(
                "APACHE II score {} indicates high mortality risk",
                m.apache_score
            ),
            recommendation: "Consider early intervention and ICU escalation".to_string(),
        });
    } else if m.apache_score >= 15 {
        alerts.push(ClinicalAlert {
            level: AlertLevel::Warning,
            message: format!("APACHE II score {} indicates moderate risk", m.apache_score),
            recommendation: "Monitor closely and reassess in 4-6 hours".to_string(),
        });
    }

    // GCS alerts
    if m.gcs_score <= 8 {
        alerts.push(ClinicalAlert {
            level: AlertLevel::Critical,
            message: format!("GCS {} indicates severe neurologic impairment", m.gcs_score),
            recommendation: " Evaluate for airway protection, consider ICU".to_string(),
        });
    } else if m.gcs_score < 13 {
        alerts.push(ClinicalAlert {
            level: AlertLevel::Warning,
            message: format!("GCS {} indicates altered consciousness", m.gcs_score),
            recommendation: "Neurological monitoring recommended".to_string(),
        });
    }

    // NEWS2 alerts
    if let Some(news) = m.news2_score {
        if news >= 7 {
            alerts.push(ClinicalAlert {
                level: AlertLevel::Critical,
                message: format!("NEWS2 {} indicates emergency response needed", news),
                recommendation: "Activate clinical review immediately".to_string(),
            });
        } else if news >= 5 {
            alerts.push(ClinicalAlert {
                level: AlertLevel::Warning,
                message: format!("NEWS2 {} indicates medium risk", news),
                recommendation: "Review within 1 hour".to_string(),
            });
        }
    }

    // SOFA alerts
    if let Some(sofa) = m.sofa_score {
        if sofa >= 12 {
            alerts.push(ClinicalAlert {
                level: AlertLevel::Critical,
                message: format!("SOFA {} indicates multi-organ dysfunction", sofa),
                recommendation: "Intensive monitoring and support required".to_string(),
            });
        }
    }

    // Mortality risk
    if m.mortality_risk > 50.0 {
        alerts.push(ClinicalAlert {
            level: AlertLevel::Critical,
            message: format!("Predicted mortality: {:.0}%", m.mortality_risk),
            recommendation: "Prepare family for high-risk scenario".to_string(),
        });
    } else if m.mortality_risk > 25.0 {
        alerts.push(ClinicalAlert {
            level: AlertLevel::Warning,
            message: format!("Predicted mortality: {:.0}%", m.mortality_risk),
            recommendation: "Discuss prognosis with team".to_string(),
        });
    }

    // News2 response action
    let response = m.news2_level.response();
    if !response.is_empty() && response != "Monitoreo habitual" {
        alerts.push(ClinicalAlert {
            level: if matches!(m.news2_level, dmart_shared::models::News2Level::Emergent) {
                AlertLevel::Critical
            } else {
                AlertLevel::Warning
            },
            message: response.to_string(),
            recommendation: "Follow NEWS2 response protocol".to_string(),
        });
    }

    if alerts.is_empty() {
        alerts.push(ClinicalAlert {
            level: AlertLevel::Info,
            message: "Patient stable".to_string(),
            recommendation: "Continue routine monitoring".to_string(),
        });
    }

    alerts
}

pub fn get_prediction_summary(m: &Measurement) -> String {
    let mut parts = vec![];

    parts.push(format!(
        "APACHE II: {} ({:.0}% riesgo)",
        m.apache_score, m.mortality_risk
    ));

    if let Some(saps) = m.saps3_score {
        if let Some(mort) = m.saps3_mortality {
            parts.push(format!("SAPS III: {} ({:.0}% riesgo)", saps, mort));
        }
    }

    if let Some(sofa) = m.sofa_score {
        if let Some(mort) = m.sofa_mortality {
            parts.push(format!("SOFA: {} ({:.0}% riesgo)", sofa, mort));
        }
    }

    parts.join(" | ")
}

#[component]
pub fn ClinicalAlerts(alerts: Vec<ClinicalAlert>) -> impl IntoView {
    view! {
        <div style="display:flex; flex-direction:column; gap:8px;">
            {alerts.iter().map(|a| view! {
                <div style={format!("padding:12px; background:{}; border-radius:8px; border-left:4px solid {}; opacity:0.9;",
                    format!("{0}20", a.level.color()),
                    a.level.color()
                )}>
                    <div style="display:flex; align-items:center; gap:8px;">
                        <span style="font-size:16px;">
                            {if matches!(a.level, AlertLevel::Info) { "ℹ️" } else if matches!(a.level, AlertLevel::Warning) { "⚠️" } else { "🚨" }}
                        </span>
                        <div>
                            <div style="font-weight:600; color:var(--uci-text); font-size:13px;">{a.message.clone()}</div>
                            <div style="font-size:12px; color:var(--uci-muted);">{a.recommendation.clone()}</div>
                        </div>
                    </div>
                </div>
            }).collect_view()}
        </div>
    }
}
