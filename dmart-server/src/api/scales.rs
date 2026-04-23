use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use dmart_shared::models::*;
use dmart_shared::scales::*;
use crate::db::Database;
use crate::db as db_ops;

// ─── Request DTOs ───────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ApacheRequest {
    pub data: ApacheIIData,
    pub notas: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GcsRequest {
    pub apertura_ocular: u8,
    pub respuesta_verbal: u8,
    pub respuesta_motora: u8,
    pub notas: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct News2Request {
    pub frecuencia_respiratoria: f32,
    pub spo2: f32,
    pub o2_suplementario: bool,
    pub presion_sistolica: f32,
    pub frecuencia_cardiaca: f32,
    pub temperatura: f32,
    pub alerta: bool,
    pub notas: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SofaRequest {
    pub pao2: f32,
    pub fio2: f32,
    pub plaquetas: f32,
    pub bilirrubina: f32,
    pub presion_arterial_media: f32,
    pub vasopresores: bool,
    pub dosis_vasopresor: f32,
    pub gcs_total: u8,
    pub creatinina: f32,
    pub diuresis_diaria: u32,
    pub notas: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Saps3Request {
    pub edad: u8,
    pub dias_pre_uci: u8,
    pub tipo_admision: Option<String>,
    pub fuente_admision: Option<String>,
    pub infeccion_admision: Option<String>,
    pub sistema_anatomico: Option<String>,
    pub presion_sistolica: f32,
    pub frecuencia_cardiaca: f32,
    pub gcs_total: u8,
    pub bilirrubina: f32,
    pub creatinina: f32,
    pub plaquetas: f32,
    pub ph_arterial: f32,
    pub ventilacion_mecanica: bool,
    pub vasopresores: bool,
    pub notas: Option<String>,
}

// ─── Response DTOs ──────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ApacheResult {
    pub measurement_id: String,
    pub patient_id: String,
    pub timestamp: String,
    pub apache_score: u32,
    pub gcs_score: u8,
    pub severity: SeverityLevel,
    pub mortality_risk: f32,
    pub notas: String,
}

#[derive(Debug, Serialize)]
pub struct GcsResult {
    pub measurement_id: String,
    pub patient_id: String,
    pub timestamp: String,
    pub apertura_ocular: u8,
    pub respuesta_verbal: u8,
    pub respuesta_motora: u8,
    pub total: u8,
    pub interpretacion: String,
    pub notas: String,
}

#[derive(Debug, Serialize)]
pub struct News2Result {
    pub measurement_id: String,
    pub patient_id: String,
    pub timestamp: String,
    pub score: u32,
    pub nivel: String,
    pub respuesta_clinica: String,
    pub notas: String,
}

#[derive(Debug, Serialize)]
pub struct SofaResult {
    pub measurement_id: String,
    pub patient_id: String,
    pub timestamp: String,
    pub score: u32,
    pub nivel: String,
    pub mortalidad_estimada: f32,
    pub notas: String,
}

#[derive(Debug, Serialize)]
pub struct Saps3Result {
    pub measurement_id: String,
    pub patient_id: String,
    pub timestamp: String,
    pub score: u32,
    pub mortalidad_estimada: f32,
    pub nivel: String,
    pub notas: String,
}

#[derive(Debug, Serialize)]
pub struct ScaleHistoryEntry {
    pub measurement_id: String,
    pub timestamp: String,
    pub apache_score: Option<u32>,
    pub gcs_score: Option<u8>,
    pub news2_score: Option<u32>,
    pub sofa_score: Option<u32>,
    pub saps3_score: Option<u32>,
    pub severity: SeverityLevel,
    pub mortality_risk: f32,
    pub notas: String,
}

// ─── Handlers ───────────────────────────────────────────────────────────────

/// POST /api/patients/:id/scales/apache
pub async fn calc_apache(
    State(db): State<Database>,
    Path(patient_id): Path<String>,
    Json(body): Json<ApacheRequest>,
) -> impl IntoResponse {
    let apache_score = calculate_apache_ii_score(&body.data);
    let gcs_score = calculate_gcs_score_from_total(body.data.gcs_total);
    let severity = SeverityLevel::from_score(apache_score);
    let mort = mortality_risk(apache_score);
    let mid = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    // Guardar medición completa
    let gcs_data = GcsData {
        apertura_ocular: body.data.gcs_ojos,
        respuesta_verbal: body.data.gcs_verbal,
        respuesta_motora: body.data.gcs_motor,
    };
    let saps3 = calculate_saps_iii_score(&body.data);
    let news2 = calculate_news2_score(&body.data);
    let sofa = calculate_sofa_score(&body.data);

    let m = Measurement {
        id: None,
        measurement_id: mid.clone(),
        patient_id: patient_id.clone(),
        timestamp: now.clone(),
        apache_data: body.data,
        gcs_data,
        apache_score,
        gcs_score,
        severity: severity.clone(),
        mortality_risk: mort,
        saps3_score: Some(saps3),
        saps3_mortality: Some(saps_iii_mortality_prediction(saps3)),
        news2_score: Some(news2),
        news2_level: News2Level::from_score(news2),
        sofa_score: Some(sofa),
        sofa_mortality: Some(sofa_mortality_estimate(sofa)),
        notas: body.notas.clone().unwrap_or_default(),
    };

    match db_ops::create_measurement(&db, m).await {
        Ok(_) => {
            // Actualizar paciente
            if let Ok(Some(mut p)) = db_ops::get_patient(&db, &patient_id).await {
                p.estado_gravedad = severity.clone();
                p.ultimo_apache_score = Some(apache_score);
                p.ultimo_gcs_score = Some(gcs_score);
                p.updated_at = Utc::now().to_rfc3339();
                let _ = db_ops::update_patient(&db, &patient_id, p).await;
            }
            let result = ApacheResult {
                measurement_id: mid,
                patient_id,
                timestamp: now,
                apache_score,
                gcs_score,
                severity,
                mortality_risk: mort,
                notas: body.notas.unwrap_or_default(),
            };
            (StatusCode::CREATED, Json(ApiResponse::ok(result))).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<ApacheResult>::err(e.to_string())),
        ).into_response(),
    }
}

/// POST /api/patients/:id/scales/gcs
pub async fn calc_gcs(
    State(db): State<Database>,
    Path(patient_id): Path<String>,
    Json(body): Json<GcsRequest>,
) -> impl IntoResponse {
    let gcs = GcsData {
        apertura_ocular: body.apertura_ocular.clamp(1, 4),
        respuesta_verbal: body.respuesta_verbal.clamp(1, 5),
        respuesta_motora: body.respuesta_motora.clamp(1, 6),
    };
    let total = gcs.total();
    let interpretacion = gcs.interpret().to_string();
    let mid = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    let mut apache_data = ApacheIIData::default();
    apache_data.gcs_total = total;
    apache_data.gcs_ojos = gcs.apertura_ocular;
    apache_data.gcs_verbal = gcs.respuesta_verbal;
    apache_data.gcs_motor = gcs.respuesta_motora;

    let apache_score = calculate_apache_ii_score(&apache_data);
    let severity = SeverityLevel::from_score(apache_score);

    let m = Measurement {
        id: None,
        measurement_id: mid.clone(),
        patient_id: patient_id.clone(),
        timestamp: now.clone(),
        apache_data,
        gcs_data: gcs,
        apache_score,
        gcs_score: total,
        severity: severity.clone(),
        mortality_risk: mortality_risk(apache_score),
        saps3_score: None,
        saps3_mortality: None,
        news2_score: None,
        news2_level: News2Level::Bajo,
        sofa_score: None,
        sofa_mortality: None,
        notas: body.notas.clone().unwrap_or_default(),
    };

    match db_ops::create_measurement(&db, m).await {
        Ok(_) => {
            if let Ok(Some(mut p)) = db_ops::get_patient(&db, &patient_id).await {
                p.ultimo_gcs_score = Some(total);
                p.updated_at = Utc::now().to_rfc3339();
                let _ = db_ops::update_patient(&db, &patient_id, p).await;
            }
            let result = GcsResult {
                measurement_id: mid,
                patient_id,
                timestamp: now,
                apertura_ocular: body.apertura_ocular,
                respuesta_verbal: body.respuesta_verbal,
                respuesta_motora: body.respuesta_motora,
                total,
                interpretacion,
                notas: body.notas.unwrap_or_default(),
            };
            (StatusCode::CREATED, Json(ApiResponse::ok(result))).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<GcsResult>::err(e.to_string())),
        ).into_response(),
    }
}

/// POST /api/patients/:id/scales/news2
pub async fn calc_news2(
    State(db): State<Database>,
    Path(patient_id): Path<String>,
    Json(body): Json<News2Request>,
) -> impl IntoResponse {
    let mut apache_data = ApacheIIData::default();
    apache_data.frecuencia_respiratoria = body.frecuencia_respiratoria;
    apache_data.spo2 = body.spo2;
    apache_data.o2_suplementario = body.o2_suplementario;
    apache_data.presion_sistolica = body.presion_sistolica;
    apache_data.frecuencia_cardiaca = body.frecuencia_cardiaca;
    apache_data.temperatura = body.temperatura;
    apache_data.alerta = body.alerta;

    let score = calculate_news2_score(&apache_data);
    let level = News2Level::from_score(score);
    let mid = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    let nivel = level.label().to_string();
    let respuesta_clinica = level.response().to_string();

    let m = Measurement {
        id: None,
        measurement_id: mid.clone(),
        patient_id: patient_id.clone(),
        timestamp: now.clone(),
        apache_data,
        gcs_data: GcsData::default(),
        apache_score: 0,
        gcs_score: 0,
        severity: SeverityLevel::Bajo,
        mortality_risk: 0.0,
        saps3_score: None,
        saps3_mortality: None,
        news2_score: Some(score),
        news2_level: level,
        sofa_score: None,
        sofa_mortality: None,
        notas: body.notas.clone().unwrap_or_default(),
    };

    match db_ops::create_measurement(&db, m).await {
        Ok(_) => {
            let result = News2Result {
                measurement_id: mid,
                patient_id,
                timestamp: now,
                score,
                nivel,
                respuesta_clinica,
                notas: body.notas.unwrap_or_default(),
            };
            (StatusCode::CREATED, Json(ApiResponse::ok(result))).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<News2Result>::err(e.to_string())),
        ).into_response(),
    }
}

/// POST /api/patients/:id/scales/sofa
pub async fn calc_sofa(
    State(db): State<Database>,
    Path(patient_id): Path<String>,
    Json(body): Json<SofaRequest>,
) -> impl IntoResponse {
    let mut apache_data = ApacheIIData::default();
    apache_data.pao2 = Some(body.pao2);
    apache_data.fio2 = body.fio2;
    apache_data.plaquetas = body.plaquetas;
    apache_data.bilirrubina = body.bilirrubina;
    apache_data.presion_arterial_media = body.presion_arterial_media;
    apache_data.vasopresores = body.vasopresores;
    apache_data.dosis_vasopresor = body.dosis_vasopresor;
    apache_data.gcs_total = body.gcs_total;
    apache_data.creatinina = body.creatinina;
    apache_data.diuresis_diaria = body.diuresis_diaria;

    let score = calculate_sofa_score(&apache_data);
    let level = SofaLevel::from_score(score);
    let mort = sofa_mortality_estimate(score);
    let mid = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    let nivel = level.label().to_string();

    let m = Measurement {
        id: None,
        measurement_id: mid.clone(),
        patient_id: patient_id.clone(),
        timestamp: now.clone(),
        apache_data,
        gcs_data: GcsData::default(),
        apache_score: 0,
        gcs_score: 0,
        severity: SeverityLevel::Bajo,
        mortality_risk: 0.0,
        saps3_score: None,
        saps3_mortality: None,
        news2_score: None,
        news2_level: News2Level::Bajo,
        sofa_score: Some(score),
        sofa_mortality: Some(mort),
        notas: body.notas.clone().unwrap_or_default(),
    };

    match db_ops::create_measurement(&db, m).await {
        Ok(_) => {
            let result = SofaResult {
                measurement_id: mid,
                patient_id,
                timestamp: now,
                score,
                nivel,
                mortalidad_estimada: mort,
                notas: body.notas.unwrap_or_default(),
            };
            (StatusCode::CREATED, Json(ApiResponse::ok(result))).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<SofaResult>::err(e.to_string())),
        ).into_response(),
    }
}

/// POST /api/patients/:id/scales/saps3
pub async fn calc_saps3(
    State(db): State<Database>,
    Path(patient_id): Path<String>,
    Json(body): Json<Saps3Request>,
) -> impl IntoResponse {
    let mut apache_data = ApacheIIData::default();
    apache_data.edad = body.edad;
    apache_data.dias_pre_uci = body.dias_pre_uci;
    apache_data.tipo_admision = body.tipo_admision.clone();
    apache_data.fuente_admision = body.fuente_admision.clone();
    apache_data.infeccion_admision = body.infeccion_admision.clone();
    apache_data.sistema_anatomico = body.sistema_anatomico.clone();
    apache_data.presion_sistolica = body.presion_sistolica;
    apache_data.frecuencia_cardiaca = body.frecuencia_cardiaca;
    apache_data.gcs_total = body.gcs_total;
    apache_data.bilirrubina = body.bilirrubina;
    apache_data.creatinina = body.creatinina;
    apache_data.plaquetas = body.plaquetas;
    apache_data.ph_arterial = body.ph_arterial;
    apache_data.ventilacion_mecanica = body.ventilacion_mecanica;
    apache_data.vasopresores = body.vasopresores;

    let score = calculate_saps_iii_score(&apache_data);
    let mort = saps_iii_mortality_prediction(score);
    let level = Saps3Level::from_score(score);
    let mid = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    let nivel = level.label().to_string();

    let m = Measurement {
        id: None,
        measurement_id: mid.clone(),
        patient_id: patient_id.clone(),
        timestamp: now.clone(),
        apache_data,
        gcs_data: GcsData::default(),
        apache_score: 0,
        gcs_score: 0,
        severity: SeverityLevel::Bajo,
        mortality_risk: 0.0,
        saps3_score: Some(score),
        saps3_mortality: Some(mort),
        news2_score: None,
        news2_level: News2Level::Bajo,
        sofa_score: None,
        sofa_mortality: None,
        notas: body.notas.clone().unwrap_or_default(),
    };

    match db_ops::create_measurement(&db, m).await {
        Ok(_) => {
            let result = Saps3Result {
                measurement_id: mid,
                patient_id,
                timestamp: now,
                score,
                mortalidad_estimada: mort,
                nivel,
                notas: body.notas.unwrap_or_default(),
            };
            (StatusCode::CREATED, Json(ApiResponse::ok(result))).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<Saps3Result>::err(e.to_string())),
        ).into_response(),
    }
}

/// GET /api/patients/:id/scales/history
pub async fn scale_history(
    State(db): State<Database>,
    Path(patient_id): Path<String>,
) -> impl IntoResponse {
    match db_ops::get_measurements_for_patient(&db, &patient_id).await {
        Ok(ms) => {
            let history: Vec<ScaleHistoryEntry> = ms.into_iter().map(|m| ScaleHistoryEntry {
                measurement_id: m.measurement_id,
                timestamp: m.timestamp,
                apache_score: if m.apache_score > 0 { Some(m.apache_score) } else { None },
                gcs_score: if m.gcs_score > 0 { Some(m.gcs_score) } else { None },
                news2_score: m.news2_score,
                sofa_score: m.sofa_score,
                saps3_score: m.saps3_score,
                severity: m.severity,
                mortality_risk: m.mortality_risk,
                notas: m.notas,
            }).collect();
            (StatusCode::OK, Json(ApiResponse::ok(history))).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<Vec<ScaleHistoryEntry>>::err(e.to_string())),
        ).into_response(),
    }
}
