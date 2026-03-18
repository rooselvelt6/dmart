/// Apache II scoring algorithm — fully implemented per KNAUS et al. (1985)
/// Reference: Knaus WA, Draper EA, Wagner DP, Zimmerman JE (1985).
/// APACHE II: a severity of disease classification system.
/// Crit Care Med. 13(10):818-29.
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
        p if p >= 70.0  => 0,
        p if p >= 50.0  => 2,
        _ => 4,
    }
}

/// Frecuencia cardíaca (lpm)
fn points_fc(fc: f32) -> u32 {
    match fc {
        fc if fc >= 180.0 => 4,
        fc if fc >= 140.0 => 3,
        fc if fc >= 110.0 => 2,
        fc if fc >= 70.0  => 0,
        fc if fc >= 55.0  => 2,
        fc if fc >= 40.0  => 3,
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
        fr if fr >= 6.0  => 2,
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
    if falla_aguda { pts * 2 } else { pts }
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
        w if w >= 3.0  => 0,
        w if w >= 1.0  => 2,
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
        0..=44  => 0,
        45..=54 => 2,
        55..=64 => 3,
        65..=74 => 5,
        _       => 6,
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
    if data.cirugia_no_operado { 5 } else { 2 }
}

// ─────────────────────────────────────────────────────────────────────────────
// Score total Apache II (0-71)
// ─────────────────────────────────────────────────────────────────────────────

pub fn calculate_apache_ii_score(data: &ApacheIIData) -> u32 {
    let aps =
        points_temperatura(data.temperatura)
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
#[derive(Debug, Clone)]
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
    let temperatura   = points_temperatura(data.temperatura);
    let pam           = points_pam(data.presion_arterial_media);
    let fc            = points_fc(data.frecuencia_cardiaca);
    let fr            = points_fr(data.frecuencia_respiratoria);
    let oxigenacion   = points_oxigenacion(data.fio2, data.pao2, data.a_ado2);
    let ph            = points_ph(data.ph_arterial);
    let sodio         = points_sodio(data.sodio_serico);
    let potasio       = points_potasio(data.potasio_serico);
    let creatinina_p  = points_creatinina(data.creatinina, data.falla_renal_aguda);
    let hematocrito   = points_hematocrito(data.hematocrito);
    let leucocitos    = points_leucocitos(data.leucocitos);
    let gcs_pts       = points_gcs(data.gcs_total);
    let aps_total     = temperatura + pam + fc + fr + oxigenacion + ph
                        + sodio + potasio + creatinina_p + hematocrito + leucocitos + gcs_pts;
    let edad_pts      = points_edad(data.edad);
    let cronicas_pts  = points_cronicas(data);
    let total         = aps_total + edad_pts + cronicas_pts;

    ApacheIIBreakdown {
        temperatura, pam, fc, fr, oxigenacion, ph, sodio, potasio,
        creatinina: creatinina_p, hematocrito, leucocitos, gcs_pts,
        aps_total, edad_pts, cronicas_pts, total,
    }
}

/// Estimación de riesgo de mortalidad hospitalaria basada en score Apache II
/// Curva derivada de los datos originales de Knaus et al. 1985 (aproximación)
pub fn mortality_risk(score: u32) -> f32 {
    match score {
        0..=4   => 4.0,
        5..=9   => 8.0,
        10..=14 => 15.0,
        15..=19 => 25.0,
        20..=24 => 40.0,
        25..=29 => 55.0,
        30..=34 => 73.0,
        _       => 85.0,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GCS helpers
// ─────────────────────────────────────────────────────────────────────────────

pub fn calculate_gcs_score(data: &GcsData) -> u8 {
    data.total()
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;

    fn normal_patient() -> ApacheIIData {
        ApacheIIData {
            temperatura: 37.0,
            presion_arterial_media: 90.0,
            frecuencia_cardiaca: 80.0,
            frecuencia_respiratoria: 16.0,
            fio2: 0.21,
            pao2: Some(80.0),
            a_ado2: None,
            ph_arterial: 7.40,
            sodio_serico: 140.0,
            potasio_serico: 4.0,
            creatinina: 1.0,
            falla_renal_aguda: false,
            hematocrito: 42.0,
            leucocitos: 8.0,
            gcs_total: 15,
            edad: 40,
            insuficiencia_hepatica: false,
            cardiovascular_severa: false,
            insuficiencia_respiratoria: false,
            insuficiencia_renal: false,
            inmunocomprometido: false,
            cirugia_no_operado: false,
        }
    }

    #[test]
    fn test_normal_patient_low_score() {
        let data = normal_patient();
        let score = calculate_apache_ii_score(&data);
        assert!(score < 10, "Normal patient should score < 10, got: {}", score);
    }

    #[test]
    fn test_critical_patient_high_score() {
        let mut data = normal_patient();
        data.temperatura = 42.0;        // +4
        data.presion_arterial_media = 180.0; // +4
        data.frecuencia_cardiaca = 190.0;    // +4
        data.frecuencia_respiratoria = 55.0; // +4
        data.ph_arterial = 7.10;        // +4
        data.sodio_serico = 185.0;      // +4
        data.potasio_serico = 7.5;      // +4
        data.creatinina = 4.0;          // +4
        data.hematocrito = 15.0;        // +4
        data.leucocitos = 45.0;         // +4
        data.gcs_total = 3;             // +12
        data.edad = 75;                 // +6
        data.insuficiencia_hepatica = true;
        data.cirugia_no_operado = true; // +5
        let score = calculate_apache_ii_score(&data);
        assert!(score >= 30, "Critical patient should score >= 30, got: {}", score);
    }

    #[test]
    fn test_gcs_total() {
        let gcs = GcsData { apertura_ocular: 4, respuesta_verbal: 5, respuesta_motora: 6 };
        assert_eq!(gcs.total(), 15);
        let gcs2 = GcsData { apertura_ocular: 1, respuesta_verbal: 1, respuesta_motora: 1 };
        assert_eq!(gcs2.total(), 3);
    }
}
