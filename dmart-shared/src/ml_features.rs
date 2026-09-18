//! SPEC-037: Feature Store Versionado — features reproductibles y versionadas para ML
//!
//! Define FeatureSet con versionado semántico, hash de integridad, extractores
//! determinísticos y normalización fit/transform para reproducibilidad total.

use std::collections::HashMap;

use chrono::{DateTime, TimeDelta, Utc};
use ndarray::{Array1, Array2};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::models::Patient;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeatureDtype {
    Float32,
    Int32,
    Bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureStats {
    pub mean: f32,
    pub std: f32,
    pub min: f32,
    pub max: f32,
}

impl FeatureStats {
    pub fn from_slice(values: &[f32]) -> Self {
        let n = values.len() as f32;
        let mean = values.iter().sum::<f32>() / n;
        let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / n;
        let std = variance.sqrt().max(1e-6);
        let min = values.iter().fold(f32::INFINITY, |a, &b| a.min(b));
        let max = values.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
        Self { mean, std, min, max }
    }

    pub fn normalize(&self, value: f32) -> f32 {
        (value - self.mean) / self.std
    }

    pub fn denormalize(&self, value: f32) -> f32 {
        value * self.std + self.mean
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureDef {
    pub name: String,
    pub dtype: FeatureDtype,
    pub description: String,
    pub transformation: Option<String>,
    pub stats: Option<FeatureStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureSet {
    pub version: Version,
    pub name: String,
    pub features: Vec<FeatureDef>,
    pub hash: String,
    pub created_at: DateTime<Utc>,
    pub description: String,
}

impl FeatureSet {
    pub fn new(name: &str, version: &str, features: Vec<FeatureDef>, description: &str) -> Self {
        let version = Version::parse(version).expect("valid semver");
        let mut fs = Self {
            version,
            name: name.to_string(),
            features,
            hash: String::new(),
            created_at: Utc::now(),
            description: description.to_string(),
        };
        fs.hash = fs.compute_hash();
        fs
    }

    fn compute_hash(&self) -> String {
        let mut hasher = Sha256::new();
        let content = format!(
            "{}|{}|{}",
            self.name,
            self.version,
            self.features
                .iter()
                .map(|f| format!("{}:{:?}:{}", f.name, f.dtype, f.transformation.as_deref().unwrap_or("")))
                .collect::<Vec<_>>()
                .join("|")
        );
        hasher.update(content.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    pub fn verify_hash(&self) -> bool {
        self.hash == self.compute_hash()
    }

    pub fn feature_names(&self) -> Vec<String> {
        self.features.iter().map(|f| f.name.clone()).collect()
    }

    pub fn feature_count(&self) -> usize {
        self.features.len()
    }

    pub fn get_stats(&self, name: &str) -> Option<&FeatureStats> {
        self.features.iter().find(|f| f.name == name).and_then(|f| f.stats.as_ref())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Normalizer {
    pub feature_stats: HashMap<String, FeatureStats>,
    pub feature_order: Vec<String>,
}

impl Normalizer {
    pub fn new() -> Self {
        Self {
            feature_stats: HashMap::new(),
            feature_order: Vec::new(),
        }
    }

    pub fn fit(&mut self, data: &Array2<f32>, feature_names: &[String]) {
        self.feature_order = feature_names.to_vec();
        for (i, name) in feature_names.iter().enumerate() {
            let col = data.column(i);
            let values: Vec<f32> = col.iter().cloned().collect();
            self.feature_stats.insert(name.clone(), FeatureStats::from_slice(&values));
        }
    }

    pub fn transform(&self, data: &mut Array2<f32>) {
        for (i, name) in self.feature_order.iter().enumerate() {
            if let Some(stats) = self.feature_stats.get(name) {
                for val in data.column_mut(i).iter_mut() {
                    *val = stats.normalize(*val);
                }
            }
        }
    }

    pub fn transform_vector(&self, vec: &mut Array1<f32>) {
        for (i, name) in self.feature_order.iter().enumerate() {
            if let Some(stats) = self.feature_stats.get(name) {
                vec[i] = stats.normalize(vec[i]);
            }
        }
    }

    pub fn save_json(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn load_json(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let json = std::fs::read_to_string(path)?;
        let normalizer: Self = serde_json::from_str(&json)?;
        Ok(normalizer)
    }
}

impl Default for Normalizer {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeWindow {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

impl TimeWindow {
    pub fn new(start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        Self { start, end }
    }

    pub fn last_hours(hours: i64) -> Self {
        let end = Utc::now();
        let start = end - TimeDelta::hours(hours);
        Self { start, end }
    }

    pub fn duration_hours(&self) -> i64 {
        (self.end - self.start).num_hours()
    }
}

pub trait FeatureExtractor: Send + Sync {
    fn feature_set(&self) -> &FeatureSet;
    fn extract(&self, patient_id: &str, window: TimeWindow) -> Result<MlFeatures, Box<dyn std::error::Error>>;
    fn extract_batch(&self, patient_ids: &[&str], window: TimeWindow) -> Result<Vec<MlFeatures>, Box<dyn std::error::Error>>;
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MlFeatures {
    pub data: HashMap<String, f32>,
    pub feature_set_hash: Option<String>,
    pub extracted_at: Option<DateTime<Utc>>,
}

impl MlFeatures {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, key: String, value: f32) -> Option<f32> {
        self.data.insert(key, value)
    }

    pub fn get(&self, key: &str) -> f32 {
        self.data.get(key).copied().unwrap_or(0.0)
    }

    pub fn to_array(&self, feature_order: &[String]) -> Array1<f32> {
        Array1::from_vec(feature_order.iter().map(|f| self.get(f)).collect())
    }

    pub fn validate_against(&self, feature_set: &FeatureSet) -> Result<(), String> {
        if let Some(hash) = &self.feature_set_hash {
            if hash != &feature_set.hash {
                return Err(format!("Feature set hash mismatch: expected {}, got {}", feature_set.hash, hash));
            }
        }
        for fdef in &feature_set.features {
            if !self.data.contains_key(&fdef.name) {
                return Err(format!("Missing feature: {}", fdef.name));
            }
        }
        Ok(())
    }
}

pub fn build_los_nn_v1() -> FeatureSet {
    let mut features = Vec::new();

    let base_vitals = [
        "heart_rate", "respiratory_rate", "spo2", "temperature",
        "systolic_bp", "diastolic_bp", "mean_arterial_pressure",
        "gcs_total", "fio2", "ph", "lactate", "creatinine",
        "bun", "sodium", "potassium", "chloride", "bicarbonate",
        "hemoglobin", "wbc", "platelets", "glucose", "bilirubin",
        "albumin", "inr", "pt", "ptt", "urine_output",
    ];

    for vital in base_vitals {
        features.push(FeatureDef {
            name: vital.to_string(),
            dtype: FeatureDtype::Float32,
            description: format!("Current value of {}", vital),
            transformation: None,
            stats: None,
        });

        for window_h in [1, 6, 12, 24] {
            features.push(FeatureDef {
                name: format!("{}_rolling_mean_{}h", vital, window_h),
                dtype: FeatureDtype::Float32,
                description: format!("Rolling mean of {} over {}h", vital, window_h),
                transformation: Some(format!("rolling_mean(window={}h)", window_h)),
                stats: None,
            });
            features.push(FeatureDef {
                name: format!("{}_rolling_std_{}h", vital, window_h),
                dtype: FeatureDtype::Float32,
                description: format!("Rolling std of {} over {}h", vital, window_h),
                transformation: Some(format!("rolling_std(window={}h)", window_h)),
                stats: None,
            });
            features.push(FeatureDef {
                name: format!("{}_rolling_min_{}h", vital, window_h),
                dtype: FeatureDtype::Float32,
                description: format!("Rolling min of {} over {}h", vital, window_h),
                transformation: Some(format!("rolling_min(window={}h)", window_h)),
                stats: None,
            });
            features.push(FeatureDef {
                name: format!("{}_rolling_max_{}h", vital, window_h),
                dtype: FeatureDtype::Float32,
                description: format!("Rolling max of {} over {}h", vital, window_h),
                transformation: Some(format!("rolling_max(window={}h)", window_h)),
                stats: None,
            });
            features.push(FeatureDef {
                name: format!("{}_delta_{}h", vital, window_h),
                dtype: FeatureDtype::Float32,
                description: format!("Delta of {} over {}h", vital, window_h),
                transformation: Some(format!("delta(window={}h)", window_h)),
                stats: None,
            });
            features.push(FeatureDef {
                name: format!("{}_trend_{}h", vital, window_h),
                dtype: FeatureDtype::Float32,
                description: format!("Linear trend slope of {} over {}h", vital, window_h),
                transformation: Some(format!("linear_trend(window={}h)", window_h)),
                stats: None,
            });
        }
    }

    features.push(FeatureDef {
        name: "age".to_string(),
        dtype: FeatureDtype::Float32,
        description: "Patient age in years".to_string(),
        transformation: None,
        stats: None,
    });
    features.push(FeatureDef {
        name: "sex_male".to_string(),
        dtype: FeatureDtype::Bool,
        description: "Sex: 1=male, 0=female".to_string(),
        transformation: None,
        stats: None,
    });
    features.push(FeatureDef {
        name: "admission_type_emergency".to_string(),
        dtype: FeatureDtype::Bool,
        description: "Emergency admission".to_string(),
        transformation: None,
        stats: None,
    });
    features.push(FeatureDef {
        name: "comorbidity_count".to_string(),
        dtype: FeatureDtype::Int32,
        description: "Number of comorbidities".to_string(),
        transformation: None,
        stats: None,
    });

    features.push(FeatureDef {
        name: "hour_sin".to_string(),
        dtype: FeatureDtype::Float32,
        description: "Sin(hour * 2π/24)".to_string(),
        transformation: Some("cyclical_encoding(hour, 24)".to_string()),
        stats: None,
    });
    features.push(FeatureDef {
        name: "hour_cos".to_string(),
        dtype: FeatureDtype::Float32,
        description: "Cos(hour * 2π/24)".to_string(),
        transformation: Some("cyclical_encoding(hour, 24)".to_string()),
        stats: None,
    });
    features.push(FeatureDef {
        name: "day_of_week_sin".to_string(),
        dtype: FeatureDtype::Float32,
        description: "Sin(day_of_week * 2π/7)".to_string(),
        transformation: Some("cyclical_encoding(day_of_week, 7)".to_string()),
        stats: None,
    });
    features.push(FeatureDef {
        name: "day_of_week_cos".to_string(),
        dtype: FeatureDtype::Float32,
        description: "Cos(day_of_week * 2π/7)".to_string(),
        transformation: Some("cyclical_encoding(day_of_week, 7)".to_string()),
        stats: None,
    });

    features.push(FeatureDef {
        name: "missingness_ratio".to_string(),
        dtype: FeatureDtype::Float32,
        description: "Fraction of missing vital signs in window".to_string(),
        transformation: Some("missingness_ratio".to_string()),
        stats: None,
    });
    features.push(FeatureDef {
        name: "measurement_count".to_string(),
        dtype: FeatureDtype::Int32,
        description: "Number of measurements in window".to_string(),
        transformation: Some("count".to_string()),
        stats: None,
    });

    FeatureSet::new(
        "los_nn_v1",
        "1.0.0",
        features,
        "Feature set for LOS-NN: temporal vital signs with rolling windows, deltas, trends, demographics, and cyclical time encoding",
    )
}

pub fn build_mortality_v2() -> FeatureSet {
    let mut features = Vec::new();

    let apache_components = [
        ("apache_score", "APACHE II total score"),
        ("gcs_total", "Glasgow Coma Scale total"),
        ("age", "Age in years"),
        ("temperature", "Temperature (Celsius)"),
        ("mean_arterial_pressure", "Mean arterial pressure (mmHg)"),
        ("heart_rate", "Heart rate (bpm)"),
        ("respiratory_rate", "Respiratory rate (breaths/min)"),
        ("fio2", "FiO2 fraction"),
        ("spo2", "SpO2 (%)"),
        ("ph", "Arterial pH"),
        ("sodium", "Serum sodium (mEq/L)"),
        ("creatinine", "Serum creatinine (mg/dL)"),
        ("wbc", "White blood cell count (K/uL)"),
        ("potassium", "Serum potassium (mEq/L)"),
        ("bun", "Blood urea nitrogen (mg/dL)"),
        ("hematocrit", "Hematocrit (%)"),
        ("albumin", "Serum albumin (g/dL)"),
        ("bilirubin", "Total bilirubin (mg/dL)"),
        ("glucose", "Serum glucose (mg/dL)"),
    ];

    for (name, desc) in apache_components {
        features.push(FeatureDef {
            name: name.to_string(),
            dtype: FeatureDtype::Float32,
            description: desc.to_string(),
            transformation: None,
            stats: None,
        });
    }

    features.push(FeatureDef {
        name: "admission_type_emergency".to_string(),
        dtype: FeatureDtype::Bool,
        description: "Emergency admission".to_string(),
        transformation: None,
        stats: None,
    });
    features.push(FeatureDef {
        name: "sex_male".to_string(),
        dtype: FeatureDtype::Bool,
        description: "Sex: 1=male, 0=female".to_string(),
        transformation: None,
        stats: None,
    });
    features.push(FeatureDef {
        name: "charlson_comorbidity_index".to_string(),
        dtype: FeatureDtype::Int32,
        description: "Charlson Comorbidity Index".to_string(),
        transformation: None,
        stats: None,
    });

    FeatureSet::new(
        "mortality_v2",
        "2.0.0",
        features,
        "Feature set for Mortality Ensemble v2: APACHE II components + demographics + comorbidities",
    )
}

pub fn build_ews_v1() -> FeatureSet {
    let mut features = Vec::new();

    let vitals = [
        "heart_rate", "respiratory_rate", "spo2", "temperature",
        "systolic_bp", "diastolic_bp", "gcs_total",
    ];

    for vital in vitals {
        features.push(FeatureDef {
            name: vital.to_string(),
            dtype: FeatureDtype::Float32,
            description: format!("Current {}", vital),
            transformation: None,
            stats: None,
        });
    }

    features.push(FeatureDef {
        name: "news2_score".to_string(),
        dtype: FeatureDtype::Float32,
        description: "NEWS2 score".to_string(),
        transformation: Some("news2_calculation".to_string()),
        stats: None,
    });
    features.push(FeatureDef {
        name: "ews_score".to_string(),
        dtype: FeatureDtype::Float32,
        description: "Early Warning Score".to_string(),
        transformation: Some("ews_calculation".to_string()),
        stats: None,
    });
    features.push(FeatureDef {
        name: "apache_ii_delta_1h".to_string(),
        dtype: FeatureDtype::Float32,
        description: "APACHE II change in last hour".to_string(),
        transformation: Some("delta_apache_1h".to_string()),
        stats: None,
    });

    FeatureSet::new(
        "ews_v1",
        "1.0.0",
        features,
        "Feature set for EWS streaming: raw vitals + clinical scores",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_set_hash_deterministic() {
        let fs1 = build_los_nn_v1();
        let fs2 = build_los_nn_v1();
        assert_eq!(fs1.hash, fs2.hash, "Hash must be deterministic");
        assert!(fs1.verify_hash());
        assert!(fs2.verify_hash());
    }

    #[test]
    fn test_feature_set_different_version_different_hash() {
        let fs1 = build_los_nn_v1();
        let mut fs2 = fs1.clone();
        fs2.version = Version::parse("1.0.1").unwrap();
        fs2.hash = fs2.compute_hash();
        assert_ne!(fs1.hash, fs2.hash, "Different version must produce different hash");
    }

    #[test]
    fn test_feature_set_feature_count() {
        let fs = build_los_nn_v1();
        assert!(fs.feature_count() > 100, "LOS-NN v1 should have >100 features");
        let fs2 = build_mortality_v2();
        assert!(fs2.feature_count() > 20, "Mortality v2 should have >20 features");
        let fs3 = build_ews_v1();
        assert!(fs3.feature_count() >= 10, "EWS v1 should have >=10 features");
    }

    #[test]
    fn test_normalizer_fit_transform() {
        let mut normalizer = Normalizer::new();
        let data = Array2::from_shape_vec(
            (10, 3),
            vec![
                1.0, 2.0, 3.0,
                2.0, 3.0, 4.0,
                3.0, 4.0, 5.0,
                4.0, 5.0, 6.0,
                5.0, 6.0, 7.0,
                6.0, 7.0, 8.0,
                7.0, 8.0, 9.0,
                8.0, 9.0, 10.0,
                9.0, 10.0, 11.0,
                10.0, 11.0, 12.0,
            ],
        ).unwrap();
        let feature_names = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        normalizer.fit(&data, &feature_names);

        let mut test_data = data.clone();
        normalizer.transform(&mut test_data);

        for i in 0..3 {
            let col = test_data.column(i);
            let mean: f32 = col.iter().sum::<f32>() / 10.0;
            let std: f32 = (col.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / 10.0).sqrt();
            assert!(mean.abs() < 1e-5, "Mean should be ~0 after normalization");
            assert!((std - 1.0).abs() < 1e-5, "Std should be ~1 after normalization");
        }
    }

    #[test]
    fn test_ml_features_validation() {
        let fs = build_mortality_v2();
        let mut features = MlFeatures::new();
        for fdef in &fs.features {
            features.insert(fdef.name.clone(), 1.0);
        }
        features.feature_set_hash = Some(fs.hash.clone());
        assert!(features.validate_against(&fs).is_ok());

        let mut bad_features = MlFeatures::new();
        bad_features.insert("apache_score".to_string(), 1.0);
        bad_features.feature_set_hash = Some(fs.hash.clone());
        assert!(bad_features.validate_against(&fs).is_err());
    }

    #[test]
    fn test_time_window() {
        let window = TimeWindow::last_hours(24);
        assert_eq!(window.duration_hours(), 24);
    }
}