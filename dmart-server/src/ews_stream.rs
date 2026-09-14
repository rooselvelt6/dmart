use crate::hl7::parser::VitalsMessage;
use crate::metrics::{EwsAlgo, EwsSeverity, ews_score_published};
use crate::realtime::{ScoreEvent, publish_event};
use dmart_shared::models::ApacheIIData;
use dmart_shared::scales::{
    calculate_apache_ii_score, calculate_news2_score, calculate_sofa_score,
};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::mpsc;

#[derive(Clone, Debug)]
pub struct EwsConfig {
    pub news2_threshold: u8,
    pub apache_delta_threshold: u8,
    pub channel_capacity: usize,
}

impl Default for EwsConfig {
    fn default() -> Self {
        Self {
            news2_threshold: 5,
            apache_delta_threshold: 5,
            channel_capacity: 10_000,
        }
    }
}

#[derive(Debug)]
pub struct EwsEngine {
    config: EwsConfig,
    last_apache: Arc<Mutex<std::collections::HashMap<String, u32>>>,
}

impl EwsEngine {
    pub fn new(
        config: EwsConfig,
    ) -> (
        Self,
        mpsc::Sender<VitalsMessage>,
        mpsc::Receiver<VitalsMessage>,
    ) {
        let (tx, rx) = mpsc::channel(config.channel_capacity);
        let engine = Self {
            config,
            last_apache: Arc::new(Mutex::new(std::collections::HashMap::new())),
        };
        (engine, tx, rx)
    }

    pub async fn run(self, mut rx: mpsc::Receiver<VitalsMessage>) {
        while let Some(vm) = rx.recv().await {
            let _ = self.process_vitals(vm).await;
        }
    }

    async fn process_vitals(&self, vm: VitalsMessage) -> Result<(), String> {
        let hr = vm
            .vitals
            .iter()
            .find(|v| v.loinc.as_deref() == Some("8867-4"))
            .map(|v| v.value)
            .unwrap_or(80.0);
        let rr = vm
            .vitals
            .iter()
            .find(|v| v.loinc.as_deref() == Some("9279-1"))
            .map(|v| v.value)
            .unwrap_or(16.0);
        let spo2 = vm
            .vitals
            .iter()
            .find(|v| v.loinc.as_deref() == Some("2708-6"))
            .map(|v| v.value)
            .unwrap_or(97.0);
        let temp = vm
            .vitals
            .iter()
            .find(|v| v.loinc.as_deref() == Some("8310-5"))
            .map(|v| v.value)
            .unwrap_or(37.0);

        let apache_data = ApacheIIData {
            temperatura: temp,
            presion_arterial_media: 80.0,
            presion_sistolica: 120.0,
            frecuencia_cardiaca: hr,
            frecuencia_respiratoria: rr,
            fio2: 0.21,
            pao2: None,
            a_ado2: None,
            spo2,
            ph_arterial: 7.4,
            sodio_serico: 140.0,
            potasio_serico: 4.0,
            creatinina: 1.0,
            falla_renal_aguda: false,
            bilirrubina: 1.0,
            hematocrito: 40.0,
            leucocitos: 10.0,
            plaquetas: 200.0,
            gcs_ojos: 4,
            gcs_verbal: 5,
            gcs_motor: 6,
            gcs_total: 15,
            edad: 65,
            insuficiencia_hepatica: false,
            cardiovascular_severa: false,
            insuficiencia_respiratoria: false,
            insuficiencia_renal: false,
            inmunocomprometido: false,
            cirugia_no_operado: false,
            ventilacion_mecanica: false,
            vasopresores: false,
            dosis_vasopresor: 0.0,
            diuresis_diaria: 1500,
            alerta: false,
            o2_suplementario: false,
            nivel_conciencia: "alert".into(),
            bicarbonate: 24.0,
            tipo_admision: Some("medical".into()),
            fuente_admision: Some("emergency_room".into()),
            dias_pre_uci: 0,
            infeccion_admision: Some("none".into()),
            sistema_anatomico: None,
        };

        let news2 = calculate_news2_score(&apache_data);
        let apache = calculate_apache_ii_score(&apache_data);
        let sofa = calculate_sofa_score(&apache_data);

        let mut last = self.last_apache.lock().await;
        let prev = last.get(&vm.patient_ref).copied().unwrap_or(apache);
        let delta = apache.abs_diff(prev);
        last.insert(vm.patient_ref.clone(), apache);
        drop(last);

        let severity = if news2 >= self.config.news2_threshold as u32
            || delta >= self.config.apache_delta_threshold as u32
        {
            EwsSeverity::High
        } else {
            EwsSeverity::Normal
        };

        let score_event = ScoreEvent {
            patient_id: vm.patient_ref,
            apache_score: apache as f64,
            news2_score: news2 as f64,
            sofa_score: sofa as f64,
            timestamp: chrono::Utc::now().to_rfc3339(),
            severity,
        };

        publish_event(score_event);
        ews_score_published(EwsAlgo::NEWS2, severity);
        ews_score_published(EwsAlgo::ApacheII, severity);
        ews_score_published(EwsAlgo::SOFA, severity);

        Ok(())
    }
}

pub async fn spawn_engine(
    config: EwsConfig,
) -> (tokio::task::JoinHandle<()>, mpsc::Sender<VitalsMessage>) {
    let (engine, tx, rx) = EwsEngine::new(config);
    let handle = tokio::spawn(async move { engine.run(rx).await });
    (handle, tx)
}
