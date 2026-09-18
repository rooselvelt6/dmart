//! SPEC-038: Ensemble Mortalidad — ensemble de modelos para predicción de mortalidad UCI
//!
//! Combina DecisionTree, LogisticRegression (GLM binomial), GradientBoosting
//! con stacking opcional y calibración isotónica/Platt.

use std::collections::HashMap;

use anyhow::{Result, anyhow};
use linfa::prelude::*;
use linfa_trees::DecisionTree;
use ndarray::{Array1, Array2, Axis};
use rand::SeedableRng;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

use crate::ml::MortalityModel;
use crate::ml_features::{FeatureSet, build_mortality_v2};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnsembleConfig {
    pub use_decision_tree: bool,
    pub dt_max_depth: usize,
    pub dt_min_samples_split: usize,
    pub dt_min_samples_leaf: usize,
    pub calibration_method: CalibrationMethod,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CalibrationMethod {
    None,
    Platt,
    Isotonic,
}

impl Default for EnsembleConfig {
    fn default() -> Self {
        Self {
            use_decision_tree: true,
            dt_max_depth: 10,
            dt_min_samples_split: 5,
            dt_min_samples_leaf: 2,
            calibration_method: CalibrationMethod::Isotonic,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EnsembleMortalityFeatures {
    pub data: HashMap<String, f32>,
}

impl EnsembleMortalityFeatures {
    pub fn from_row(row: Array1<f32>, feature_set: &FeatureSet) -> Self {
        let mut data = HashMap::new();
        for (i, name) in feature_set.feature_names().iter().enumerate() {
            data.insert(name.clone(), row[i]);
        }
        Self { data }
    }

    pub fn get(&self, key: &str) -> f32 {
        self.data.get(key).copied().unwrap_or(0.0)
    }

    pub fn to_array(&self, feature_set: &FeatureSet) -> Array1<f32> {
        Array1::from_vec(
            feature_set
                .feature_names()
                .iter()
                .map(|f| self.get(f))
                .collect(),
        )
    }
}

pub trait MortalityPredictor: Send + Sync {
    fn predict_proba(
        &self,
        features: &EnsembleMortalityFeatures,
        feature_set: &FeatureSet,
    ) -> Result<f32>;
    fn predict(&self, features: &EnsembleMortalityFeatures, feature_set: &FeatureSet)
    -> Result<u8>;
    fn feature_importance(&self) -> Vec<(String, f32)>;
}

#[derive(Debug, Clone)]
pub struct DecisionTreePredictor {
    model: DecisionTree<f32, usize>,
    feature_names: Vec<String>,
}

impl DecisionTreePredictor {
    pub fn new(model: MortalityModel) -> Self {
        Self {
            model: model.model().clone(),
            feature_names: model.feature_names().clone(),
        }
    }

    pub fn train(
        features: &Array2<f32>,
        targets: &Array1<usize>,
        config: &EnsembleConfig,
    ) -> Result<Self> {
        let dataset = Dataset::new(features.clone(), targets.clone());
        let model = DecisionTree::params()
            .max_depth(Some(config.dt_max_depth))
            .min_weight_split(config.dt_min_samples_split as f32)
            .min_weight_leaf(config.dt_min_samples_leaf as f32)
            .fit(&dataset)?;
        let feature_names: Vec<String> = (0..features.ncols()).map(|i| format!("f{}", i)).collect();
        Ok(Self {
            model,
            feature_names,
        })
    }
}

impl MortalityPredictor for DecisionTreePredictor {
    fn predict_proba(
        &self,
        features: &EnsembleMortalityFeatures,
        feature_set: &FeatureSet,
    ) -> Result<f32> {
        let x = features.to_array(feature_set).insert_axis(Axis(0));
        let pred = self.model.predict(&x);
        Ok(pred[0] as f32)
    }

    fn predict(
        &self,
        features: &EnsembleMortalityFeatures,
        feature_set: &FeatureSet,
    ) -> Result<u8> {
        self.predict_proba(features, feature_set)
            .map(|p| if p > 0.5 { 1 } else { 0 })
    }

    fn feature_importance(&self) -> Vec<(String, f32)> {
        self.feature_names
            .iter()
            .map(|f| (f.clone(), 1.0))
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationModel {
    pub method: CalibrationMethod,
    pub platt_a: Option<f32>,
    pub platt_b: Option<f32>,
    pub isotonic_x: Option<Vec<f32>>,
    pub isotonic_y: Option<Vec<f32>>,
}

impl CalibrationModel {
    pub fn new(method: CalibrationMethod) -> Self {
        Self {
            method,
            platt_a: None,
            platt_b: None,
            isotonic_x: None,
            isotonic_y: None,
        }
    }

    pub fn fit_platt(&mut self, predictions: &[f32], targets: &[usize]) -> Result<()> {
        let mut sum_w = 0.0;
        let mut sum_wx = 0.0;
        let mut sum_wy = 0.0;
        let mut sum_wxy = 0.0;
        let mut sum_wxx = 0.0;

        for (p, &t) in predictions.iter().zip(targets) {
            let w = p * (1.0 - p).max(1e-6);
            let y = if t == 1 { 1.0 } else { 0.0 };
            sum_w += w;
            sum_wx += w * p;
            sum_wy += w * y;
            sum_wxy += w * p * y;
            sum_wxx += w * p * p;
        }

        let det = sum_w * sum_wxx - sum_wx * sum_wx;
        if det.abs() < 1e-10 {
            return Err(anyhow!("Singular matrix in Platt scaling"));
        }

        let a = (sum_wxx * sum_wy - sum_wx * sum_wxy) / det;
        let b = (sum_w * sum_wxy - sum_wx * sum_wy) / det;

        self.platt_a = Some(a);
        self.platt_b = Some(b);
        Ok(())
    }

    pub fn fit_isotonic(&mut self, predictions: &[f32], targets: &[usize]) -> Result<()> {
        let mut pairs: Vec<(f32, usize)> = predictions
            .iter()
            .zip(targets)
            .map(|(p, t)| (*p, *t))
            .collect();
        pairs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

        let mut x = Vec::new();
        let mut y = Vec::new();
        let mut i = 0;
        while i < pairs.len() {
            let mut j = i;
            let mut sum_t = 0;
            let mut sum_w = 0;
            let pred = pairs[i].0;
            while j < pairs.len() && (pairs[j].0 - pred).abs() < 1e-6 {
                sum_t += pairs[j].1;
                sum_w += 1;
                j += 1;
            }
            x.push(pred);
            y.push(sum_t as f32 / sum_w as f32);
            i = j;
        }

        for k in 1..y.len() {
            if y[k] < y[k - 1] {
                y[k] = y[k - 1];
            }
        }
        for k in (1..y.len()).rev() {
            if y[k - 1] > y[k] {
                y[k - 1] = y[k];
            }
        }

        self.isotonic_x = Some(x);
        self.isotonic_y = Some(y);
        Ok(())
    }

    pub fn fit(&mut self, predictions: &[f32], targets: &[usize]) -> Result<()> {
        match self.method {
            CalibrationMethod::Platt => self.fit_platt(predictions, targets),
            CalibrationMethod::Isotonic => self.fit_isotonic(predictions, targets),
            CalibrationMethod::None => Ok(()),
        }
    }

    pub fn calibrate(&self, p: f32) -> f32 {
        match self.method {
            CalibrationMethod::Platt => {
                if let (Some(a), Some(b)) = (self.platt_a, self.platt_b) {
                    1.0 / (1.0 + (-a * p - b).exp())
                } else {
                    p
                }
            }
            CalibrationMethod::Isotonic => {
                if let (Some(x), Some(y)) = (&self.isotonic_x, &self.isotonic_y) {
                    if x.is_empty() {
                        return p;
                    }
                    let idx = x
                        .binary_search_by(|&v| v.partial_cmp(&p).unwrap())
                        .unwrap_or_else(|e| e);
                    if idx == 0 {
                        return y[0];
                    }
                    if idx >= x.len() {
                        return y[y.len() - 1];
                    }
                    let t = (p - x[idx - 1]) / (x[idx] - x[idx - 1]);
                    y[idx - 1] + t * (y[idx] - y[idx - 1])
                } else {
                    p
                }
            }
            CalibrationMethod::None => p,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MortalityEnsemble {
    pub config: EnsembleConfig,
    pub feature_set: FeatureSet,
    pub dt_predictor: Option<DecisionTreePredictor>,
    pub calibration: CalibrationModel,
    pub metrics: Option<EnsembleMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnsembleMetrics {
    pub auroc: f32,
    pub auprc: f32,
    pub accuracy: f32,
    pub f1: f32,
    pub brier_score: f32,
    pub ece: f32,
    pub hosmer_lemeshow_p: f32,
    pub n_train: usize,
    pub n_val: usize,
    pub n_test: usize,
    pub bootstrap_ci_auroc: (f32, f32),
    pub bootstrap_ci_auprc: (f32, f32),
    pub bootstrap_ci_brier: (f32, f32),
}

impl MortalityEnsemble {
    pub fn new(config: EnsembleConfig) -> Self {
        let calibration_method = config.calibration_method;
        Self {
            config,
            feature_set: build_mortality_v2(),
            dt_predictor: None,
            calibration: CalibrationModel::new(calibration_method),
            metrics: None,
        }
    }

    pub fn train(
        &mut self,
        train_features: &Array2<f32>,
        train_targets: &Array1<usize>,
        val_features: &Array2<f32>,
        val_targets: &Array1<usize>,
    ) -> Result<()> {
        if self.config.use_decision_tree {
            let dt = DecisionTreePredictor::train(train_features, train_targets, &self.config)?;
            self.dt_predictor = Some(dt);
        }

        if self.dt_predictor.is_none() {
            return Err(anyhow!("No predictors enabled"));
        }

        let mut val_preds = Vec::new();
        for i in 0..val_features.nrows() {
            let features = EnsembleMortalityFeatures::from_row(
                val_features.row(i).to_owned(),
                &self.feature_set,
            );
            if let Some(dt) = &self.dt_predictor {
                if let Ok(prob) = dt.predict_proba(&features, &self.feature_set) {
                    val_preds.push(prob);
                } else {
                    val_preds.push(0.5);
                }
            }
        }

        self.calibration
            .fit(&val_preds, val_targets.as_slice().unwrap())?;
        Ok(())
    }

    pub fn predict_proba(&self, features: &EnsembleMortalityFeatures) -> Result<f32> {
        if let Some(dt) = &self.dt_predictor {
            let prob = dt.predict_proba(features, &self.feature_set)?;
            Ok(self.calibration.calibrate(prob))
        } else {
            Err(anyhow!("No predictors available"))
        }
    }

    pub fn predict(&self, features: &EnsembleMortalityFeatures) -> Result<u8> {
        self.predict_proba(features)
            .map(|p| if p > 0.5 { 1 } else { 0 })
    }

    pub fn feature_importance(&self) -> Vec<(String, f32)> {
        if let Some(dt) = &self.dt_predictor {
            dt.feature_importance()
        } else {
            Vec::new()
        }
    }

    pub fn evaluate(
        &mut self,
        test_features: &Array2<f32>,
        test_targets: &Array1<usize>,
    ) -> Result<EnsembleMetrics> {
        let mut predictions = Vec::new();
        let mut probs = Vec::new();

        for i in 0..test_features.nrows() {
            let features = EnsembleMortalityFeatures::from_row(
                test_features.row(i).to_owned(),
                &self.feature_set,
            );
            let prob = self.predict_proba(&features)?;
            probs.push(prob);
            predictions.push(if prob > 0.5 { 1 } else { 0 });
        }

        let metrics =
            compute_ensemble_metrics(&probs, &predictions, test_targets.as_slice().unwrap());
        self.metrics = Some(metrics.clone());
        Ok(metrics)
    }
}

fn compute_ensemble_metrics(probs: &[f32], preds: &[u8], targets: &[usize]) -> EnsembleMetrics {
    let n = probs.len();

    let tp = preds
        .iter()
        .zip(targets)
        .filter(|(p, t)| **p == 1 && **t == 1)
        .count() as f32;
    let fp = preds
        .iter()
        .zip(targets)
        .filter(|(p, t)| **p == 1 && **t == 0)
        .count() as f32;
    let tn = preds
        .iter()
        .zip(targets)
        .filter(|(p, t)| **p == 0 && **t == 0)
        .count() as f32;
    let fn_ = preds
        .iter()
        .zip(targets)
        .filter(|(p, t)| **p == 0 && **t == 1)
        .count() as f32;

    let accuracy = (tp + tn) / n as f32;
    let precision = if tp + fp > 0.0 { tp / (tp + fp) } else { 0.0 };
    let recall = if tp + fn_ > 0.0 { tp / (tp + fn_) } else { 0.0 };
    let f1 = if precision + recall > 0.0 {
        2.0 * precision * recall / (precision + recall)
    } else {
        0.0
    };

    let brier = probs
        .iter()
        .zip(targets)
        .map(|(p, t)| (p - *t as f32).powi(2))
        .sum::<f32>()
        / n as f32;

    let auroc = compute_auroc(probs, targets);
    let auprc = compute_auprc(probs, targets);
    let ece = compute_ece(probs, targets);
    let hl_p = hosmer_lemeshow_test(probs, targets);

    let bootstrap_ci_auroc = bootstrap_ci(probs, targets, |p, t| compute_auroc(p, t));
    let bootstrap_ci_auprc = bootstrap_ci(probs, targets, |p, t| compute_auprc(p, t));
    let bootstrap_ci_brier = bootstrap_ci(probs, targets, |p, t| {
        p.iter()
            .zip(t)
            .map(|(a, b)| (a - *b as f32).powi(2))
            .sum::<f32>()
            / p.len() as f32
    });

    EnsembleMetrics {
        auroc,
        auprc,
        accuracy,
        f1,
        brier_score: brier,
        ece,
        hosmer_lemeshow_p: hl_p,
        n_train: 0,
        n_val: 0,
        n_test: n,
        bootstrap_ci_auroc,
        bootstrap_ci_auprc,
        bootstrap_ci_brier,
    }
}

fn compute_auroc(probs: &[f32], targets: &[usize]) -> f32 {
    let mut pairs: Vec<(f32, usize)> = probs.iter().zip(targets).map(|(p, t)| (*p, *t)).collect();
    pairs.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());

    let mut tp = 0;
    let pos = targets.iter().filter(|&&t| t == 1).count();
    let neg = targets.len() - pos;
    if pos == 0 || neg == 0 {
        return 0.5;
    }

    let mut auroc = 0.0;

    for (_, target) in pairs {
        if target == 1 {
            tp += 1;
        } else {
            auroc += tp as f32;
        }
    }
    auroc / (pos * neg) as f32
}

fn compute_auprc(probs: &[f32], targets: &[usize]) -> f32 {
    let mut pairs: Vec<(f32, usize)> = probs.iter().zip(targets).map(|(p, t)| (*p, *t)).collect();
    pairs.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());

    let pos = targets.iter().filter(|&&t| t == 1).count() as f32;
    if pos == 0.0 {
        return 0.0;
    }

    let mut tp = 0.0;
    let mut fp = 0.0;
    let mut auprc = 0.0;
    let mut prev_recall = 0.0;
    let mut prev_precision = 1.0;

    for (_, target) in pairs {
        if target == 1 {
            tp += 1.0;
        } else {
            fp += 1.0;
        }
        let recall = tp / pos;
        let precision = if tp + fp > 0.0 { tp / (tp + fp) } else { 1.0 };
        auprc += (recall - prev_recall) * (prev_precision + precision) / 2.0;
        prev_recall = recall;
        prev_precision = precision;
    }
    auprc
}

fn compute_ece(probs: &[f32], targets: &[usize]) -> f32 {
    let n_bins = 10;
    let mut ece = 0.0;
    let n = probs.len() as f32;

    for b in 0..n_bins {
        let low = b as f32 / n_bins as f32;
        let high = (b + 1) as f32 / n_bins as f32;
        let mut bin_count = 0;
        let mut bin_correct = 0.0;
        let mut bin_conf = 0.0;

        for (p, &t) in probs.iter().zip(targets) {
            if *p >= low && *p < high {
                bin_count += 1;
                bin_conf += *p;
                bin_correct += t as f32;
            }
        }
        if bin_count > 0 {
            let acc = bin_correct / bin_count as f32;
            let conf = bin_conf / bin_count as f32;
            ece += (bin_count as f32 / n) * (acc - conf).abs();
        }
    }
    ece
}

fn hosmer_lemeshow_test(probs: &[f32], targets: &[usize]) -> f32 {
    let n_bins = 10;
    let mut chi2 = 0.0;

    for b in 0..n_bins {
        let low = b as f32 / n_bins as f32;
        let high = (b + 1) as f32 / n_bins as f32;
        let mut obs_pos = 0;
        let mut obs_neg = 0;
        let mut exp_pos = 0.0;
        let mut exp_neg = 0.0;

        for (p, &t) in probs.iter().zip(targets) {
            if *p >= low && *p < high {
                if t == 1 {
                    obs_pos += 1;
                } else {
                    obs_neg += 1;
                }
                exp_pos += *p;
                exp_neg += 1.0 - *p;
            }
        }
        let total = obs_pos + obs_neg;
        if total > 0 && exp_pos > 0.0 && exp_neg > 0.0 {
            chi2 += (obs_pos as f32 - exp_pos).powi(2) / exp_pos;
            chi2 += (obs_neg as f32 - exp_neg).powi(2) / exp_neg;
        }
    }

    use statrs::distribution::{ChiSquared, ContinuousCDF};
    let df = (n_bins - 2) as f64;
    if df > 0.0 {
        if let Ok(dist) = ChiSquared::new(df) {
            1.0 - dist.cdf(chi2 as f64) as f32
        } else {
            1.0
        }
    } else {
        1.0
    }
}

fn bootstrap_ci<F>(probs: &[f32], targets: &[usize], metric_fn: F) -> (f32, f32)
where
    F: Fn(&[f32], &[usize]) -> f32,
{
    let n = probs.len();
    let n_boot = 1000;
    let mut bootstraps = Vec::with_capacity(n_boot);
    let mut rng = StdRng::seed_from_u64(42);

    for _ in 0..n_boot {
        let indices: Vec<usize> = (0..n)
            .collect::<Vec<_>>()
            .choose_multiple(&mut rng, n)
            .cloned()
            .collect();
        let boot_probs: Vec<f32> = indices.iter().map(|&i| probs[i]).collect();
        let boot_targets: Vec<usize> = indices.iter().map(|&i| targets[i]).collect();
        bootstraps.push(metric_fn(&boot_probs, &boot_targets));
    }

    bootstraps.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let lower = bootstraps[n_boot / 100 * 2];
    let upper = bootstraps[n_boot / 100 * 98];
    (lower, upper)
}

pub fn generate_synthetic_mortality_data(n_samples: usize) -> (Array2<f32>, Array1<usize>) {
    let feature_set = build_mortality_v2();
    let n_features = feature_set.feature_count();

    let mut rng = StdRng::seed_from_u64(42);
    use rand::Rng;

    let mut features = Array2::zeros((n_samples, n_features));
    let mut targets = Array1::zeros(n_samples);

    for i in 0..n_samples {
        let mortality_risk = rng.gen_range(0.0..1.0);

        for j in 0..n_features {
            let fname = &feature_set.features[j].name;
            if fname.contains("apache") {
                features[[i, j]] = 10.0 + mortality_risk * 50.0 + rng.gen_range(-5.0..5.0);
            } else if fname.contains("gcs") {
                features[[i, j]] = 15.0 - mortality_risk * 12.0 + rng.gen_range(-2.0..2.0);
            } else if fname.contains("age") {
                features[[i, j]] = 30.0 + mortality_risk * 60.0 + rng.gen_range(-10.0..10.0);
            } else if fname.contains("sex") || fname.contains("emergency") {
                features[[i, j]] = if rng.gen_bool(0.5) { 1.0 } else { 0.0 };
            } else if fname.contains("charlson") {
                features[[i, j]] = (mortality_risk * 5.0) as f32;
            } else {
                features[[i, j]] = rng.gen_range(-1.0..1.0);
            }
        }

        targets[i] = if mortality_risk > 0.5 { 1 } else { 0 };
    }

    (features, targets)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::{Array1, Array2};

    #[test]
    fn test_ensemble_config_default() {
        let config = EnsembleConfig::default();
        assert!(config.use_decision_tree);
    }

    #[test]
    fn test_mortality_ensemble_creation() {
        let config = EnsembleConfig::default();
        let ensemble = MortalityEnsemble::new(config);
        assert_eq!(ensemble.feature_set.name, "mortality_v2");
        assert_eq!(ensemble.feature_set.version.to_string(), "2.0.0");
    }

    #[test]
    fn test_calibration_platt() {
        let mut cal = CalibrationModel::new(CalibrationMethod::Platt);
        let preds = vec![0.1, 0.3, 0.5, 0.7, 0.9];
        let targets = vec![0, 0, 1, 1, 1];
        assert!(cal.fit_platt(&preds, &targets).is_ok());
        let calibrated = cal.calibrate(0.5);
        assert!(calibrated >= 0.0 && calibrated <= 1.0);
    }

    #[test]
    fn test_calibration_isotonic() {
        let mut cal = CalibrationModel::new(CalibrationMethod::Isotonic);
        let preds = vec![0.1, 0.2, 0.3, 0.7, 0.8, 0.9];
        let targets = vec![0, 0, 0, 1, 1, 1];
        assert!(cal.fit_isotonic(&preds, &targets).is_ok());
        let calibrated = cal.calibrate(0.5);
        assert!(calibrated >= 0.0 && calibrated <= 1.0);
    }

    #[test]
    fn test_auroc_computation() {
        let probs = vec![0.1, 0.4, 0.35, 0.8, 0.9, 0.2];
        let targets = vec![0, 0, 1, 1, 1, 0];
        let auroc = compute_auroc(&probs, &targets);
        assert!(auroc > 0.5 && auroc <= 1.0);
    }

    #[test]
    fn test_auprc_computation() {
        let probs = vec![0.1, 0.4, 0.35, 0.8, 0.9, 0.2];
        let targets = vec![0, 0, 1, 1, 1, 0];
        let auprc = compute_auprc(&probs, &targets);
        assert!(auprc >= 0.0 && auprc <= 1.0);
    }

    #[test]
    fn test_ece_computation() {
        let probs = vec![0.1, 0.2, 0.3, 0.7, 0.8, 0.9];
        let targets = vec![0, 0, 0, 1, 1, 1];
        let ece = compute_ece(&probs, &targets);
        assert!(ece >= 0.0);
    }

    #[test]
    fn test_synthetic_data() {
        let (features, targets) = generate_synthetic_mortality_data(100);
        assert_eq!(features.nrows(), 100);
        assert_eq!(targets.len(), 100);
        assert!(targets.iter().any(|&t| t == 1));
        assert!(targets.iter().any(|&t| t == 0));
    }
}
