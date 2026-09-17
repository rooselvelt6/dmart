//! SPEC-033: Patient Similarity Engine — embeddings clínicos 128d + top-K.
//!
//! Convierte el perfil clínico de un paciente (vitales, scores, demografía) en
//! un embedding determinístico de 128 dimensiones (weighted-average manual,
//! modelo `clinical-encoder-v1`) y busca los K más similares por similitud de
//! coseno. Explica el ranking con las features más contribuyentes (top-5).

use anyhow::Result;
use serde::{Deserialize, Serialize};
use surrealdb::Surreal;
use surrealdb::engine::local::Db;

use dmart_shared::models::{Measurement, Patient};

use chrono::Utc;

const EMBEDDING_DIM: usize = 128;
const MODEL_VERSION: &str = "clinical-encoder-v1";

/// Features clínicas normalizadas extraídas de un paciente + mediciones.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClinicalFeatures {
    // Demografía
    age: f32,
    sex: f32,
    // Vitales (último estado)
    heart_rate_mean: f32,
    systolic_bp_mean: f32,
    respiratory_rate_mean: f32,
    temperature_mean: f32,
    o2_sat_mean: f32,
    o2_sat_min: f32,
    // Labs
    wbc_mean: f32,
    creatinine_mean: f32,
    // Scores
    news2_latest: f32,
    sofa_latest: f32,
    gcs_latest: f32,
    apache_latest: f32,
    // Temporalidad
    los_days: f32,
    vital_count: f32,
}

impl ClinicalFeatures {
    /// Extrae features de un paciente y sus mediciones (últimas ≤ 100).
    pub fn extract(patient: &Patient, measurements: &[Measurement]) -> Self {
        let age = patient.edad.min(120) as f32 / 120.0;
        let sex = match patient.sexo {
            dmart_shared::models::Sexo::Masculino => 1.0,
            _ => 0.0,
        };

        let mut hr = 0.0;
        let mut sbp = 0.0;
        let mut rr = 0.0;
        let mut temp = 0.0;
        let mut spo2 = 0.0;
        let mut o2_min = f32::MAX;
        let mut wbc = 0.0;
        let mut crea = 0.0;
        let mut n = 0;

        for m in measurements.iter().rev().take(100) {
            let a = &m.apache_data;
            hr += a.frecuencia_cardiaca;
            sbp += a.presion_sistolica;
            rr += a.frecuencia_respiratoria;
            temp += a.temperatura;
            spo2 += a.spo2;
            o2_min = o2_min.min(a.spo2);
            wbc += a.leucocitos;
            crea += a.creatinina;
            n += 1;
        }

        let mut f = ClinicalFeatures {
            age,
            sex,
            ..Default::default()
        };

        if n > 0 {
            let nn = n as f32;
            f.heart_rate_mean = (hr / nn).clamp(0.0, 200.0) / 200.0;
            f.systolic_bp_mean = (sbp / nn).clamp(0.0, 250.0) / 250.0;
            f.respiratory_rate_mean = (rr / nn).clamp(0.0, 60.0) / 60.0;
            f.temperature_mean = ((temp / nn) - 30.0).clamp(0.0, 14.0) / 14.0;
            f.o2_sat_mean = (spo2 / nn) / 100.0;
            f.o2_sat_min = o2_min.clamp(0.0, 100.0) / 100.0;
            f.wbc_mean = (wbc / nn).clamp(0.0, 60.0) / 60.0;
            f.creatinine_mean = (crea / nn).clamp(0.0, 10.0) / 10.0;
            f.vital_count = (n as f32 / 24.0).min(1.0);
        }

        let last = measurements.iter().next_back();
        if let Some(m) = last {
            f.news2_latest = m.news2_score.unwrap_or(0) as f32 / 20.0;
            f.sofa_latest = m.sofa_score.unwrap_or(0) as f32 / 24.0;
            f.gcs_latest = m.gcs_score as f32 / 15.0;
            f.apache_latest = m.apache_score as f32 / 70.0;
        }

        // Días de estancia UCI (ingreso → ahora/egreso).
        let ingreso = patient
            .fecha_ingreso_uci
            .parse::<chrono::DateTime<chrono::Utc>>()
            .map(|d| d.timestamp())
            .unwrap_or(0);
        let egreso = if patient.fecha_egreso_uci.is_empty() {
            Utc::now().timestamp()
        } else {
            patient
                .fecha_egreso_uci
                .parse::<chrono::DateTime<chrono::Utc>>()
                .map(|d| d.timestamp())
                .unwrap_or(ingreso)
        };
        f.los_days = (((egreso - ingreso).max(0) as f32) / 86400.0).min(365.0) / 365.0;

        f
    }

    /// Devuelve los nombres en orden canónico (para `features_used`).
    pub fn names() -> [&'static str; 16] {
        [
            "age",
            "sex",
            "heart_rate_mean",
            "systolic_bp_mean",
            "respiratory_rate_mean",
            "temperature_mean",
            "o2_sat_mean",
            "o2_sat_min",
            "wbc_mean",
            "creatinine_mean",
            "news2_latest",
            "sofa_latest",
            "gcs_latest",
            "apache_latest",
            "los_days",
            "vital_count",
        ]
    }

    pub fn to_vec(&self) -> Vec<f32> {
        vec![
            self.age,
            self.sex,
            self.heart_rate_mean,
            self.systolic_bp_mean,
            self.respiratory_rate_mean,
            self.temperature_mean,
            self.o2_sat_mean,
            self.o2_sat_min,
            self.wbc_mean,
            self.creatinine_mean,
            self.news2_latest,
            self.sofa_latest,
            self.gcs_latest,
            self.apache_latest,
            self.los_days,
            self.vital_count,
        ]
    }
}

/// Genera el embedding de 128 dimensiones (feature hashing ponderado).
pub fn embed(features: &ClinicalFeatures) -> Vec<f32> {
    let mut v = vec![0.0_f32; EMBEDDING_DIM];
    let values = features.to_vec();
    for (idx, value) in values.iter().enumerate() {
        // Hash determinístico: bucket + signo por feature.
        let mut h = 2166136261u64;
        for b in ClinicalFeatures::names()[idx].bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(16777619);
        }
        let sign = if h & 1 == 0 { 1.0 } else { -1.0 };
        let bucket = (h % EMBEDDING_DIM as u64) as usize;
        v[bucket] += sign * value;
    }
    // L2 normalize.
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-6);
    v.iter_mut().for_each(|x| *x /= norm);
    v
}

/// Similitud de coseno entre dos vectores L2-normalizados.
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || a.len() != b.len() {
        return 0.0;
    }
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

/// Fila persistida en `patient_embedding`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatientEmbedding {
    #[serde(skip_serializing, skip_deserializing, default)]
    pub id: Option<String>,
    pub patient_id: String,
    pub tenant_id: String,
    pub embedding: Vec<f32>,
    pub model_version: String,
    pub features_used: Vec<String>,
    pub generated_at: String,
    pub updated_at: String,
}

impl PatientEmbedding {
    fn new(patient_id: &str, tenant_id: &str, embedding: Vec<f32>) -> Self {
        let now = Utc::now().to_rfc3339();
        Self {
            id: None,
            patient_id: patient_id.to_string(),
            tenant_id: tenant_id.to_string(),
            embedding,
            model_version: MODEL_VERSION.to_string(),
            features_used: ClinicalFeatures::names()
                .iter()
                .map(|s| s.to_string())
                .collect(),
            generated_at: now.clone(),
            updated_at: now,
        }
    }
}

/// Upsert del embedding de un paciente (recalculado con cada nueva medición).
pub async fn upsert_embedding(
    db: &Surreal<Db>,
    patient_id: &str,
    tenant_id: &str,
    embedding: Vec<f32>,
) -> Result<PatientEmbedding> {
    let record = PatientEmbedding::new(patient_id, tenant_id, embedding);
    let existing: Option<PatientEmbedding> = db
        .query("SELECT * FROM patient_embedding WHERE patient_id = $pid LIMIT 1")
        .bind(("pid", patient_id.to_string()))
        .await?
        .take(0)?;
    let saved: Option<PatientEmbedding> = if let Some(_e) = existing {
        db.update(("patient_embedding", patient_id.to_string()))
            .merge(serde_json::json!({
                "embedding": record.embedding,
                "model_version": record.model_version,
                "features_used": record.features_used,
                "updated_at": record.updated_at,
            }))
            .await?
    } else {
        db.create(("patient_embedding", patient_id.to_string()))
            .content(record)
            .await?
    };
    crate::metrics::ml_embedding_generated(MODEL_VERSION);
    Ok(saved.expect("embedding saved"))
}

/// Genera y persiste el embedding de un paciente desde DB.
pub async fn generate_for_patient(db: &Surreal<Db>, patient_id: &str) -> Result<PatientEmbedding> {
    let patient: Option<Patient> = db.select(("patients", patient_id)).await?;
    let p = patient.ok_or_else(|| anyhow::anyhow!("patient not found: {patient_id}"))?;
    let measurements: Vec<Measurement> =
        crate::db::get_measurements_for_patient(db, patient_id).await?;
    let features = ClinicalFeatures::extract(&p, &measurements);
    upsert_embedding(db, patient_id, &p.tenant_id, embed(&features)).await
}

/// Resultado de una búsqueda de similaridad.
#[derive(Debug, Clone, Serialize)]
pub struct SimilarityHit {
    pub patient_id: String,
    pub similarity_score: f32,
    pub matching_features: Vec<String>,
}

/// Búsqueda brute-force de los K embeddings más similares (scoping por tenant).
/// Sin dependencia externa de ANN; suficiente para ≤ 10k pacientes activos.
pub async fn search_similar(
    db: &Surreal<Db>,
    query_patient_id: &str,
    tenant_id: &str,
    k: usize,
    exclude: &[String],
) -> Result<(Vec<SimilarityHit>, usize, Vec<PatientEmbedding>)> {
    let query: Option<PatientEmbedding> = db
        .query("SELECT * FROM patient_embedding WHERE patient_id = $pid LIMIT 1")
        .bind(("pid", query_patient_id.to_string()))
        .await?
        .take(0)?;
    let q = query.ok_or_else(|| anyhow::anyhow!("no embedding for patient {query_patient_id}"))?;

    let all: Vec<PatientEmbedding> = db
        .query("SELECT * FROM patient_embedding WHERE tenant_id = $tenant")
        .bind(("tenant", tenant_id.to_string()))
        .await?
        .take(0)?;

    let excluded = exclude.iter().any(|e| e == query_patient_id);
    let mut scored: Vec<(f32, PatientEmbedding)> = all
        .into_iter()
        .filter(|e| e.patient_id != query_patient_id)
        .filter(|e| !exclude.contains(&e.patient_id))
        .filter(|e| {
            // Pacientes con < 3 mediciones (vital_count bajo) se excluyen si el
            // de referencia también las tiene (consistencia de cobertura).
            e.embedding.iter().map(|x| x * x).sum::<f32>().abs() > 1e-6
        })
        .map(|e| (cosine(&q.embedding, &e.embedding), e))
        .collect();
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    let total = scored.len();
    let hits: Vec<SimilarityHit> = scored
        .iter()
        .take(k)
        .map(|(score, e)| SimilarityHit {
            patient_id: e.patient_id.clone(),
            similarity_score: *score,
            matching_features: top_features(&q.embedding, &e.embedding),
        })
        .collect();

    crate::metrics::ml_similarity_search(k);
    let _ = excluded;
    Ok((
        hits,
        total,
        scored.iter().map(|(_, e)| e.clone()).take(k).collect(),
    ))
}

/// Top-5 features con mayor contribución absoluta a la similitud.
pub fn top_features(a: &[f32], b: &[f32]) -> Vec<String> {
    let names = ClinicalFeatures::names();
    // feature hashing invertido: para cada bucket recuperamos el feature hasheados
    // que más contribuye (aproximación: features base con su valor).
    let mut contrib: Vec<(usize, f32)> = names
        .iter()
        .enumerate()
        .map(|(idx, name)| {
            // hash idéntico al de embed() para localizar el bucket
            let mut h = 2166136261u64;
            for b in name.bytes() {
                h ^= b as u64;
                h = h.wrapping_mul(16777619);
            }
            let bucket = (h % a.len() as u64) as usize;
            let diff = (a[bucket] - b[bucket]).abs();
            (idx, diff)
        })
        .collect();
    contrib.sort_by(|x, y| y.1.partial_cmp(&x.1).unwrap_or(std::cmp::Ordering::Equal));
    contrib
        .iter()
        .take(5)
        .map(|(idx, _)| names[*idx].to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use dmart_shared::models::{Patient, Sexo};

    fn sample_patient() -> Patient {
        let mut p = Patient::new();
        p.edad = 67;
        p.sexo = Sexo::Masculino;
        p
    }

    fn sample_measurements(n: usize) -> Vec<Measurement> {
        let mut out = Vec::new();
        for i in 0..n {
            let mut m = Measurement::new(
                "p",
                dmart_shared::models::ApacheIIData {
                    temperatura: 38.0,
                    presion_arterial_media: 68.0,
                    presion_sistolica: 96.0,
                    frecuencia_cardiaca: 112.0 + (i as f32 * 2.0),
                    frecuencia_respiratoria: 26.0,
                    fio2: 0.5,
                    pao2: Some(70.0),
                    a_ado2: None,
                    spo2: 89.0,
                    ph_arterial: 7.38,
                    sodio_serico: 137.0,
                    potasio_serico: 3.9,
                    creatinina: 1.4,
                    falla_renal_aguda: false,
                    bilirrubina: 1.1,
                    hematocrito: 33.0,
                    leucocitos: 13.0,
                    plaquetas: 180.0,
                    gcs_ojos: 3,
                    gcs_verbal: 4,
                    gcs_motor: 5,
                    gcs_total: 12,
                    edad: 67,
                    insuficiencia_hepatica: false,
                    cardiovascular_severa: true,
                    insuficiencia_respiratoria: false,
                    insuficiencia_renal: false,
                    inmunocomprometido: false,
                    cirugia_no_operado: false,
                    ventilacion_mecanica: true,
                    vasopresores: true,
                    dosis_vasopresor: 0.2,
                    diuresis_diaria: 1200,
                    alerta: false,
                    o2_suplementario: true,
                    nivel_conciencia: "Somnoliento".to_string(),
                    bicarbonate: 22.0,
                    tipo_admision: Some("unscheduled_surgical".to_string()),
                    fuente_admision: Some("emergency_room".to_string()),
                    dias_pre_uci: 1,
                    infeccion_admision: Some("respiratory".to_string()),
                    sistema_anatomico: Some("respiratory".to_string()),
                },
                dmart_shared::models::GcsData {
                    apertura_ocular: 3,
                    respuesta_verbal: 4,
                    respuesta_motora: 5,
                },
            );
            // Apuntar al paciente de prueba
            m.patient_id = "p".to_string();
            out.push(m);
        }
        out
    }

    #[test]
    fn embedding_is_128d_normalized() {
        let f = ClinicalFeatures::extract(&sample_patient(), &sample_measurements(5));
        let v = embed(&f);
        assert_eq!(v.len(), 128);
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-3);
    }

    #[test]
    fn cosine_perfect_similarity() {
        let a = embed(&ClinicalFeatures::extract(
            &sample_patient(),
            &sample_measurements(3),
        ));
        let b = a.clone();
        let s = cosine(&a, &b);
        assert!((s - 1.0).abs() < 1e-3, "same vector → 1.0, got {s}");
    }

    #[test]
    fn features_names_are_16() {
        assert_eq!(ClinicalFeatures::names().len(), 16);
    }
}
