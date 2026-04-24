use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Serialize;
use dmart_shared::models::*;
use crate::db::Database;
use crate::db as db_ops;

fn calculate_age(fecha_nacimiento: &str) -> u8 {
    if fecha_nacimiento.is_empty() || fecha_nacimiento.len() < 4 {
        return 0;
    }
    let birth_year: i32 = fecha_nacimiento[..4].parse().unwrap_or(2000);
    let current_year = 2026;
    (current_year - birth_year).max(0) as u8
}

#[derive(Serialize)]
pub struct UciStats {
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

pub async fn get_stats(
    State(db): State<Database>,
) -> impl IntoResponse {
    let result = db_ops::list_patients(&db).await;

    match result {
        Ok(patients) => {
            let total = patients.len();
            
            let criticos = patients.iter().filter(|p| matches!(p.estado_gravedad, SeverityLevel::Critico)).count();
            let severos = patients.iter().filter(|p| matches!(p.estado_gravedad, SeverityLevel::Severo)).count();
            let moderados = patients.iter().filter(|p| matches!(p.estado_gravedad, SeverityLevel::Moderado)).count();
            let bajos = patients.iter().filter(|p| matches!(p.estado_gravedad, SeverityLevel::Bajo)).count();

            let mut apache_sum = 0u32;
            let mut gcs_sum = 0u32;
            let mut sofa_sum = 0u32;
            let mut saps_sum = 0u32;
            let mut news_sum = 0u32;
            let mut count_with_scores = 0usize;

            for p in &patients {
                if let Some(s) = p.ultimo_apache_score {
                    apache_sum += s;
                }
                if let Some(s) = p.ultimo_gcs_score {
                    gcs_sum += s as u32;
                }
                if let Some(s) = p.ultimo_sofa_score {
                    sofa_sum += s;
                }
                if let Some(s) = p.ultimo_saps3_score {
                    saps_sum += s;
                }
                if let Some(s) = p.ultimo_news2_score {
                    news_sum += s;
                }
                if p.ultimo_apache_score.is_some() || p.ultimo_sofa_score.is_some() {
                    count_with_scores += 1;
                }
            }

            let count = if count_with_scores > 0 { count_with_scores } else { 1 };
            
            let promedios = PromedioScores {
                apache_promedio: apache_sum as f32 / count as f32,
                gcs_promedio: gcs_sum as f32 / count as f32,
                sofa_promedio: sofa_sum as f32 / count as f32,
                saps3_promedio: saps_sum as f32 / count as f32,
                news2_promedio: news_sum as f32 / count as f32,
            };

            let gravedad = GravedadStats {
                criticos,
                severos,
                moderados,
                bajos,
            };

            let items: Vec<PatientListItem> = patients.iter().map(|p| {
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
            }).collect();

            let stats = UciStats {
                total_pacientes: total,
                pacientes_activos: total,
                por_gravedad: gravedad,
                promedios,
                reciente: items,
            };

            (StatusCode::OK, Json(ApiResponse::ok(stats))).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<UciStats>::err(e.to_string())),
        ).into_response(),
    }
}