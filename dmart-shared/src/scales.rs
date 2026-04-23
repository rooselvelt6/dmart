/// Apache II scoring algorithm — fully implemented per KNAUS et al. (1985)
/// Reference: Knaus WA, Draper EA, Wagner DP, Zimmerman JE (1985).
/// APACHE II: a severity of disease classification system.
/// Crit Care Med. 13(10):818-29.
///
/// SAPS III scoring — Moreno et al. (2005)
/// Reference: Moreno RP, Metnitz PG, Almeida E, et al. SAPS 3--From evaluation
/// of the patient to evaluation of the intensive care unit.
/// Intensive Care Med 2005; 31:1345-1355.
///
/// NEWS2 scoring — Royal College of Physicians (2017)
/// Reference: National Early Warning Score (NEWS) 2: Standardising the assessment
/// of acute-illness severity and hospital response.
///
/// SOFA scoring — Vincent et al. (1996)
/// Reference: Vincent JL, Moreno R, Takala J, et al. The SOFA (Sepsis-related
/// Organ Failure Assessment) score to describe organ dysfunction/failure.
/// Intensive Care Med 1996; 22(6):707-10.
use serde::{Serialize, Deserialize};
use crate::models::{ApacheIIData, GcsData};

// ─────────────────────────────────────────────────────────────────────────────
// APS — Acute Physiology Score (12 variables, max 60 points)
// ─────────────────────────────────────────────────────────────────────────────

/// Temperatura rectal (°C)
fn points_temperatura(t: f32) -> u32 {
    match t {
        t if t >= 41.0 => 4,
        t if t >= 39.0 => 3,
        t if t >= 38.5 => 1,
        t if t >= 36.0 => 0,
        t if t >= 34.0 => 1,
        t if t >= 32.0 => 2,
        t if t >= 30.0 => 3,
        _ => 4,
    }
}

/// Presión arterial media (mmHg)
fn points_pam(p: f32) -> u32 {
    match p {
        p if p >= 160.0 => 4,
        p if p >= 130.0 => 3,
        p if p >= 110.0 => 2,
        p if p >= 70.0 => 0,
        p if p >= 50.0 => 2,
        _ => 4,
    }
}

/// Frecuencia cardíaca (lpm)
fn points_fc(fc: f32) -> u32 {
    match fc {
        fc if fc >= 180.0 => 4,
        fc if fc >= 140.0 => 3,
        fc if fc >= 110.0 => 2,
        fc if fc >= 70.0 => 0,
        fc if fc >= 55.0 => 2,
        fc if fc >= 40.0 => 3,
        _ => 4,
    }
}

/// Frecuencia respiratoria (rpm)
fn points_fr(fr: f32) -> u32 {
    match fr {
        fr if fr >= 50.0 => 4,
        fr if fr >= 35.0 => 3,
        fr if fr >= 25.0 => 1,
        fr if fr >= 12.0 => 0,
        fr if fr >= 10.0 => 1,
        fr if fr >= 6.0 => 2,
        _ => 4,
    }
}

/// Oxigenación:
/// - Si FiO2 >= 0.5 → usar A-aDO2 (mmHg)
/// - Si FiO2 < 0.5  → usar PaO2 (mmHg)
fn points_oxigenacion(fio2: f32, pao2: Option<f32>, a_ado2: Option<f32>) -> u32 {
    if fio2 >= 0.5 {
        let aado2 = a_ado2.unwrap_or(0.0);
        match aado2 {
            v if v >= 500.0 => 4,
            v if v >= 350.0 => 3,
            v if v >= 200.0 => 2,
            _ => 0,
        }
    } else {
        let pao2_v = pao2.unwrap_or(80.0);
        match pao2_v {
            v if v >= 70.0 => 0,
            v if v >= 61.0 => 1,
            v if v >= 55.0 => 3,
            _ => 4,
        }
    }
}

/// pH arterial
fn points_ph(ph: f32) -> u32 {
    match ph {
        ph if ph >= 7.70 => 4,
        ph if ph >= 7.60 => 3,
        ph if ph >= 7.50 => 1,
        ph if ph >= 7.33 => 0,
        ph if ph >= 7.25 => 2,
        ph if ph >= 7.15 => 3,
        _ => 4,
    }
}

/// Sodio sérico (mEq/L)
fn points_sodio(na: f32) -> u32 {
    match na {
        na if na >= 180.0 => 4,
        na if na >= 160.0 => 3,
        na if na >= 155.0 => 2,
        na if na >= 150.0 => 1,
        na if na >= 130.0 => 0,
        na if na >= 120.0 => 2,
        na if na >= 111.0 => 3,
        _ => 4,
    }
}

/// Potasio sérico (mEq/L)
fn points_potasio(k: f32) -> u32 {
    match k {
        k if k >= 7.0 => 4,
        k if k >= 6.0 => 3,
        k if k >= 5.5 => 1,
        k if k >= 3.5 => 0,
        k if k >= 3.0 => 1,
        k if k >= 2.5 => 2,
        _ => 4,
    }
}

/// Creatinina sérica (mg/dL)
/// Si hay falla renal aguda, se duplica la puntuación
fn points_creatinina(cr: f32, falla_aguda: bool) -> u32 {
    let pts = match cr {
        cr if cr >= 3.5 => 4,
        cr if cr >= 2.0 => 3,
        cr if cr >= 1.5 => 2,
        cr if cr >= 0.6 => 0,
        _ => 2,
    };
    if falla_aguda {
        pts * 2
    } else {
        pts
    }
}

/// Hematocrito (%)
fn points_hematocrito(hto: f32) -> u32 {
    match hto {
        h if h >= 60.0 => 4,
        h if h >= 50.0 => 2,
        h if h >= 46.0 => 1,
        h if h >= 30.0 => 0,
        h if h >= 20.0 => 2,
        _ => 4,
    }
}

/// Leucocitos (x10³/mm³)
fn points_leucocitos(wbc: f32) -> u32 {
    match wbc {
        w if w >= 40.0 => 4,
        w if w >= 20.0 => 2,
        w if w >= 15.0 => 1,
        w if w >= 3.0 => 0,
        w if w >= 1.0 => 2,
        _ => 4,
    }
}

/// GCS → puntos APS: 15 - GCS actual
fn points_gcs(gcs: u8) -> u32 {
    (15u8.saturating_sub(gcs)) as u32
}

// ─────────────────────────────────────────────────────────────────────────────
// Puntos por edad
// ─────────────────────────────────────────────────────────────────────────────

fn points_edad(edad: u8) -> u32 {
    match edad {
        0..=44 => 0,
        45..=54 => 2,
        55..=64 => 3,
        65..=74 => 5,
        _ => 6,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Puntos por enfermedades crónicas
// ─────────────────────────────────────────────────────────────────────────────

fn points_cronicas(data: &ApacheIIData) -> u32 {
    let tiene_cronica = data.insuficiencia_hepatica
        || data.cardiovascular_severa
        || data.insuficiencia_respiratoria
        || data.insuficiencia_renal
        || data.inmunocomprometido;

    if !tiene_cronica {
        return 0;
    }
    // No quirúrgico o cirugía de emergencia → 5 pts; electiva → 2 pts
    if data.cirugia_no_operado {
        5
    } else {
        2
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Score total Apache II (0-71)
// ─────────────────────────────────────────────────────────────────────────────

pub fn calculate_apache_ii_score(data: &ApacheIIData) -> u32 {
    let aps = points_temperatura(data.temperatura)
        + points_pam(data.presion_arterial_media)
        + points_fc(data.frecuencia_cardiaca)
        + points_fr(data.frecuencia_respiratoria)
        + points_oxigenacion(data.fio2, data.pao2, data.a_ado2)
        + points_ph(data.ph_arterial)
        + points_sodio(data.sodio_serico)
        + points_potasio(data.potasio_serico)
        + points_creatinina(data.creatinina, data.falla_renal_aguda)
        + points_hematocrito(data.hematocrito)
        + points_leucocitos(data.leucocitos)
        + points_gcs(data.gcs_total);

    let age_pts = points_edad(data.edad);
    let chronic_pts = points_cronicas(data);

    aps + age_pts + chronic_pts
}

/// Componentes individuales del score (para mostrar desglose en UI)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApacheIIBreakdown {
    pub temperatura: u32,
    pub pam: u32,
    pub fc: u32,
    pub fr: u32,
    pub oxigenacion: u32,
    pub ph: u32,
    pub sodio: u32,
    pub potasio: u32,
    pub creatinina: u32,
    pub hematocrito: u32,
    pub leucocitos: u32,
    pub gcs_pts: u32,
    pub aps_total: u32,
    pub edad_pts: u32,
    pub cronicas_pts: u32,
    pub total: u32,
}

pub fn apache_ii_breakdown(data: &ApacheIIData) -> ApacheIIBreakdown {
    let temperatura = points_temperatura(data.temperatura);
    let pam = points_pam(data.presion_arterial_media);
    let fc = points_fc(data.frecuencia_cardiaca);
    let fr = points_fr(data.frecuencia_respiratoria);
    let oxigenacion = points_oxigenacion(data.fio2, data.pao2, data.a_ado2);
    let ph = points_ph(data.ph_arterial);
    let sodio = points_sodio(data.sodio_serico);
    let potasio = points_potasio(data.potasio_serico);
    let creatinina_p = points_creatinina(data.creatinina, data.falla_renal_aguda);
    let hematocrito = points_hematocrito(data.hematocrito);
    let leucocitos = points_leucocitos(data.leucocitos);
    let gcs_pts = points_gcs(data.gcs_total);
    let aps_total = temperatura
        + pam
        + fc
        + fr
        + oxigenacion
        + ph
        + sodio
        + potasio
        + creatinina_p
        + hematocrito
        + leucocitos
        + gcs_pts;
    let edad_pts = points_edad(data.edad);
    let cronicas_pts = points_cronicas(data);
    let total = aps_total + edad_pts + cronicas_pts;

    ApacheIIBreakdown {
        temperatura,
        pam,
        fc,
        fr,
        oxigenacion,
        ph,
        sodio,
        potasio,
        creatinina: creatinina_p,
        hematocrito,
        leucocitos,
        gcs_pts,
        aps_total,
        edad_pts,
        cronicas_pts,
        total,
    }
}

/// Estimación de riesgo de mortalidad hospitalaria basada en score Apache II
/// Curva derivada de los datos originales de Knaus et al. 1985 (aproximación)
pub fn mortality_risk(score: u32) -> f32 {
    match score {
        0..=4 => 4.0,
        5..=9 => 8.0,
        10..=14 => 15.0,
        15..=19 => 25.0,
        20..=24 => 40.0,
        25..=29 => 55.0,
        30..=34 => 73.0,
        _ => 85.0,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GCS helpers
// ─────────────────────────────────────────────────────────────────────────────

pub fn calculate_gcs_score(data: &GcsData) -> u8 {
    data.total()
}

pub fn calculate_gcs_score_from_total(total: u8) -> u8 {
    total.clamp(3, 15)
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Serialize, Deserialize};
use crate::models::*;

    fn normal_patient() -> ApacheIIData {
        ApacheIIData {
            temperatura: 37.0,
            presion_arterial_media: 90.0,
            presion_sistolica: 120.0,
            frecuencia_cardiaca: 80.0,
            frecuencia_respiratoria: 16.0,
            fio2: 0.21,
            pao2: Some(80.0),
            a_ado2: None,
            spo2: 98.0,
            ph_arterial: 7.40,
            sodio_serico: 140.0,
            potasio_serico: 4.0,
            creatinina: 1.0,
            falla_renal_aguda: false,
            bilirrubina: 0.8,
            hematocrito: 42.0,
            leucocitos: 8.0,
            plaquetas: 250.0,
            gcs_total: 15,
            edad: 40,
            insuficiencia_hepatica: false,
            cardiovascular_severa: false,
            insuficiencia_respiratoria: false,
            insuficiencia_renal: false,
            inmunocomprometido: false,
            cirugia_no_operado: false,
            ventilacion_mecanica: false,
            o2_suplementario: false,
            alerta: true,
            vasopresores: false,
            dosis_vasopresor: 0.0,
            diuresis_diaria: 1500,
            tipo_admision: None,
            fuente_admision: None,
            dias_pre_uci: 0,
            infeccion_admision: None,
            sistema_anatomico: None,
            bicarbonate: 24.0,
            nivel_conciencia: String::new(),
        }
    }

    fn critical_patient() -> ApacheIIData {
        let mut data = normal_patient();
        data.temperatura = 42.0;
        data.presion_arterial_media = 180.0;
        data.presion_sistolica = 200.0;
        data.frecuencia_cardiaca = 190.0;
        data.frecuencia_respiratoria = 55.0;
        data.ph_arterial = 7.10;
        data.sodio_serico = 185.0;
        data.potasio_serico = 7.5;
        data.creatinina = 4.0;
        data.hematocrito = 15.0;
        data.leucocitos = 45.0;
        data.plaquetas = 30.0;
        data.bilirrubina = 8.0;
        data.gcs_total = 3;
        data.edad = 75;
        data.inmunocomprometido = true;
        data.ventilacion_mecanica = true;
        data.vasopresores = true;
        data.dosis_vasopresor = 15.0;
        data.diuresis_diaria = 100;
        data.tipo_admision = Some("medical".to_string());
        data.fuente_admision = Some("emergency_room".to_string());
        data.dias_pre_uci = 2;
        data.infeccion_admision = Some("respiratory".to_string());
        data
    }

    #[test]
    fn test_normal_patient_low_score() {
        let data = normal_patient();
        let score = calculate_apache_ii_score(&data);
        assert!(
            score < 10,
            "Normal patient should score < 10, got: {}",
            score
        );
    }

    #[test]
    fn test_critical_patient_high_score() {
        let mut data = normal_patient();
        data.temperatura = 42.0; // +4
        data.presion_arterial_media = 180.0; // +4
        data.frecuencia_cardiaca = 190.0; // +4
        data.frecuencia_respiratoria = 55.0; // +4
        data.ph_arterial = 7.10; // +4
        data.sodio_serico = 185.0; // +4
        data.potasio_serico = 7.5; // +4
        data.creatinina = 4.0; // +4
        data.hematocrito = 15.0; // +4
        data.leucocitos = 45.0; // +4
        data.gcs_total = 3; // +12
        data.edad = 75; // +6
        data.insuficiencia_hepatica = true;
        data.cirugia_no_operado = true; // +5
        let score = calculate_apache_ii_score(&data);
        assert!(
            score >= 30,
            "Critical patient should score >= 30, got: {}",
            score
        );
    }

    #[test]
    fn test_gcs_total() {
        let gcs = GcsData {
            apertura_ocular: 4,
            respuesta_verbal: 5,
            respuesta_motora: 6,
        };
        assert_eq!(gcs.total(), 15);
        let gcs2 = GcsData {
            apertura_ocular: 1,
            respuesta_verbal: 1,
            respuesta_motora: 1,
        };
        assert_eq!(gcs2.total(), 3);
    }

    #[test]
    fn test_saps3_normal_patient() {
        let data = normal_patient();
        let score = calculate_saps_iii_score(&data);
        let mort = saps_iii_mortality_prediction(score);
        assert!(
            score < 30,
            "Normal patient SAPS3 should be < 30, got: {}",
            score
        );
        assert!(
            mort < 15.0,
            "Normal patient mortality should be < 15%, got: {}%",
            mort
        );
    }

    #[test]
    fn test_saps3_critical_patient() {
        let data = critical_patient();
        let score = calculate_saps_iii_score(&data);
        let mort = saps_iii_mortality_prediction(score);
        assert!(
            score >= 50,
            "Critical patient SAPS3 should be >= 50, got: {}",
            score
        );
        assert!(
            mort >= 30.0,
            "Critical patient mortality should be >= 30%, got: {}%",
            mort
        );
    }

    #[test]
    fn test_news2_normal_patient() {
        let data = normal_patient();
        let score = calculate_news2_score(&data);
        assert!(
            score < 5,
            "Normal patient NEWS2 should be < 5, got: {}",
            score
        );
    }

    #[test]
    fn test_news2_critical_patient() {
        let data = critical_patient();
        let score = calculate_news2_score(&data);
        assert!(
            score >= 10,
            "Critical patient NEWS2 should be >= 10, got: {}",
            score
        );
    }

    #[test]
    fn test_sofa_normal_patient() {
        let data = normal_patient();
        let score = calculate_sofa_score(&data);
        let mort = sofa_mortality_estimate(score);
        assert!(
            score < 5,
            "Normal patient SOFA should be < 5, got: {}",
            score
        );
        assert!(
            mort < 10.0,
            "Normal patient SOFA mortality should be < 10%, got: {}%",
            mort
        );
    }

    #[test]
    fn test_sofa_critical_patient() {
        let data = critical_patient();
        let score = calculate_sofa_score(&data);
        let mort = sofa_mortality_estimate(score);
        assert!(
            score >= 10,
            "Critical patient SOFA should be >= 10, got: {}",
            score
        );
        assert!(
            mort >= 30.0,
            "Critical patient mortality should be >= 30%, got: {}%",
            mort
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// SAPS III Scoring (0-104 points)
// ─────────────────────────────────────────────────────────────────────────────

fn points_saps_edad(edad: u8) -> u32 {
    match edad {
        0..=39 => 0,
        40..=59 => 6,
        60..=69 => 11,
        70..=74 => 13,
        75..=79 => 15,
        _ => 18,
    }
}

fn points_saps_comorbilidad(inmunocomprometido: bool) -> u32 {
    if inmunocomprometido {
        9
    } else {
        0
    }
}

fn points_saps_vasoactivos(vasoactivos: bool) -> u32 {
    if vasoactivos {
        8
    } else {
        0
    }
}

fn points_saps_fuente(fuente: Option<&str>) -> u32 {
    match fuente {
        Some("emergency_room") => 5,
        Some("ward") => 3,
        Some("other_icu") => 6,
        _ => 0,
    }
}

fn points_saps_dias_pre_uci(dias: u8) -> u32 {
    match dias {
        0 => 0,
        1..=1 => 2,
        2..=3 => 4,
        _ => 6,
    }
}

fn points_saps_tipo_admision(tipo: Option<&str>) -> u32 {
    match tipo {
        Some("medical") => 6,
        Some("unscheduled_surgical") => 8,
        Some("scheduled_surgical") => 0,
        _ => 0,
    }
}

fn points_saps_infeccion(inf: Option<&str>) -> u32 {
    match inf {
        Some("respiratory") => 5,
        Some("nosocomial") => 7,
        _ => 0,
    }
}

fn points_saps_gcs(gcs: u8) -> u32 {
    match gcs {
        15 => 0,
        13..=14 => 5,
        10..=12 => 8,
        6..=9 => 13,
        _ => 18,
    }
}

fn points_saps_fc(fc: f32) -> u32 {
    match fc {
        fc if fc >= 160.0 => 7,
        fc if fc >= 120.0 => 4,
        fc if fc >= 80.0 => 0,
        fc if fc >= 50.0 => 2,
        _ => 4,
    }
}

fn points_saps_pas(pas: f32) -> u32 {
    match pas {
        pas if pas >= 120.0 => 0,
        pas if pas >= 80.0 => 4,
        _ => 7,
    }
}

fn points_saps_temperatura(temp: f32) -> u32 {
    match temp {
        temp if temp >= 39.0 => 6,
        temp if temp >= 38.0 => 2,
        temp if temp >= 36.0 => 0,
        temp if temp >= 35.0 => 2,
        _ => 6,
    }
}

fn points_saps_bilirrubina(bili: f32) -> u32 {
    match bili {
        bili if bili >= 4.0 => 9,
        bili if bili >= 2.0 => 5,
        bili if bili >= 1.2 => 2,
        _ => 0,
    }
}

fn points_saps_creatinina(creat: f32) -> u32 {
    match creat {
        creat if creat >= 3.0 => 7,
        creat if creat >= 2.0 => 4,
        creat if creat >= 1.2 => 2,
        _ => 0,
    }
}

fn points_saps_leucocitos(wbc: f32) -> u32 {
    match wbc {
        w if w >= 20.0 => 5,
        w if w >= 12.0 => 2,
        w if w >= 4.0 => 0,
        _ => 3,
    }
}

fn points_saps_ph(ph: f32) -> u32 {
    match ph {
        ph if ph >= 7.50 => 0,
        ph if ph >= 7.33 => 3,
        ph if ph >= 7.25 => 5,
        _ => 9,
    }
}

fn points_saps_plaquetas(plq: f32) -> u32 {
    match plq {
        plq if plq >= 100.0 => 0,
        plq if plq >= 50.0 => 3,
        _ => 5,
    }
}

fn points_saps_oxigenacion(vm: bool, pao2fio2: f32) -> u32 {
    if vm {
        match pao2fio2 {
            r if r >= 300.0 => 0,
            r if r >= 200.0 => 4,
            r if r >= 100.0 => 7,
            _ => 10,
        }
    } else {
        0
    }
}

pub fn calculate_saps_iii_score(data: &ApacheIIData) -> u32 {
    let edad_pts = points_saps_edad(data.edad);
    let comorb_pts = points_saps_comorbilidad(data.inmunocomprometido);
    let vaso_pts = points_saps_vasoactivos(data.vasopresores);
    let fuente_pts = points_saps_fuente(data.fuente_admision.as_deref());
    let dias_pts = points_saps_dias_pre_uci(data.dias_pre_uci);
    let tipo_pts = points_saps_tipo_admision(data.tipo_admision.as_deref());
    let inf_pts = points_saps_infeccion(data.infeccion_admision.as_deref());

    let gcs_pts = points_saps_gcs(data.gcs_total);
    let fc_pts = points_saps_fc(data.frecuencia_cardiaca);
    let pas_pts = points_saps_pas(data.presion_sistolica);
    let temp_pts = points_saps_temperatura(data.temperatura);
    let bili_pts = points_saps_bilirrubina(data.bilirrubina);
    let creat_pts = points_saps_creatinina(data.creatinina);
    let wbc_pts = points_saps_leucocitos(data.leucocitos);
    let ph_pts = points_saps_ph(data.ph_arterial);
    let plq_pts = points_saps_plaquetas(data.plaquetas);

    let pao2 = data.pao2.unwrap_or(80.0);
    let pao2fio2 = if data.fio2 > 0.0 {
        pao2 / data.fio2
    } else {
        300.0
    };
    let ox_pts = points_saps_oxigenacion(data.ventilacion_mecanica, pao2fio2);

    let box1 = edad_pts + comorb_pts + vaso_pts + fuente_pts + dias_pts;
    let box2 = tipo_pts + inf_pts;
    let box3 = gcs_pts
        + fc_pts
        + pas_pts
        + temp_pts
        + bili_pts
        + creat_pts
        + wbc_pts
        + ph_pts
        + plq_pts
        + ox_pts;

    box1 + box2 + box3
}

pub fn saps_iii_mortality_prediction(score: u32) -> f32 {
    let logit = -7.7631 + 0.0847 * score as f32;
    let probability = 1.0 / (1.0 + (-logit).exp());
    (probability * 100.0).min(100.0).max(0.0)
}

// Saps3Breakdown and saps_iii_breakdown moved to consolidated section below

// ─────────────────────────────────────────────────────────────────────────────
// NEWS2 Scoring (0-64 points)
// ─────────────────────────────────────────────────────────────────────────────

fn points_news2_respiracion(fr: f32) -> u32 {
    match fr {
        fr if fr >= 25.0 => 3,
        fr if fr >= 21.0 => 2,
        fr if fr >= 12.0 => 1,
        fr if fr >= 9.0 => 0,
        _ => 2,
    }
}

fn points_news2_spo2(spo2: f32) -> u32 {
    match spo2 {
        s if s >= 96.0 => 0,
        s if s >= 94.0 => 1,
        s if s >= 92.0 => 2,
        s if s >= 88.0 => 3,
        _ => 3,
    }
}

fn points_news2_airway(escala2: bool, spo2: f32) -> u32 {
    if escala2 && spo2 < 92.0 {
        match spo2 {
            s if s >= 88.0 => 2,
            s if s >= 86.0 => 3,
            _ => 3,
        }
    } else {
        0
    }
}

fn points_news2_pas(pas: f32) -> u32 {
    match pas {
        p if p >= 220.0 => 3,
        p if p >= 110.0 => 0,
        p if p >= 100.0 => 2,
        p if p >= 90.0 => 3,
        _ => 3,
    }
}

fn points_news2_fc(fc: f32) -> u32 {
    match fc {
        f if f >= 130.0 => 3,
        f if f >= 110.0 => 2,
        f if f >= 50.0 => 0,
        f if f >= 40.0 => 1,
        _ => 3,
    }
}

fn points_news2_temperatura(temp: f32) -> u32 {
    match temp {
        t if t >= 39.0 => 2,
        t if t >= 38.0 => 1,
        t if t >= 36.0 => 0,
        t if t >= 35.0 => 1,
        _ => 2,
    }
}

fn points_news2_conciencia(gcs: u8) -> u32 {
    match gcs {
        15 => 0,
        14 => 1,
        10..=13 => 2,
        _ => 3,
    }
}

pub fn calculate_news2_score(data: &ApacheIIData) -> u32 {
    let fr_pts = points_news2_respiracion(data.frecuencia_respiratoria);
    let spo2_pts = points_news2_spo2(data.spo2);
    let airway_pts = points_news2_airway(false, data.spo2);
    let o2_pts = if data.o2_suplementario { 2 } else { 0 };
    let pas_pts = points_news2_pas(data.presion_sistolica);
    let fc_pts = points_news2_fc(data.frecuencia_cardiaca);
    let temp_pts = points_news2_temperatura(data.temperatura);
    let conciencia_pts = points_news2_conciencia(data.gcs_total);

    fr_pts + spo2_pts + airway_pts + o2_pts + pas_pts + fc_pts + temp_pts + conciencia_pts
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct News2Breakdown {
    pub fr: u32,
    pub spo2: u32,
    pub airway: u32,
    pub o2: u32,
    pub pas: u32,
    pub fc: u32,
    pub temp: u32,
    pub conciencia: u32,
    pub total: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SofaBreakdown {
    pub respiratorio: u32,
    pub coagulacion: u32,
    pub hepatico: u32,
    pub cardiovascular: u32,
    pub neurologico: u32,
    pub renal: u32,
    pub total: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Saps3Breakdown {
    pub box1: u32,
    pub box2: u32,
    pub box3: u32,
    pub total: u32,
}

pub fn sofa_breakdown(data: &ApacheIIData) -> SofaBreakdown {
    let pao2 = data.pao2.unwrap_or(80.0);
    let pao2fio2 = if data.fio2 > 0.0 { pao2 / data.fio2 } else { 400.0 };
    
    let r = points_sofa_respiratorio(pao2fio2, data.ventilacion_mecanica);
    let c = points_sofa_coagulacion(data.plaquetas);
    let h = points_sofa_hepatico(data.bilirrubina);
    let cv = points_sofa_cardiovascular(data.presion_arterial_media, data.vasopresores, data.dosis_vasopresor);
    let n = points_sofa_neurologico(data.gcs_total);
    let ren = points_sofa_renal(data.creatinina, data.diuresis_diaria);
    let total = r + c + h + cv + n + ren;
    
    SofaBreakdown { respiratorio: r, coagulacion: c, hepatico: h, cardiovascular: cv, neurologico: n, renal: ren, total }
}

pub fn calculate_saps3_breakdown(data: &ApacheIIData) -> Saps3Breakdown {
    // Simplified SAPS III logic for dashboard visualization
    let b1 = 0; // Pre-admission
    let b2 = 0; // Admission circumstances
    let b3 = calculate_saps_iii_score(data); // Acute physiology
    Saps3Breakdown { box1: b1, box2: b2, box3: b3, total: b1 + b2 + b3 }
}

pub fn news2_breakdown(data: &ApacheIIData) -> News2Breakdown {
    let fr = points_news2_respiracion(data.frecuencia_respiratoria);
    let spo2 = points_news2_spo2(data.spo2);
    let airway = points_news2_airway(false, data.spo2);
    let o2 = if data.o2_suplementario { 2 } else { 0 };
    let pas = points_news2_pas(data.presion_sistolica);
    let fc = points_news2_fc(data.frecuencia_cardiaca);
    let temp = points_news2_temperatura(data.temperatura);
    let conciencia = points_news2_conciencia(data.gcs_total);
    let total = fr + spo2 + airway + o2 + pas + fc + temp + conciencia;

    News2Breakdown {
        fr,
        spo2,
        airway,
        o2,
        pas,
        fc,
        temp,
        conciencia,
        total,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// SOFA Scoring (0-24 points)
// ─────────────────────────────────────────────────────────────────────────────

fn points_sofa_respiratorio(pao2fio2: f32, vm: bool) -> u32 {
    if vm {
        match pao2fio2 {
            r if r >= 400.0 => 0,
            r if r >= 300.0 => 1,
            r if r >= 200.0 => 2,
            r if r >= 100.0 => 3,
            _ => 4,
        }
    } else {
        match pao2fio2 {
            r if r >= 400.0 => 0,
            r if r >= 300.0 => 1,
            r if r >= 200.0 => 2,
            r if r >= 100.0 => 3,
            _ => 4,
        }
    }
}

fn points_sofa_coagulacion(plaquetas: f32) -> u32 {
    match plaquetas {
        p if p >= 150.0 => 0,
        p if p >= 100.0 => 1,
        p if p >= 50.0 => 2,
        p if p >= 20.0 => 3,
        _ => 4,
    }
}

fn points_sofa_hepatico(bilirrubina: f32) -> u32 {
    match bilirrubina {
        b if b >= 12.0 => 4,
        b if b >= 6.0 => 3,
        b if b >= 2.0 => 2,
        b if b >= 1.2 => 1,
        _ => 0,
    }
}

fn points_sofa_cardiovascular(pam: f32, vasoactivos: bool, dosis: f32) -> u32 {
    if pam >= 70.0 {
        if !vasoactivos {
            0
        } else {
            match dosis {
                d if d <= 0.1 => 2,
                d if d <= 5.0 => 2,
                _ => 3,
            }
        }
    } else {
        1
    }
}

fn points_sofa_neurologico(gcs: u8) -> u32 {
    match gcs {
        15 => 0,
        13..=14 => 1,
        10..=12 => 2,
        6..=9 => 3,
        _ => 4,
    }
}

fn points_sofa_renal(creatinina: f32, diuresis: u32) -> u32 {
    match (creatinina, diuresis) {
        (c, d) if c >= 5.0 || d < 200 => 4,
        (c, d) if c >= 3.5 || d < 500 => 3,
        (c, _) if c >= 2.0 => 2,
        (c, _) if c >= 1.2 => 1,
        _ => 0,
    }
}

pub fn calculate_sofa_score(data: &ApacheIIData) -> u32 {
    let pao2 = data.pao2.unwrap_or(80.0);
    let pao2fio2 = if data.fio2 > 0.0 {
        pao2 / data.fio2
    } else {
        400.0
    };

    let resp_pts = points_sofa_respiratorio(pao2fio2, data.ventilacion_mecanica);
    let coag_pts = points_sofa_coagulacion(data.plaquetas);
    let hep_pts = points_sofa_hepatico(data.bilirrubina);
    let cardio_pts = points_sofa_cardiovascular(
        data.presion_arterial_media,
        data.vasopresores,
        data.dosis_vasopresor,
    );
    let neuro_pts = points_sofa_neurologico(data.gcs_total);
    let renal_pts = points_sofa_renal(data.creatinina, data.diuresis_diaria);

    resp_pts + coag_pts + hep_pts + cardio_pts + neuro_pts + renal_pts
}

pub fn sofa_mortality_estimate(score: u32) -> f32 {
    match score {
        0..=6 => 7.0,
        7..=9 => 18.0,
        10..=12 => 45.0,
        13..=14 => 58.0,
        15..=17 => 73.0,
        _ => 90.0,
    }
}

// Redundant SofaBreakdown and sofa_breakdown removed
