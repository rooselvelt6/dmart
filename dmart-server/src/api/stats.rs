use crate::db as db_ops;
use crate::db::Database;
use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use chrono::Datelike;
use dmart_shared::models::*;
use serde::Serialize;

fn calculate_age(fecha_nacimiento: &str) -> u8 {
    if fecha_nacimiento.is_empty() || fecha_nacimiento.len() < 4 {
        return 0;
    }
    let birth_year: i32 = fecha_nacimiento[..4].parse().unwrap_or(2000);
    let current_year = chrono::Utc::now().year();
    (current_year - birth_year).max(0) as u8
}

#[derive(Serialize)]
pub struct UciStats {
    pub ejecutivo: EjecutivoKpi,
    pub total_pacientes: usize,
    pub pacientes_activos: usize,
    pub por_gravedad: GravedadStats,
    pub promedios: PromedioScores,
    pub reciente: Vec<PatientListItem>,
}

#[derive(Serialize)]
pub struct GravedadStats {
    pub criticos: usize,
    pub severos: usize,
    pub moderados: usize,
    pub bajos: usize,
}

#[derive(Serialize)]
pub struct PromedioScores {
    pub apache_promedio: f32,
    pub gcs_promedio: f32,
    pub sofa_promedio: f32,
    pub saps3_promedio: f32,
    pub news2_promedio: f32,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub struct EjecutivoKpi {
    pub egresados: u64,
    pub fallecidos: u64,
    pub mortalidad_real_pct: f64,
    pub mortalidad_predicha_pct: f64,
    pub los_dias_promedio: f64,
}

pub async fn get_stats(State(db): State<Database>) -> impl IntoResponse {
    let agg_result = db_ops::aggregate_patient_stats(&db).await;
    let recent_result = db_ops::list_patients(&db, 50, 0).await;

    match (agg_result, recent_result) {
        (Ok(agg), Ok(patients)) => {
            let gravedad = GravedadStats {
                criticos: agg.criticos as usize,
                severos: agg.severos as usize,
                moderados: agg.moderados as usize,
                bajos: agg.bajos as usize,
            };

            let avg =
                |sum: f64, n: u64| -> f32 { if n > 0 { (sum / n as f64) as f32 } else { 0.0 } };

            let ejecutivo = EjecutivoKpi {
                egresados: agg.egresados,
                fallecidos: agg.fallecidos,
                mortalidad_real_pct: {
                    let pct = if agg.fallecidos > 0 {
                        agg.fallecidos as f64 * 100.0 / agg.egresados as f64
                    } else {
                        0.0
                    };
                    if (0.0..=100.0).contains(&pct) {
                        pct
                    } else {
                        0.0
                    }
                },
                mortalidad_predicha_pct: {
                    let pct = if agg.mortalidad_predicha_n > 0 {
                        agg.mortalidad_predicha_sum / agg.mortalidad_predicha_n as f64
                    } else {
                        0.0
                    };
                    if (0.0..=100.0).contains(&pct) {
                        pct
                    } else {
                        0.0
                    }
                },
                los_dias_promedio: {
                    if agg.los_dias_sum > 0.0 {
                        agg.los_dias_sum / agg.los_dias_n as f64
                    } else {
                        0.0
                    }
                },
            };

            let promedios = PromedioScores {
                apache_promedio: avg(agg.apache_sum, agg.apache_n),
                gcs_promedio: avg(agg.gcs_sum, agg.gcs_n),
                sofa_promedio: avg(agg.sofa_sum, agg.sofa_n),
                saps3_promedio: avg(agg.saps3_sum, agg.saps3_n),
                news2_promedio: avg(agg.news2_sum, agg.news2_n),
            };

            let items: Vec<PatientListItem> = patients
                .iter()
                .map(|p| {
                    let edad = calculate_age(&p.fecha_nacimiento);
                    PatientListItem {
                        id: p.patient_id.clone(),
                        nombre_completo: p.nombre_completo(),
                        cedula: p.cedula.clone(),
                        historia_clinica: p.historia_clinica.clone(),
                        edad,
                        sexo: p.sexo.clone(),
                        fecha_ingreso_uci: p.fecha_ingreso_uci.clone(),
                        estado_gravedad: p.estado_gravedad.clone(),
                        ultimo_apache_score: p.ultimo_apache_score,
                        ultimo_gcs_score: p.ultimo_gcs_score,
                        ultimo_sofa_score: p.ultimo_sofa_score,
                        ultimo_saps3_score: p.ultimo_saps3_score,
                        ultimo_news2_score: p.ultimo_news2_score,
                        mortality_risk: p.mortality_risk,
                    }
                })
                .collect();

            let stats = UciStats {
                ejecutivo,
                total_pacientes: agg.total as usize,
                pacientes_activos: agg.total as usize,
                por_gravedad: gravedad,
                promedios,
                reciente: items,
            };

            (StatusCode::OK, Json(ApiResponse::ok(stats))).into_response()
        }
        (Err(e), _) | (_, Err(e)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<UciStats>::err(e.to_string())),
        )
            .into_response(),
    }
}
