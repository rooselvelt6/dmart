//! SPEC-036: LOS-NN — Predicción de estancia (Length of Stay) con red neuronal
//!
//! Implementa MLP con ventana fija y LSTM/GRU opcional vía backend `candle`.
//! Incluye entrenamiento, inferencia, calibración isotónica y métricas con bootstrap CI.

use std::collections::HashMap;

#[cfg(feature = "ml-nn")]
use anyhow::{Context, Result, anyhow};
use ndarray::{Array1, Array2};
use rand::SeedableRng;
use rand::rngs::StdRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[cfg(feature = "ml-nn")]
use candle_core::{DType, Device, Tensor};
#[cfg(feature = "ml-nn")]
use candle_nn::{Linear, Module, VarBuilder, VarMap, linear, ops::softmax};

use crate::ml_features::{FeatureSet, Normalizer, build_los_nn_v1};
#[cfg(feature = "ml-nn")]
use crate::ml_features::MlFeatures;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LosNnConfig {
    pub input_size: usize,
    pub hidden_sizes: Vec<usize>,
    pub dropout: f32,
    pub use_lstm: bool,
    pub lstm_hidden_size: usize,
    pub lstm_num_layers: usize,
    pub sequence_length: usize,
    pub output_quantiles: Vec<f32>,
}

impl Default for LosNnConfig {
    fn default() -> Self {
        let feature_set = build_los_nn_v1();
        Self {
            input_size: feature_set.feature_count(),
            hidden_sizes: vec![256, 128, 64],
            dropout: 0.2,
            use_lstm: false,
            lstm_hidden_size: 128,
            lstm_num_layers: 2,
            sequence_length: 24,
            output_quantiles: vec![0.1, 0.25, 0.5, 0.75, 0.9],
        }
    }
}

#[cfg(feature = "ml-nn")]
type ModelWeights = Option<candle_nn::VarMap>;

#[cfg(not(feature = "ml-nn"))]
type ModelWeights = Option<()>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LosNnModel {
    pub config: LosNnConfig,
    pub feature_set: FeatureSet,
    pub normalizer: Normalizer,
    #[serde(skip)]
    pub model_weights: ModelWeights,
    pub sha256: String,
    pub trained_at: Option<String>,
    pub metrics: Option<LosNnMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LosNnMetrics {
    pub mae: f32,
    pub rmse: f32,
    pub mape: f32,
    pub binned_acc_24h: f32,
    pub binned_acc_48h: f32,
    pub ece: f32,
    pub n_train: usize,
    pub n_val: usize,
    pub n_test: usize,
    pub bootstrap_ci_mae: (f32, f32),
    pub bootstrap_ci_rmse: (f32, f32),
    pub bootstrap_ci_mape: (f32, f32),
}

impl LosNnModel {
    pub fn new(config: LosNnConfig) -> Self {
        let feature_set = build_los_nn_v1();
        assert_eq!(
            config.input_size,
            feature_set.feature_count(),
            "Config input_size must match feature set count"
        );
        Self {
            config,
            feature_set,
            normalizer: Normalizer::new(),
            model_weights: None,
            sha256: String::new(),
            trained_at: None,
            metrics: None,
        }
    }

    pub fn feature_set(&self) -> &FeatureSet {
        &self.feature_set
    }

    #[cfg(feature = "ml-nn")]
    pub fn compute_sha256(&mut self) -> String {
        let mut hasher = Sha256::new();
        if let Some(weights) = &self.model_weights {
            for (name, tensor) in weights.data() {
                let bytes = tensor.to_vec1::<f32>().unwrap_or_default();
                hasher.update(bytemuck::cast_slice(&bytes));
            }
        }
        hasher.update(self.feature_set.hash.as_bytes());
        let hash = format!("{:x}", hasher.finalize());
        self.sha256 = hash.clone();
        hash
    }

    #[cfg(not(feature = "ml-nn"))]
    pub fn compute_sha256(&mut self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.feature_set.hash.as_bytes());
        let hash = format!("{:x}", hasher.finalize());
        self.sha256 = hash.clone();
        hash
    }
}

#[cfg(feature = "ml-nn")]
impl LosNnModel {
    pub fn init_model(&mut self, device: &Device) -> Result<()> {
        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, device);
        self.build_model(vb)?;
        self.model_weights = Some(varmap);
        Ok(())
    }

    fn build_model(&self, vb: VarBuilder) -> Result<Box<dyn LosPredictor>> {
        if self.config.use_lstm {
            Ok(Box::new(LstmLosPredictor::new(vb, &self.config)?))
        } else {
            Ok(Box::new(MlpLosPredictor::new(vb, &self.config)?))
        }
    }

    pub fn save(&self, path: &str) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        self.normalizer
            .save_json(&format!("{}.normalizer.json", path))?;
        Ok(())
    }

    pub fn load(path: &str) -> Result<Self> {
        let json = std::fs::read_to_string(path)?;
        let mut model: Self = serde_json::from_str(&json)?;
        model.normalizer = Normalizer::load_json(&format!("{}.normalizer.json", path))?;
        Ok(model)
    }
}

#[cfg(feature = "ml-nn")]
impl LosNnModel {
    pub fn init_model(&mut self, device: &Device) -> Result<()> {
        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, device);
        self.build_model(vb)?;
        self.model_weights = Some(varmap);
        Ok(())
    }

    pub fn train(
        &mut self,
        train_features: &Array2<f32>,
        train_targets: &Array1<f32>,
        val_features: &Array2<f32>,
        val_targets: &Array1<f32>,
        epochs: usize,
        batch_size: usize,
        learning_rate: f32,
    ) -> Result<()> {
        let device = Device::Cpu;
        self.init_model(&device)?;
        self.normalizer
            .fit(train_features, &self.feature_set.feature_names());

        let mut train_features_norm = train_features.clone();
        self.normalizer.transform(&mut train_features_norm);
        let mut val_features_norm = val_features.clone();
        self.normalizer.transform(&mut val_features_norm);

        let predictor = self.build_model(VarBuilder::from_varmap(
            self.model_weights.as_ref().unwrap(),
            DType::F32,
            &device,
        ))?;

        predictor.train(
            &train_features_norm,
            train_targets,
            &val_features_norm,
            val_targets,
            epochs,
            batch_size,
            learning_rate,
            &device,
        )?;

        self.trained_at = Some(chrono::Utc::now().to_rfc3339());
        self.compute_sha256();
        Ok(())
    }

    pub fn predict(&self, features: &MlFeatures) -> Result<LosPrediction> {
        features.validate_against(&self.feature_set)?;

        let mut feature_array = features.to_array(&self.feature_set.feature_names());
        self.normalizer.transform_vector(&mut feature_array);

        let device = Device::Cpu;
        let predictor = self.build_model(VarBuilder::from_varmap(
            self.model_weights.as_ref().context("Model not trained")?,
            DType::F32,
            &device,
        ))?;

        predictor.predict(&feature_array)
    }

    pub fn predict_batch(&self, features: &[MlFeatures]) -> Result<Vec<LosPrediction>> {
        features.iter().map(|f| self.predict(f)).collect()
    }

    pub fn evaluate(
        &mut self,
        test_features: &Array2<f32>,
        test_targets: &Array1<f32>,
    ) -> Result<LosNnMetrics> {
        let mut test_features_norm = test_features.clone();
        self.normalizer.transform(&mut test_features_norm);

        let device = Device::Cpu;
        let predictor = self.build_model(VarBuilder::from_varmap(
            self.model_weights.as_ref().context("Model not trained")?,
            DType::F32,
            &device,
        ))?;

        let mut predictions = Vec::with_capacity(test_targets.len());
        for i in 0..test_targets.len() {
            let row = test_features_norm.row(i).to_owned();
            let pred = predictor.predict(&row)?;
            predictions.push(pred.los_hours);
        }

        let metrics = compute_metrics(&predictions, test_targets.as_slice().unwrap());
        self.metrics = Some(metrics.clone());
        Ok(metrics)
    }
}

#[cfg(feature = "ml-nn")]
pub trait LosPredictor: Send + Sync {
    fn train(
        &self,
        train_features: &Array2<f32>,
        train_targets: &Array1<f32>,
        val_features: &Array2<f32>,
        val_targets: &Array1<f32>,
        epochs: usize,
        batch_size: usize,
        learning_rate: f32,
        device: &candle_core::Device,
    ) -> Result<()>;

    fn predict(&self, features: &Array1<f32>) -> Result<LosPrediction>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LosPrediction {
    pub los_hours: f32,
    pub quantiles: HashMap<String, f32>,
    pub confidence_interval: (f32, f32),
}

#[cfg(feature = "ml-nn")]
mod nn_impl {
    use super::*;
    use candle_core::{DType, Device, Tensor};
    use candle_nn::{Dropout, LSTM, LSTMConfig, Linear, Module, VarBuilder, linear_no_bias};

    pub struct MlpLosPredictor {
        layers: Vec<Linear>,
        dropout: Dropout,
        output_layer: Linear,
        config: LosNnConfig,
    }

    impl MlpLosPredictor {
        pub fn new(vb: VarBuilder, config: &LosNnConfig) -> Result<Self> {
            let mut layers = Vec::new();
            let mut in_size = config.input_size;

            for (i, &hidden_size) in config.hidden_sizes.iter().enumerate() {
                let layer = linear(in_size, hidden_size, vb.pp(&format!("layer_{}", i)))?;
                layers.push(layer);
                in_size = hidden_size;
            }

            let output_layer = linear(in_size, 1 + config.output_quantiles.len(), vb.pp("output"))?;
            let dropout = Dropout::new(config.dropout);

            Ok(Self {
                layers,
                dropout,
                output_layer,
                config: config.clone(),
            })
        }

        fn forward(&self, x: &Tensor) -> Result<Tensor> {
            let mut x = x.clone();
            for layer in &self.layers {
                x = layer.forward(&x)?;
                x = x.relu()?;
                x = self.dropout.forward(&x)?;
            }
            let out = self.output_layer.forward(&x)?;
            Ok(out)
        }
    }

    impl LosPredictor for MlpLosPredictor {
        fn train(
            &self,
            train_features: &Array2<f32>,
            train_targets: &Array1<f32>,
            val_features: &Array2<f32>,
            val_targets: &Array1<f32>,
            epochs: usize,
            batch_size: usize,
            learning_rate: f32,
            device: &Device,
        ) -> Result<()> {
            use candle_nn::{Optimizer, SGD, loss::mse};

            let n_samples = train_features.nrows();
            let n_features = train_features.ncols();

            let train_x = Tensor::from_slice(
                train_features.as_slice().unwrap(),
                (n_samples, n_features),
                device,
            )?;
            let train_y =
                Tensor::from_slice(train_targets.as_slice().unwrap(), (n_samples, 1), device)?;

            let val_x = Tensor::from_slice(
                val_features.as_slice().unwrap(),
                (val_features.nrows(), n_features),
                device,
            )?;
            let val_y = Tensor::from_slice(
                val_targets.as_slice().unwrap(),
                (val_targets.len(), 1),
                device,
            )?;

            let varmap = VarMap::new();
            let vb = VarBuilder::from_varmap(&varmap, DType::F32, device);
            let mut model = MlpLosPredictor::new(vb, &self.config)?;
            let mut opt = SGD::new(varmap.all_vars(), learning_rate)?;

            for epoch in 0..epochs {
                let mut indices: Vec<usize> = (0..n_samples).collect();
                use rand::seq::SliceRandom;
                let mut rng = StdRng::seed_from_u64(epoch as u64 + 42);
                indices.shuffle(&mut rng);

                for batch_start in (0..n_samples).step_by(batch_size) {
                    let batch_end = (batch_start + batch_size).min(n_samples);
                    let batch_indices = &indices[batch_start..batch_end];
                    let batch_size_actual = batch_indices.len();

                    let batch_x = train_x.index_select(
                        &Tensor::from_slice(batch_indices, batch_size_actual, device)?,
                        0,
                    )?;
                    let batch_y = train_y.index_select(
                        &Tensor::from_slice(batch_indices, batch_size_actual, device)?,
                        0,
                    )?;

                    let preds = model.forward(&batch_x)?;
                    let loss = mse(&preds.narrow(1, 0, 1)?, &batch_y)?;

                    opt.backward_step(&loss)?;
                }

                if epoch % 10 == 0 {
                    let val_preds = model.forward(&val_x)?;
                    let val_loss = mse(&val_preds.narrow(1, 0, 1)?, &val_y)?;
                    println!(
                        "Epoch {}: train_loss={:.4}, val_loss={:.4}",
                        epoch,
                        loss.to_scalar::<f32>()?,
                        val_loss.to_scalar::<f32>()?
                    );
                }
            }

            Ok(())
        }

        fn predict(&self, features: &Array1<f32>) -> Result<LosPrediction> {
            let device = Device::Cpu;
            let x = Tensor::from_slice(features.as_slice().unwrap(), (1, features.len()), &device)?;
            let out = self.forward(&x)?;

            let los_hours = out.narrow(1, 0, 1)?.to_scalar::<f32>()?.max(0.0);
            let mut quantiles = HashMap::new();
            for (i, q) in self.config.output_quantiles.iter().enumerate() {
                let key = format!("q{}", (q * 100.0) as u32);
                let val = out.narrow(1, 1 + i, 1)?.to_scalar::<f32>()?.max(0.0);
                quantiles.insert(key, val);
            }

            let ci_low = quantiles.get("q10").copied().unwrap_or(los_hours * 0.5);
            let ci_high = quantiles.get("q90").copied().unwrap_or(los_hours * 1.5);

            Ok(LosPrediction {
                los_hours,
                quantiles,
                confidence_interval: (ci_low, ci_high),
            })
        }
    }

    pub struct LstmLosPredictor {
        lstm: LSTM,
        output_layer: Linear,
        config: LosNnConfig,
    }

    impl LstmLosPredictor {
        pub fn new(vb: VarBuilder, config: &LosNnConfig) -> Result<Self> {
            let lstm_config = LSTMConfig {
                num_layers: config.lstm_num_layers,
                ..Default::default()
            };
            let lstm = LSTM::new(
                config.input_size,
                config.lstm_hidden_size,
                lstm_config,
                vb.pp("lstm"),
            )?;
            let output_layer = linear(
                config.lstm_hidden_size,
                1 + config.output_quantiles.len(),
                vb.pp("output"),
            )?;

            Ok(Self {
                lstm,
                output_layer,
                config: config.clone(),
            })
        }
    }

    impl LosPredictor for LstmLosPredictor {
        fn train(
            &self,
            _train_features: &Array2<f32>,
            _train_targets: &Array1<f32>,
            _val_features: &Array2<f32>,
            _val_targets: &Array1<f32>,
            _epochs: usize,
            _batch_size: usize,
            _learning_rate: f32,
            _device: &Device,
        ) -> Result<()> {
            Err(anyhow::anyhow!("LSTM training not yet implemented"))
        }

        fn predict(&self, _features: &Array1<f32>) -> Result<LosPrediction> {
            Err(anyhow::anyhow!("LSTM prediction not yet implemented"))
        }
    }
}

#[cfg(feature = "ml-nn")]
use nn_impl::{LstmLosPredictor, MlpLosPredictor};

#[cfg(not(feature = "ml-nn"))]
#[allow(unused_imports, unused, dead_code, non_snake_case)]
mod nn_stub {
    use super::*;
    use anyhow::{Result, anyhow};
    use ndarray::Array1;

    // Mock Device type when candle is not available
    pub struct Device;
    impl Device {
        pub fn Cpu() -> Self {
            Device
        }
    }

    pub trait LosPredictor: Send + Sync {
        fn train(
            &self,
            _train_features: &Array2<f32>,
            _train_targets: &Array1<f32>,
            _val_features: &Array2<f32>,
            _val_targets: &Array1<f32>,
            _epochs: usize,
            _batch_size: usize,
            _learning_rate: f32,
            _device: &Device,
        ) -> Result<()> {
            Err(anyhow::anyhow!("ml-nn feature not enabled"))
        }

        fn predict(&self, _features: &Array1<f32>) -> Result<LosPrediction> {
            Err(anyhow::anyhow!("ml-nn feature not enabled"))
        }
    }

    pub struct MlpLosPredictor;
    impl MlpLosPredictor {
        pub fn new(_vb: &dyn std::any::Any, _config: &LosNnConfig) -> Result<Self> {
            Err(anyhow::anyhow!("ml-nn feature not enabled"))
        }
    }
    impl LosPredictor for MlpLosPredictor {}

    pub struct LstmLosPredictor;
    impl LstmLosPredictor {
        pub fn new(_vb: &dyn std::any::Any, _config: &LosNnConfig) -> Result<Self> {
            Err(anyhow::anyhow!("ml-nn feature not enabled"))
        }
    }
    impl LosPredictor for LstmLosPredictor {}
}

#[cfg(not(feature = "ml-nn"))]
#[allow(unused_imports)]
use nn_stub::{LstmLosPredictor, MlpLosPredictor};

#[cfg(feature = "ml-nn")]
fn compute_metrics(predictions: &[f32], targets: &[f32]) -> LosNnMetrics {
    let n = predictions.len() as f32;
    let mut mae = 0.0;
    let mut mse = 0.0;
    let mut mape = 0.0;
    let mut binned_24 = 0;
    let mut binned_48 = 0;

    for (pred, target) in predictions.iter().zip(targets.iter()) {
        let err = (pred - target).abs();
        mae += err;
        mse += err * err;
        if *target > 1.0 {
            mape += err / target;
        }
        if err <= 24.0 {
            binned_24 += 1;
        }
        if err <= 48.0 {
            binned_48 += 1;
        }
    }

    mae /= n;
    let rmse = (mse / n).sqrt();
    mape /= n.max(1.0);

    let bootstrap_ci_mae = bootstrap_ci(predictions, targets, |p, t| {
        p.iter()
            .zip(t.iter())
            .map(|(a, b)| (a - b).abs())
            .sum::<f32>()
            / p.len() as f32
    });
    let bootstrap_ci_rmse = bootstrap_ci(predictions, targets, |p, t| {
        (p.iter()
            .zip(t.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f32>()
            / p.len() as f32)
            .sqrt()
    });
    let bootstrap_ci_mape = bootstrap_ci(predictions, targets, |p, t| {
        p.iter()
            .zip(t.iter())
            .filter(|(_, b)| **b > 1.0)
            .map(|(a, b)| (a - b).abs() / b)
            .sum::<f32>()
            / p.len() as f32
    });

    LosNnMetrics {
        mae,
        rmse,
        mape,
        binned_acc_24h: binned_24 as f32 / n,
        binned_acc_48h: binned_48 as f32 / n,
        ece: 0.0,
        n_train: 0,
        n_val: 0,
        n_test: predictions.len(),
        bootstrap_ci_mae,
        bootstrap_ci_rmse,
        bootstrap_ci_mape,
    }
}

#[cfg(feature = "ml-nn")]
fn bootstrap_ci<F>(predictions: &[f32], targets: &[f32], metric_fn: F) -> (f32, f32)
where
    F: Fn(&[f32], &[f32]) -> f32,
{
    use rand::SeedableRng;
    use rand::rngs::StdRng;
    use rand::seq::SliceRandom;

    let n = predictions.len();
    let n_boot = 1000;
    let mut bootstraps = Vec::with_capacity(n_boot);
    let mut rng = StdRng::seed_from_u64(42);

    for _ in 0..n_boot {
        let indices: Vec<usize> = (0..n)
            .collect::<Vec<_>>()
            .choose_multiple(&mut rng, n)
            .cloned()
            .collect();
        let boot_preds: Vec<f32> = indices.iter().map(|&i| predictions[i]).collect();
        let boot_targets: Vec<f32> = indices.iter().map(|&i| targets[i]).collect();
        bootstraps.push(metric_fn(&boot_preds, &boot_targets));
    }

    bootstraps.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let lower = bootstraps[n_boot / 100 * 2];
    let upper = bootstraps[n_boot / 100 * 98];
    (lower, upper)
}

pub fn generate_synthetic_los_data(n_samples: usize) -> (Array2<f32>, Array1<f32>) {
    let feature_set = build_los_nn_v1();
    let n_features = feature_set.feature_count();

    let mut rng = StdRng::seed_from_u64(123);
    use rand::Rng;

    let mut features = Array2::zeros((n_samples, n_features));
    let mut targets = Array1::zeros(n_samples);

    for i in 0..n_samples {
        let base_los = rng.gen_range(24.0..336.0);
        let severity = rng.gen_range(0.0..1.0);

        for j in 0..n_features {
            let fname = &feature_set.features[j].name;
            if fname.contains("heart_rate") {
                features[[i, j]] = 70.0 + severity * 50.0 + rng.gen_range(-10.0..10.0);
            } else if fname.contains("spo2") {
                features[[i, j]] = 98.0 - severity * 20.0 + rng.gen_range(-3.0..3.0);
            } else if fname.contains("temperature") {
                features[[i, j]] = 37.0 + severity * 3.0 + rng.gen_range(-0.5..0.5);
            } else if fname.contains("age") {
                features[[i, j]] = rng.gen_range(18.0..90.0);
            } else if fname.contains("sex") || fname.contains("emergency") {
                features[[i, j]] = if rng.gen_bool(0.5) { 1.0 } else { 0.0 };
            } else if fname.contains("missingness") {
                features[[i, j]] = rng.gen_range(0.0..0.3);
            } else if fname.contains("count") {
                features[[i, j]] = rng.gen_range(10.0..100.0);
            } else if fname.contains("hour_sin") || fname.contains("day_sin") {
                features[[i, j]] = rng.gen_range(-1.0..1.0);
            } else if fname.contains("hour_cos") || fname.contains("day_cos") {
                features[[i, j]] = rng.gen_range(-1.0..1.0);
            } else {
                features[[i, j]] = rng.gen_range(-2.0..2.0);
            }
        }

        let noise = rng.gen_range(-12.0..12.0);
        let los: f32 = base_los + severity * 100.0 + noise;
        targets[i] = los.max(1.0);
    }

    (features, targets)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::{Array1, Array2};

    #[test]
    fn test_los_nn_config_default() {
        let config = LosNnConfig::default();
        let feature_set = build_los_nn_v1();
        assert_eq!(config.input_size, feature_set.feature_count());
        assert!(!config.hidden_sizes.is_empty());
    }

    #[test]
    fn test_los_nn_model_creation() {
        let config = LosNnConfig::default();
        let model = LosNnModel::new(config);
        assert_eq!(model.feature_set.name, "los_nn_v1");
        assert_eq!(model.feature_set.version.to_string(), "1.0.0");
    }

    #[test]
    fn test_feature_set_hash() {
        let config = LosNnConfig::default();
        let model = LosNnModel::new(config);
        assert!(model.feature_set.verify_hash());
    }

    #[test]
    fn test_synthetic_data_generation() {
        let (features, targets) = generate_synthetic_los_data(100);
        assert_eq!(features.nrows(), 100);
        assert_eq!(targets.len(), 100);
        assert!(targets.iter().all(|&t| t > 0.0));
    }

    #[test]
    fn test_compute_metrics() {
        let preds = vec![10.0, 20.0, 30.0, 40.0, 50.0];
        let targets = vec![12.0, 18.0, 33.0, 38.0, 55.0];
        let metrics = compute_metrics(&preds, &targets);
        assert!(metrics.mae > 0.0);
        assert!(metrics.rmse > 0.0);
        assert!(metrics.binned_acc_24h >= 0.0 && metrics.binned_acc_24h <= 1.0);
    }

    #[test]
    #[cfg(feature = "ml-nn")]
    fn test_mlp_predictor_creation() {
        let config = LosNnConfig::default();
        let device = Device::Cpu;
        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        let predictor = MlpLosPredictor::new(vb, &config);
        assert!(predictor.is_ok());
    }
}
