//! ML module for mortality prediction from Apache II score
//! Fase 5.9: ML piloto - predicción deterioro (ApacheII→riesgo)

use bincode;
use linfa::prelude::*;
use linfa_trees::DecisionTree;
use ndarray::{Array1, Array2, Axis};
use rand::SeedableRng;
use rand::rngs::StdRng;
use serde::{Deserialize, Serialize};
use std::fs;

use crate::models::ApacheIIData;
use crate::scales::{calculate_apache_ii_score, mortality_risk};

/// Features para el modelo de predicción de mortalidad
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MortalityFeatures {
    pub apache_score: f32,
    pub gcs_total: f32,
    pub edad: f32,
    pub temperatura: f32,
    pub presion_arterial_media: f32,
    pub frecuencia_cardiaca: f32,
    pub frecuencia_respiratoria: f32,
    pub fio2: f32,
    pub spo2: f32,
    pub ph_arterial: f32,
    pub sodio_serico: f32,
    pub creatinina: f32,
    pub leucocitos: f32,
    pub potasio_serico: f32,
}

impl MortalityFeatures {
    pub fn from_apache(data: &ApacheIIData) -> Self {
        Self {
            apache_score: calculate_apache_ii_score(data) as f32,
            gcs_total: data.gcs_total as f32,
            edad: data.edad as f32,
            temperatura: data.temperatura,
            presion_arterial_media: data.presion_arterial_media,
            frecuencia_cardiaca: data.frecuencia_cardiaca,
            frecuencia_respiratoria: data.frecuencia_respiratoria,
            fio2: data.fio2,
            spo2: data.spo2,
            ph_arterial: data.ph_arterial,
            sodio_serico: data.sodio_serico,
            creatinina: data.creatinina,
            leucocitos: data.leucocitos,
            potasio_serico: data.potasio_serico,
        }
    }

    pub fn to_array(&self) -> Array1<f32> {
        Array1::from_vec(vec![
            self.apache_score,
            self.gcs_total,
            self.edad,
            self.temperatura,
            self.presion_arterial_media,
            self.frecuencia_cardiaca,
            self.frecuencia_respiratoria,
            self.fio2,
            self.spo2,
            self.ph_arterial,
            self.sodio_serico,
            self.creatinina,
            self.leucocitos,
            self.potasio_serico,
        ])
    }
}

/// Modelo de predicción de mortalidad (serializable con bincode)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MortalityModel {
    model: DecisionTree<f32, usize>,
    feature_names: Vec<String>,
}

impl MortalityModel {
    /// Entrena un nuevo modelo usando datos sintéticos basados en reglas clínicas
    pub fn train() -> Result<Self, Box<dyn std::error::Error>> {
        let feature_names = vec![
            "apache_score".to_string(),
            "gcs_total".to_string(),
            "edad".to_string(),
            "temperatura".to_string(),
            "presion_arterial_media".to_string(),
            "frecuencia_cardiaca".to_string(),
            "frecuencia_respiratoria".to_string(),
            "fio2".to_string(),
            "spo2".to_string(),
            "ph_arterial".to_string(),
            "sodio_serico".to_string(),
            "creatinina".to_string(),
            "leucocitos".to_string(),
            "potasio_serico".to_string(),
        ];

        // Generar dataset sintético basado en reglas APACHE II
        let (records, targets) = Self::generate_synthetic_data(1000);

        // Convertir a formato linfa
        let n_samples = records.len();
        let n_features = feature_names.len();

        let mut data = Array2::zeros((n_samples, n_features));
        let mut target_array = Array1::zeros(n_samples);

        for (i, (record, target)) in records.into_iter().zip(targets).enumerate() {
            data.row_mut(i).assign(&record);
            target_array[i] = target;
        }

        let dataset = Dataset::new(data, target_array);

        // Entrenar Decision Tree
        let model = DecisionTree::params()
            .max_depth(Some(10))
            .min_weight_split(5.0)
            .min_weight_leaf(2.0)
            .fit(&dataset)?;

        Ok(Self {
            model,
            feature_names,
        })
    }

    /// Genera datos sintéticos para entrenamiento
    /// Basado en reglas APACHE II: mortalidad aumenta con score
    fn generate_synthetic_data(n: usize) -> (Vec<Array1<f32>>, Vec<usize>) {
        let mut rng = StdRng::seed_from_u64(42);
        let mut records = Vec::with_capacity(n);
        let mut targets = Vec::with_capacity(n);

        use rand::Rng;

        for _ in 0..n {
            // Generar features realistas
            let apache_score = rng.gen_range(0..=71) as f32;
            let gcs_total = rng.gen_range(3..=15) as f32;
            let edad = rng.gen_range(18..=100) as f32;
            let temperatura = 35.0 + rng.gen_range(0.0..=5.0);
            let presion_arterial_media = 50.0 + rng.gen_range(0.0..=80.0);
            let frecuencia_cardiaca = 40.0 + rng.gen_range(0.0..=140.0);
            let frecuencia_respiratoria = 5.0 + rng.gen_range(0.0..=40.0);
            let fio2 = 0.21 + rng.gen_range(0.0..=0.79);
            let spo2 = 70.0 + rng.gen_range(0.0..=30.0);
            let ph_arterial = 7.0 + rng.gen_range(0.0..=0.5);
            let sodio_serico = 120.0 + rng.gen_range(0.0..=40.0);
            let creatinina = 0.3 + rng.gen_range(0.0..=5.0);
            let leucocitos = 1.0 + rng.gen_range(0.0..=30.0);
            let potasio_serico = 2.5 + rng.gen_range(0.0..=3.0);

            let record = Array1::from_vec(vec![
                apache_score,
                gcs_total,
                edad,
                temperatura,
                presion_arterial_media,
                frecuencia_cardiaca,
                frecuencia_respiratoria,
                fio2,
                spo2,
                ph_arterial,
                sodio_serico,
                creatinina,
                leucocitos,
                potasio_serico,
            ]);

            // Regla de mortalidad: basada en Apache II score + factores
            let base_mortality = mortality_risk(apache_score as u32);
            let gcs_factor = if gcs_total <= 8.0 {
                0.3
            } else if gcs_total < 13.0 {
                0.15
            } else {
                0.0
            };
            let edad_factor = if edad > 75.0 {
                0.1
            } else if edad > 65.0 {
                0.05
            } else {
                0.0
            };
            let ph_factor = if ph_arterial < 7.2 {
                0.15
            } else if ph_arterial < 7.3 {
                0.05
            } else {
                0.0
            };

            let mortality_prob =
                (base_mortality + gcs_factor + edad_factor + ph_factor).clamp(0.0, 0.95);

            // Añadir ruido
            let mortality_prob = mortality_prob + rng.gen_range(-0.05..=0.05);
            let mortality_prob = mortality_prob.clamp(0.0, 1.0);

            let target = if mortality_prob > 0.5 { 1 } else { 0 };

            records.push(record);
            targets.push(target);
        }

        (records, targets)
    }

    /// Predice mortalidad para un paciente
    pub fn predict(&self, features: &MortalityFeatures) -> f32 {
        let x = features.to_array().insert_axis(Axis(0));
        let pred = self.model.predict(&x);
        pred[0] as f32
    }

    /// Getter for the inner decision tree model
    pub fn model(&self) -> &DecisionTree<f32, usize> {
        &self.model
    }

    /// Getter for feature names
    pub fn feature_names(&self) -> &Vec<String> {
        &self.feature_names
    }

    /// Predice probabilidad de mortalidad (0.0 - 1.0)
    pub fn predict_proba(&self, features: &MortalityFeatures) -> f32 {
        // Para DecisionTree, usar la predicción de clase como proxy
        // En un caso real usaríamos predict_proba si estuviera disponible
        self.predict(features)
    }

    /// Guarda el modelo a disco (serialización binaria con bincode)
    pub fn save(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let encoded = bincode::serialize(self)?;
        fs::write(path, encoded)?;
        Ok(())
    }

    /// Carga el modelo desde disco (deserialización binaria con bincode)
    pub fn load(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let data = fs::read(path)?;
        let model: Self = bincode::deserialize(&data)?;
        Ok(model)
    }

    /// Obtiene importancia de features
    pub fn feature_importance(&self) -> Vec<(String, f32)> {
        // Placeholder - en linfa 0.7 no hay feature_importance directo
        // Se puede implementar con permutation importance
        self.feature_names
            .iter()
            .map(|f| (f.clone(), 1.0))
            .collect()
    }
}

/// Entrena y evalúa el modelo
pub fn train_and_evaluate() -> Result<MortalityModel, Box<dyn std::error::Error>> {
    println!("🤖 Entrenando modelo de predicción de mortalidad...");

    let model = MortalityModel::train()?;

    // Evaluación simple con datos de prueba
    let (test_records, test_targets) = MortalityModel::generate_synthetic_data(200);
    let mut correct = 0;
    let mut total = 0;

    for (record, target) in test_records.into_iter().zip(test_targets) {
        let features = MortalityFeatures {
            apache_score: record[0],
            gcs_total: record[1],
            edad: record[2],
            temperatura: record[3],
            presion_arterial_media: record[4],
            frecuencia_cardiaca: record[5],
            frecuencia_respiratoria: record[6],
            fio2: record[7],
            spo2: record[8],
            ph_arterial: record[9],
            sodio_serico: record[10],
            creatinina: record[11],
            leucocitos: record[12],
            potasio_serico: record[13],
        };

        let pred = model.predict(&features) as usize;
        if pred == target {
            correct += 1;
        }
        total += 1;
    }

    let accuracy = correct as f32 / total as f32;
    println!("✅ Modelo entrenado - Accuracy: {:.2}%", accuracy * 100.0);

    Ok(model)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mortality_features_from_apache() {
        let apache = ApacheIIData::default();
        let features = MortalityFeatures::from_apache(&apache);
        // Default patient has normal vitals but age 50, so score > 0
        assert!(features.apache_score > 0.0 && features.apache_score < 71.0);
    }

    #[test]
    fn test_model_training() {
        let model = MortalityModel::train().unwrap();
        let features = MortalityFeatures {
            apache_score: 30.0,
            gcs_total: 10.0,
            edad: 65.0,
            temperatura: 38.5,
            presion_arterial_media: 70.0,
            frecuencia_cardiaca: 110.0,
            frecuencia_respiratoria: 25.0,
            fio2: 0.6,
            spo2: 92.0,
            ph_arterial: 7.30,
            sodio_serico: 138.0,
            creatinina: 1.5,
            leucocitos: 14.0,
            potasio_serico: 4.2,
        };
        let pred = model.predict(&features);
        assert!((0.0..=1.0).contains(&pred));
    }

    #[test]
    fn test_train_and_evaluate() {
        let model = train_and_evaluate().unwrap();
        assert!(!model.feature_names.is_empty());
    }

    #[test]
    fn test_model_save_load_roundtrip() {
        let model = MortalityModel::train().unwrap();
        let temp_path = "/tmp/test_mortality_model.bin";

        // Save
        model.save(temp_path).unwrap();

        // Load
        let loaded_model = MortalityModel::load(temp_path).unwrap();

        // Test predictions match
        let features = MortalityFeatures {
            apache_score: 30.0,
            gcs_total: 10.0,
            edad: 65.0,
            temperatura: 38.5,
            presion_arterial_media: 70.0,
            frecuencia_cardiaca: 110.0,
            frecuencia_respiratoria: 25.0,
            fio2: 0.6,
            spo2: 92.0,
            ph_arterial: 7.30,
            sodio_serico: 138.0,
            creatinina: 1.5,
            leucocitos: 14.0,
            potasio_serico: 4.2,
        };

        let pred1 = model.predict(&features);
        let pred2 = loaded_model.predict(&features);

        // Predictions should be identical (bit-for-bit same model)
        assert_eq!(
            pred1, pred2,
            "Loaded model predictions differ from original"
        );

        // Cleanup
        std::fs::remove_file(temp_path).ok();
    }
}
