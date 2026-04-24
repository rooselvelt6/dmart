//! Tests de validación para los cálculos de escalas clínicas
//!
//! Referencia: Knaus WA et al. (1985) APACHE II: a severity of disease classification system
//! Crit Care Med. 13(10):818-29

use dmart_shared::models::{ApacheIIData, GcsData};
use dmart_shared::scales::{
    apache_ii_breakdown, calculate_apache_ii_score, calculate_news2_score, calculate_saps_iii_score,
    calculate_sofa_score, mortality_risk, saps_iii_mortality_prediction, sofa_mortality_estimate,
};

// Función helper para crear un paciente con todos los valores en rango normal (0 puntos)
fn paciente_base() -> ApacheIIData {
    ApacheIIData {
        // Vitales normales
        temperatura: 37.0,
        presion_arterial_media: 80.0,
        presion_sistolica: 120.0,
        frecuencia_cardiaca: 75.0,
        frecuencia_respiratoria: 14.0,
        // Oxigenación normal
        fio2: 0.21,
        pao2: Some(85.0),
        a_ado2: None,
        spo2: 98.0,
        // Laboratorios normales
        ph_arterial: 7.40,
        sodio_serico: 140.0,
        potasio_serico: 4.0,
        creatinina: 1.0,
        falla_renal_aguda: false,
        bilirrubina: 0.8,
        hematocrito: 42.0,
        leucocitos: 7.0,
        plaquetas: 250.0,
        // GCS normal
        gcs_ojos: 4,
            gcs_verbal: 5,
            gcs_motor: 6,
            gcs_total: 15,
        // Edad media (sin puntos)
        edad: 40,
        // Sin enfermedades crónicas
        insuficiencia_hepatica: false,
        cardiovascular_severa: false,
        insuficiencia_respiratoria: false,
        insuficiencia_renal: false,
        inmunocomprometido: false,
        cirugia_no_operado: false,
        // Soporte vital
        ventilacion_mecanica: false,
        vasopresores: false,
        dosis_vasopresor: 0.0,
        diuresis_diaria: 1500,
        // NEWS2
        alerta: true,
        o2_suplementario: false,
        nivel_conciencia: String::new(),
        // SAPS III
        bicarbonate: 24.0,
        tipo_admision: None,
        fuente_admision: None,
        dias_pre_uci: 0,
        infeccion_admision: None,
        sistema_anatomico: None,
    }
}

mod apache_ii {
    use super::*;

    // ─────────────────────────────────────────────────────────────────────────────
    // Temperatura - puntos según rango
    // ─────────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_temperatura_normal() {
        let mut data = paciente_base();
        data.temperatura = 37.0;
        let score = calculate_apache_ii_score(&data);
        assert_eq!(score, 0, "37°C debe dar 0 puntos");
    }

    #[test]
    fn test_temperatura_fiebre_alta() {
        let mut data = paciente_base();
        data.temperatura = 39.5;
        let score = calculate_apache_ii_score(&data);
        assert_eq!(score, 3, "39.5°C debe dar 3 puntos (variable 37→39.5 = +3)");
    }

    #[test]
    fn test_temperatura_muy_alta() {
        let mut data = paciente_base();
        data.temperatura = 41.5;
        let score = calculate_apache_ii_score(&data);
        assert_eq!(score, 4, ">41°C debe dar 4 puntos");
    }

    #[test]
    fn test_temperatura_hipotermia() {
        let mut data = paciente_base();
        data.temperatura = 31.0;
        let score = calculate_apache_ii_score(&data);
        assert!(score >= 2, "31°C (hipotermia) debe dar al menos 2 puntos");
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // Presión Arterial Media
    // ─────────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_pam_normal() {
        let mut data = paciente_base();
        data.presion_arterial_media = 80.0;
        let score = calculate_apache_ii_score(&data);
        assert_eq!(score, 0, "PAM 80mmHg debe dar 0 puntos");
    }

    #[test]
    fn test_pam_alta() {
        let mut data = paciente_base();
        data.presion_arterial_media = 150.0;
        let score = calculate_apache_ii_score(&data);
        assert_eq!(score, 3, "PAM 150mmHg debe dar 3 puntos");
    }

    #[test]
    fn test_pam_baja() {
        let mut data = paciente_base();
        data.presion_arterial_media = 45.0;
        let score = calculate_apache_ii_score(&data);
        assert_eq!(score, 4, "PAM 45mmHg debe dar 4 puntos");
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // Frecuencia Cardíaca
    // ─────────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_fc_normal() {
        let mut data = paciente_base();
        data.frecuencia_cardiaca = 75.0;
        let score = calculate_apache_ii_score(&data);
        assert_eq!(score, 0, "FC 75 lpm debe dar 0 puntos");
    }

    #[test]
    fn test_fc_taquicardia() {
        let mut data = paciente_base();
        data.frecuencia_cardiaca = 150.0;
        let score = calculate_apache_ii_score(&data);
        assert_eq!(score, 3, "FC 150 lpm debe dar 3 puntos");
    }

    #[test]
    fn test_fc_bradicardia() {
        let mut data = paciente_base();
        data.frecuencia_cardiaca = 35.0;
        let score = calculate_apache_ii_score(&data);
        assert!(score >= 3, "FC 35 lpm debe dar al menos 3 puntos");
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // Frecuencia Respiratoria
    // ─────────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_fr_normal() {
        let mut data = paciente_base();
        data.frecuencia_respiratoria = 14.0;
        let score = calculate_apache_ii_score(&data);
        assert_eq!(score, 0, "FR 14 rpm debe dar 0 puntos");
    }

    #[test]
    fn test_fr_alta() {
        let mut data = paciente_base();
        data.frecuencia_respiratoria = 36.0;
        let score = calculate_apache_ii_score(&data);
        assert_eq!(score, 3, "FR 36 rpm debe dar 3 puntos");
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // Oxigenación - PaO2
    // ─────────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_pao2_normal() {
        let mut data = paciente_base();
        data.fio2 = 0.21;
        data.pao2 = Some(85.0);
        data.a_ado2 = None;
        let score = calculate_apache_ii_score(&data);
        assert_eq!(score, 0, "PaO2 85mmHg con FiO2 0.21 debe dar 0 puntos");
    }

    #[test]
    fn test_pao2_bajo() {
        let mut data = paciente_base();
        data.fio2 = 0.21;
        data.pao2 = Some(50.0);
        data.a_ado2 = None;
        let score = calculate_apache_ii_score(&data);
        assert!(score >= 3, "PaO2 50mmHg debe dar al menos 3 puntos");
    }

    #[test]
    fn test_pao2_critico() {
        let mut data = paciente_base();
        data.fio2 = 0.21;
        data.pao2 = Some(40.0);
        data.a_ado2 = None;
        let score = calculate_apache_ii_score(&data);
        assert_eq!(score, 4, "PaO2 40mmHg debe dar 4 puntos");
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // Oxigenación - A-aDO2 (cuando FiO2 >= 0.5)
    // ─────────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_aado2_normal() {
        let mut data = paciente_base();
        data.fio2 = 0.5;
        data.pao2 = None;
        data.a_ado2 = Some(100.0);
        let score = calculate_apache_ii_score(&data);
        assert_eq!(score, 0, "A-aDO2 100mmHg con FiO2 0.5 debe dar 0 puntos");
    }

    #[test]
    fn test_aado2_alto() {
        let mut data = paciente_base();
        data.fio2 = 1.0;
        data.pao2 = None;
        data.a_ado2 = Some(400.0);
        let score = calculate_apache_ii_score(&data);
        assert_eq!(score, 3, "A-aDO2 400mmHg debe dar 3 puntos");
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // pH Arterial
    // ─────────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_ph_normal() {
        let mut data = paciente_base();
        data.ph_arterial = 7.40;
        let score = calculate_apache_ii_score(&data);
        assert_eq!(score, 0, "pH 7.40 debe dar 0 puntos");
    }

    #[test]
    fn test_ph_acidosis() {
        let mut data = paciente_base();
        data.ph_arterial = 7.20;
        let score = calculate_apache_ii_score(&data);
        assert!(score >= 2, "pH 7.20 (acidosis) debe dar al menos 2 puntos");
    }

    #[test]
    fn test_ph_alcalosis() {
        let mut data = paciente_base();
        data.ph_arterial = 7.65;
        let score = calculate_apache_ii_score(&data);
        assert_eq!(score, 3, "pH 7.65 (alcalosis) debe dar 3 puntos");
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // Sodio Sérico
    // ─────────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_sodio_normal() {
        let mut data = paciente_base();
        data.sodio_serico = 140.0;
        let score = calculate_apache_ii_score(&data);
        assert_eq!(score, 0, "Na 140 mEq/L debe dar 0 puntos");
    }

    #[test]
    fn test_sodio_alto() {
        let mut data = paciente_base();
        data.sodio_serico = 165.0;
        let score = calculate_apache_ii_score(&data);
        assert_eq!(score, 3, "Na 165 mEq/L debe dar 3 puntos");
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // Potasio Sérico
    // ─────────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_potasio_normal() {
        let mut data = paciente_base();
        data.potasio_serico = 4.0;
        let score = calculate_apache_ii_score(&data);
        assert_eq!(score, 0, "K 4.0 mEq/L debe dar 0 puntos");
    }

    #[test]
    fn test_potasio_alto() {
        let mut data = paciente_base();
        data.potasio_serico = 6.5;
        let score = calculate_apache_ii_score(&data);
        assert_eq!(score, 3, "K 6.5 mEq/L debe dar 3 puntos");
    }

    #[test]
    fn test_potasio_bajo() {
        let mut data = paciente_base();
        data.potasio_serico = 2.7;
        let score = calculate_apache_ii_score(&data);
        assert_eq!(score, 2, "K 2.7 mEq/L debe dar 2 puntos");
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // Creatinina
    // ─────────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_creatinina_normal() {
        let mut data = paciente_base();
        data.creatinina = 1.0;
        data.falla_renal_aguda = false;
        let score = calculate_apache_ii_score(&data);
        assert_eq!(score, 0, "Cr 1.0 mg/dL debe dar 0 puntos");
    }

    #[test]
    fn test_creatinina_alta() {
        let mut data = paciente_base();
        data.creatinina = 2.5;
        data.falla_renal_aguda = false;
        let score = calculate_apache_ii_score(&data);
        assert_eq!(score, 3, "Cr 2.5 mg/dL debe dar 3 puntos");
    }

    #[test]
    fn test_creatinina_falla_aguda() {
        let mut data = paciente_base();
        data.creatinina = 1.5;
        data.falla_renal_aguda = true;
        // Cr 1.5 = 2 puntos, duplicado = 4 puntos
        let score = calculate_apache_ii_score(&data);
        assert!(
            score >= 4,
            "Cr 1.5 con falla aguda debe dar al menos 4 puntos"
        );
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // Hematocrito
    // ─────────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_hematocrito_normal() {
        let mut data = paciente_base();
        data.hematocrito = 42.0;
        let score = calculate_apache_ii_score(&data);
        assert_eq!(score, 0, "Hto 42% debe dar 0 puntos");
    }

    #[test]
    fn test_hematocrito_bajo() {
        let mut data = paciente_base();
        data.hematocrito = 25.0;
        let score = calculate_apache_ii_score(&data);
        assert_eq!(score, 2, "Hto 25% debe dar 2 puntos");
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // Leucocitos
    // ─────────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_leucocitos_normal() {
        let mut data = paciente_base();
        data.leucocitos = 7.0;
        let score = calculate_apache_ii_score(&data);
        assert_eq!(score, 0, "WBC 7.0 x10³ debe dar 0 puntos");
    }

    #[test]
    fn test_leucocitos_alto() {
        let mut data = paciente_base();
        data.leucocitos = 25.0;
        let score = calculate_apache_ii_score(&data);
        assert_eq!(score, 2, "WBC 25.0 x10³ debe dar 2 puntos");
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // Puntos por Edad
    // ─────────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_edad_joven() {
        let mut data = paciente_base();
        data.edad = 30;
        let score = calculate_apache_ii_score(&data);
        assert_eq!(score, 0, "30 años debe dar 0 puntos");
    }

    #[test]
    fn test_edad_media() {
        let mut data = paciente_base();
        data.edad = 50;
        let score = calculate_apache_ii_score(&data);
        assert_eq!(score, 2, "50 años debe dar 2 puntos");
    }

    #[test]
    fn test_edad_anciano() {
        let mut data = paciente_base();
        data.edad = 70;
        let score = calculate_apache_ii_score(&data);
        assert_eq!(score, 5, "70 años debe dar 5 puntos");
    }

    #[test]
    fn test_edad_muy_anciano() {
        let mut data = paciente_base();
        data.edad = 80;
        let score = calculate_apache_ii_score(&data);
        assert_eq!(score, 6, "80 años debe dar 6 puntos");
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // GCS
    // ─────────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_gcs_normal() {
        let mut data = paciente_base();
        data.gcs_total = 15;
        let score = calculate_apache_ii_score(&data);
        assert_eq!(score, 0, "GCS 15 debe dar 0 puntos (15-15=0)");
    }

    #[test]
    fn test_gcs_moderado() {
        let mut data = paciente_base();
        data.gcs_total = 10;
        let score = calculate_apache_ii_score(&data);
        assert_eq!(score, 5, "GCS 10 debe dar 5 puntos (15-10=5)");
    }

    #[test]
    fn test_gcs_coma() {
        let mut data = paciente_base();
        data.gcs_total = 5;
        let score = calculate_apache_ii_score(&data);
        assert_eq!(score, 10, "GCS 5 debe dar 10 puntos (15-5=10)");
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // Score Máximo
    // ─────────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_score_maximo() {
        let data = ApacheIIData {
            temperatura: 42.0,
            presion_arterial_media: 180.0,
            presion_sistolica: 220.0,
            frecuencia_cardiaca: 200.0,
            frecuencia_respiratoria: 55.0,
            fio2: 0.21,
            pao2: Some(30.0),
            a_ado2: None,
            spo2: 85.0,
            ph_arterial: 7.10,
            sodio_serico: 185.0,
            potasio_serico: 7.5,
            creatinina: 4.0,
            falla_renal_aguda: false,
            bilirrubina: 12.0,
            hematocrito: 65.0,
            leucocitos: 45.0,
            plaquetas: 20.0,
            gcs_ojos: 1,
            gcs_verbal: 1,
            gcs_motor: 1,
            gcs_total: 3,
            edad: 80,
            insuficiencia_hepatica: true,
            cardiovascular_severa: true,
            insuficiencia_respiratoria: true,
            insuficiencia_renal: true,
            inmunocomprometido: true,
            cirugia_no_operado: false,
            ventilacion_mecanica: false,
            vasopresores: false,
            dosis_vasopresor: 0.0,
            diuresis_diaria: 200,
            alerta: false,
            o2_suplementario: false,
            nivel_conciencia: String::new(),
            bicarbonate: 12.0,
            tipo_admision: None,
            fuente_admision: None,
            dias_pre_uci: 0,
            infeccion_admision: None,
            sistema_anatomico: None,
        };

        let breakdown = apache_ii_breakdown(&data);
        assert!(breakdown.aps_total <= 60, "APS no puede exceder 60");
        assert!(breakdown.edad_pts <= 6, "Edad no puede exceder 6");
    }
}

mod gcs {
    use super::*;

    #[test]
    fn test_gcs_coma_completo() {
        let gcs = GcsData {
            apertura_ocular: 1,
            respuesta_verbal: 1,
            respuesta_motora: 1,
        };
        assert_eq!(gcs.total(), 3, "GCS mínimo = 3");
    }

    #[test]
    fn test_gcs_normal() {
        let gcs = GcsData {
            apertura_ocular: 4,
            respuesta_verbal: 5,
            respuesta_motora: 6,
        };
        assert_eq!(gcs.total(), 15, "GCS normal = 15");
    }

    #[test]
    fn test_gcs_trauma_moderado() {
        let gcs = GcsData {
            apertura_ocular: 3,
            respuesta_verbal: 4,
            respuesta_motora: 5,
        };
        assert_eq!(gcs.total(), 12, "GCS trauma moderado = 12");
    }

    #[test]
    fn test_gcs_trauma_severo() {
        let gcs = GcsData {
            apertura_ocular: 2,
            respuesta_verbal: 2,
            respuesta_motora: 3,
        };
        assert_eq!(gcs.total(), 7, "GCS trauma severo = 7");
    }

    #[test]
    fn test_gcs_interpretacion_15() {
        let gcs = GcsData {
            apertura_ocular: 4,
            respuesta_verbal: 5,
            respuesta_motora: 6,
        };
        let interp = gcs.interpret();
        assert!(
            interp.contains("Consciente") || interp.contains("Normal"),
            "GCS 15 debe ser normal"
        );
    }

    #[test]
    fn test_gcs_interpretacion_moderada() {
        let gcs = GcsData {
            apertura_ocular: 3,
            respuesta_verbal: 3,
            respuesta_motora: 4,
        };
        let interp = gcs.interpret();
        assert!(
            interp.contains("moderada"),
            "GCS 12 debe ser lesión moderada"
        );
    }

    #[test]
    fn test_gcs_interpretacion_severa() {
        let gcs = GcsData {
            apertura_ocular: 1,
            respuesta_verbal: 1,
            respuesta_motora: 3,
        };
        let interp = gcs.interpret();
        assert!(
            interp.contains("grave") || interp.contains("Coma"),
            "GCS 7 debe ser lesión grave"
        );
    }
}

mod mortalidad {
    use super::*;

    #[test]
    fn test_mortalidad_bajo_riesgo() {
        let riesgo = mortality_risk(5);
        assert!(riesgo < 15.0, "Score 5 debe tener riesgo bajo");
    }

    #[test]
    fn test_mortalidad_riesgo_medio() {
        let riesgo = mortality_risk(15);
        assert!(
            riesgo > 10.0 && riesgo < 40.0,
            "Score 15 debe tener riesgo medio"
        );
    }

    #[test]
    fn test_mortalidad_riesgo_alto() {
        let riesgo = mortality_risk(25);
        assert!(
            riesgo > 25.0 && riesgo < 70.0,
            "Score 25 debe tener riesgo alto"
        );
    }

    #[test]
    fn test_mortalidad_riesgo_critico() {
        let riesgo = mortality_risk(35);
        assert!(riesgo > 50.0, "Score 35 debe tener riesgo muy alto");
    }

    #[test]
    fn test_mortalidad_aumenta_con_score() {
        let riesgo_10 = mortality_risk(10);
        let riesgo_30 = mortality_risk(30);
        assert!(riesgo_30 > riesgo_10, "Mayor score = mayor mortalidad");
    }
}

mod integracion {
    use super::*;

    #[test]
    fn test_paciente_realistico_critico() {
        let data = ApacheIIData {
            temperatura: 39.5,
            presion_arterial_media: 55.0,
            presion_sistolica: 80.0,
            frecuencia_cardiaca: 120.0,
            frecuencia_respiratoria: 32.0,
            fio2: 0.6,
            pao2: Some(60.0),
            a_ado2: Some(180.0),
            spo2: 88.0,
            ph_arterial: 7.35,
            sodio_serico: 135.0,
            potasio_serico: 4.2,
            creatinina: 1.8,
            falla_renal_aguda: false,
            bilirrubina: 2.5,
            hematocrito: 35.0,
            leucocitos: 18.0,
            plaquetas: 150.0,
            gcs_ojos: 2,
            gcs_verbal: 2,
            gcs_motor: 2,
            gcs_total: 12,
            edad: 65,
            insuficiencia_hepatica: false,
            cardiovascular_severa: true,
            insuficiencia_respiratoria: true,
            insuficiencia_renal: false,
            inmunocomprometido: false,
            cirugia_no_operado: true,
            ventilacion_mecanica: false,
            vasopresores: false,
            dosis_vasopresor: 0.0,
            diuresis_diaria: 800,
            alerta: true,
            o2_suplementario: true,
            nivel_conciencia: String::new(),
            bicarbonate: 20.0,
            tipo_admision: Some("medical".to_string()),
            fuente_admision: None,
            dias_pre_uci: 2,
            infeccion_admision: None,
            sistema_anatomico: None,
        };

        let score = calculate_apache_ii_score(&data);
        let riesgo = mortality_risk(score);

        assert!(score > 10, "Paciente crítico debe tener score >10");
        assert!(score < 71, "Score no puede exceder 71");
        assert!(riesgo > 5.0, "Debe tener riesgo de mortalidad");
    }

    #[test]
    fn test_paciente_estable() {
        let data = paciente_base();

        let score = calculate_apache_ii_score(&data);
        let riesgo = mortality_risk(score);

        assert!(score < 10, "Paciente estable debe tener score <10");
        assert!(riesgo < 15.0, "Paciente estable debe tener riesgo bajo");
    }
}

mod news2_tests {
    use super::*;

    #[test]
    fn test_news2_paciente_estable() {
        let mut data = paciente_base();
        data.spo2 = 98.0;
        data.frecuencia_respiratoria = 14.0;
        data.frecuencia_cardiaca = 70.0;
        data.presion_sistolica = 120.0;
        data.temperatura = 37.0;
        data.alerta = true;

        let score = calculate_news2_score(&data);
        assert!(score < 5, "NEWS2 estable debe ser < 5, got: {}", score);
    }

    #[test]
    fn test_news2_paciente_alto() {
        let mut data = paciente_base();
        data.spo2 = 88.0;
        data.frecuencia_respiratoria = 28.0;
        data.frecuencia_cardiaca = 130.0;
        data.presion_sistolica = 90.0;
        data.temperatura = 39.0;
        data.alerta = false;

        let score = calculate_news2_score(&data);
        assert!(score >= 7, "NEWS2 crítico debe ser >= 7, got: {}", score);
    }

    #[test]
    fn test_news2_con_hipoxemia() {
        let mut data = paciente_base();
        data.spo2 = 85.0;
        data.o2_suplementario = true;

        let score = calculate_news2_score(&data);
        assert!(score >= 3, "NEWS2 con hipoxemia debe dar puntos");
    }
}

mod saps3_tests {
    use super::*;

    #[test]
    fn test_saps3_paciente_estable() {
        let mut data = paciente_base();
        data.edad = 35;
        data.inmunocomprometido = false;
        data.vasopresores = false;
        data.tipo_admision = Some("scheduled_surgical".to_string());
        data.fuente_admision = None;
        data.dias_pre_uci = 0;
        data.infeccion_admision = None;
        data.gcs_total = 15;

        let score = calculate_saps_iii_score(&data);
        let mort = saps_iii_mortality_prediction(score);
        assert!(score < 30, "SAPS3 estable debe ser < 30, got: {}", score);
        assert!(mort < 10.0, "Mortalidad estable debe ser < 10%, got: {}%", mort);
    }

    #[test]
    fn test_saps3_paciente_critico() {
        let mut data = paciente_base();
        data.edad = 75;
        data.inmunocomprometido = true;
        data.vasopresores = true;
        data.dosis_vasopresor = 10.0;
        data.tipo_admision = Some("medical".to_string());
        data.fuente_admision = Some("emergency_room".to_string());
        data.dias_pre_uci = 5;
        data.infeccion_admision = Some("nosocomial".to_string());
        data.gcs_total = 8;

        let score = calculate_saps_iii_score(&data);
        let mort = saps_iii_mortality_prediction(score);
        assert!(score >= 50, "SAPS3 crítico debe ser >= 50, got: {}", score);
        assert!(mort >= 15.0, "Mortalidad crítica debe ser >= 15%, got: {}%", mort);
    }

    #[test]
    fn test_saps3_edad_puntos() {
        let mut data = paciente_base();
        data.edad = 30;
        let score_joven = calculate_saps_iii_score(&data);
        data.edad = 80;
        let score_anciano = calculate_saps_iii_score(&data);
        assert!(score_anciano > score_joven, " mayor edad debe dar más puntos");
    }
}

mod sofa_tests {
    use super::*;

    #[test]
    fn test_sofa_paciente_estable() {
        let mut data = paciente_base();
        data.bilirrubina = 0.8;
        data.plaquetas = 250.0;
        data.gcs_total = 15;
        data.creatinina = 1.0;
        data.diuresis_diaria = 1500;
        data.vasopresores = false;
        data.fio2 = 0.21;
        data.pao2 = Some(85.0);

        let score = calculate_sofa_score(&data);
        let mort = sofa_mortality_estimate(score);
        assert!(score < 5, "SOFA estable debe ser < 5, got: {}", score);
        assert!(mort < 10.0, "Mortalidad estable debe ser < 10%, got: {}%", mort);
    }

    #[test]
    fn test_sofa_fallo_multiple() {
        let mut data = paciente_base();
        data.bilirrubina = 8.0;
        data.plaquetas = 40.0;
        data.gcs_total = 6;
        data.creatinina = 3.5;
        data.diuresis_diaria = 200;
        data.vasopresores = true;
        data.dosis_vasopresor = 15.0;
        data.fio2 = 0.8;
        data.pao2 = Some(60.0);

        let score = calculate_sofa_score(&data);
        let mort = sofa_mortality_estimate(score);
        assert!(score >= 10, "SOFA fallo múltiple debe ser >= 10, got: {}", score);
        assert!(mort >= 30.0, "Mortalidad alta debe ser >= 30%, got: {}%", mort);
    }

    #[test]
    fn test_sofa_renal() {
        let mut data = paciente_base();
        data.diuresis_diaria = 300;
        data.creatinina = 4.0;
        let score = calculate_sofa_score(&data);
        assert!(score >= 2, "SOFA con falla renal debe dar puntos");
    }

    #[test]
    fn test_sofa_coagulacion() {
        let mut data = paciente_base();
        data.plaquetas = 50.0;
        let score = calculate_sofa_score(&data);
        assert!(score >= 1, "SOFA con plaquetas bajas debe dar puntos");
    }
}
