//! Validación de rangos clínicos para datos de pacientes UCI
//!
//! Este módulo proporciona funciones para validar que los valores
//! de las mediciones clínicas estén dentro de rangos físicos posibles

use crate::models::{ApacheIIData, GcsData};

/// Resultado de validación clínica
#[derive(Debug, Clone, serde::Serialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub warnings: Vec<ValidationWarning>,
    pub errors: Vec<ValidationError>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ValidationWarning {
    pub field: String,
    pub message: String,
    pub value: f32,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
    pub value: f32,
}

/// Rangos válidos para cada variable del APACHE II
pub struct ClinicalRange {
    pub name: &'static str,
    pub min: f32,
    pub max: f32,
    pub critical_low: Option<f32>,
    pub critical_high: Option<f32>,
    pub unit: &'static str,
}

pub const APACHE_RANGES: &[ClinicalRange] = &[
    ClinicalRange {
        name: "temperatura",
        min: 25.0,
        max: 45.0,
        critical_low: Some(30.0),
        critical_high: Some(42.0),
        unit: "°C",
    },
    ClinicalRange {
        name: "presion_arterial_media",
        min: 0.0,
        max: 250.0,
        critical_low: Some(40.0),
        critical_high: Some(200.0),
        unit: "mmHg",
    },
    ClinicalRange {
        name: "frecuencia_cardiaca",
        min: 0.0,
        max: 250.0,
        critical_low: Some(30.0),
        critical_high: Some(200.0),
        unit: "lpm",
    },
    ClinicalRange {
        name: "frecuencia_respiratoria",
        min: 0.0,
        max: 60.0,
        critical_low: Some(5.0),
        critical_high: Some(50.0),
        unit: "rpm",
    },
    ClinicalRange {
        name: "fio2",
        min: 0.21,
        max: 1.0,
        critical_low: None,
        critical_high: None,
        unit: "",
    },
    ClinicalRange {
        name: "pao2",
        min: 0.0,
        max: 600.0,
        critical_low: Some(40.0),
        critical_high: None,
        unit: "mmHg",
    },
    ClinicalRange {
        name: "a_ado2",
        min: 0.0,
        max: 700.0,
        critical_low: None,
        critical_high: Some(600.0),
        unit: "mmHg",
    },
    ClinicalRange {
        name: "ph_arterial",
        min: 6.8,
        max: 7.8,
        critical_low: Some(7.0),
        critical_high: Some(7.7),
        unit: "",
    },
    ClinicalRange {
        name: "sodio_serico",
        min: 100.0,
        max: 200.0,
        critical_low: Some(120.0),
        critical_high: Some(170.0),
        unit: "mEq/L",
    },
    ClinicalRange {
        name: "potasio_serico",
        min: 1.5,
        max: 9.0,
        critical_low: Some(2.5),
        critical_high: Some(7.0),
        unit: "mEq/L",
    },
    ClinicalRange {
        name: "creatinina",
        min: 0.1,
        max: 15.0,
        critical_low: None,
        critical_high: Some(10.0),
        unit: "mg/dL",
    },
    ClinicalRange {
        name: "hematocrito",
        min: 5.0,
        max: 75.0,
        critical_low: Some(15.0),
        critical_high: Some(60.0),
        unit: "%",
    },
    ClinicalRange {
        name: "leucocitos",
        min: 0.1,
        max: 100.0,
        critical_low: Some(0.5),
        critical_high: Some(50.0),
        unit: "x10³",
    },
];

/// Valida una medición de APACHE II
pub fn validate_apache_measurement(data: &ApacheIIData) -> ValidationResult {
    let mut warnings = Vec::new();
    let mut errors = Vec::new();

    // Temperatura
    validate_field(
        "temperatura",
        data.temperatura,
        25.0,
        45.0,
        Some(30.0),
        Some(42.0),
        &mut warnings,
        &mut errors,
    );

    // Presión arterial media
    validate_field(
        "presion_arterial_media",
        data.presion_arterial_media,
        0.0,
        250.0,
        Some(40.0),
        Some(200.0),
        &mut warnings,
        &mut errors,
    );

    // Frecuencia cardíaca
    validate_field(
        "frecuencia_cardiaca",
        data.frecuencia_cardiaca,
        0.0,
        250.0,
        Some(30.0),
        Some(200.0),
        &mut warnings,
        &mut errors,
    );

    // Frecuencia respiratoria
    validate_field(
        "frecuencia_respiratoria",
        data.frecuencia_respiratoria,
        0.0,
        60.0,
        Some(5.0),
        Some(50.0),
        &mut warnings,
        &mut errors,
    );

    // FiO2
    if data.fio2 < 0.21 {
        errors.push(ValidationError {
            field: "fio2".to_string(),
            message: "FiO2 no puede ser menor a 0.21 (aire ambiente)".to_string(),
            value: data.fio2,
        });
    } else if data.fio2 > 1.0 {
        errors.push(ValidationError {
            field: "fio2".to_string(),
            message: "FiO2 no puede exceder 1.0 (100% oxígeno)".to_string(),
            value: data.fio2,
        });
    }

    // pH arterial
    validate_field(
        "ph_arterial",
        data.ph_arterial,
        6.8,
        7.8,
        Some(7.0),
        Some(7.7),
        &mut warnings,
        &mut errors,
    );

    // Sodio sérico
    validate_field(
        "sodio_serico",
        data.sodio_serico,
        100.0,
        200.0,
        Some(120.0),
        Some(170.0),
        &mut warnings,
        &mut errors,
    );

    // Potasio sérico
    validate_field(
        "potasio_serico",
        data.potasio_serico,
        1.5,
        9.0,
        Some(2.5),
        Some(7.0),
        &mut warnings,
        &mut errors,
    );

    // Creatinina
    validate_field(
        "creatinina",
        data.creatinina,
        0.1,
        15.0,
        None,
        Some(10.0),
        &mut warnings,
        &mut errors,
    );

    // Hematocrito
    validate_field(
        "hematocrito",
        data.hematocrito,
        5.0,
        75.0,
        Some(15.0),
        Some(60.0),
        &mut warnings,
        &mut errors,
    );

    // Leucocitos
    validate_field(
        "leucocitos",
        data.leucocitos,
        0.1,
        100.0,
        Some(0.5),
        Some(50.0),
        &mut warnings,
        &mut errors,
    );

    // PaO2 (si aplica)
    if let Some(pao2) = data.pao2 {
        validate_field(
            "pao2",
            pao2,
            0.0,
            600.0,
            Some(40.0),
            None,
            &mut warnings,
            &mut errors,
        );
    }

    // A-aDO2 (si aplica)
    if let Some(a_ado2) = data.a_ado2 {
        validate_field(
            "a_ado2",
            a_ado2,
            0.0,
            700.0,
            None,
            Some(600.0),
            &mut warnings,
            &mut errors,
        );
    }

    // Edad
    if data.edad > 120 {
        errors.push(ValidationError {
            field: "edad".to_string(),
            message: "La edad no puede exceder 120 años".to_string(),
            value: data.edad as f32,
        });
    }

    ValidationResult {
        valid: errors.is_empty(),
        warnings,
        errors,
    }
}

/// Valida una medición de GCS
pub fn validate_gcs_measurement(gcs: &GcsData) -> ValidationResult {
    let warnings = Vec::new();
    let mut errors = Vec::new();

    // Validar componentes del GCS
    if gcs.apertura_ocular < 1 || gcs.apertura_ocular > 4 {
        errors.push(ValidationError {
            field: "apertura_ocular".to_string(),
            message: "Apertura ocular debe estar entre 1 y 4".to_string(),
            value: gcs.apertura_ocular as f32,
        });
    }

    if gcs.respuesta_verbal < 1 || gcs.respuesta_verbal > 5 {
        errors.push(ValidationError {
            field: "respuesta_verbal".to_string(),
            message: "Respuesta verbal debe estar entre 1 y 5".to_string(),
            value: gcs.respuesta_verbal as f32,
        });
    }

    if gcs.respuesta_motora < 1 || gcs.respuesta_motora > 6 {
        errors.push(ValidationError {
            field: "respuesta_motora".to_string(),
            message: "Respuesta motora debe estar entre 1 y 6".to_string(),
            value: gcs.respuesta_motora as f32,
        });
    }

    // Verificar que el total coincida
    let total = gcs.total();
    if total < 3 || total > 15 {
        errors.push(ValidationError {
            field: "gcs_total".to_string(),
            message: "GCS total debe estar entre 3 y 15".to_string(),
            value: total as f32,
        });
    }

    ValidationResult {
        valid: errors.is_empty(),
        warnings,
        errors,
    }
}

fn validate_field(
    name: &str,
    value: f32,
    min: f32,
    max: f32,
    critical_low: Option<f32>,
    critical_high: Option<f32>,
    warnings: &mut Vec<ValidationWarning>,
    errors: &mut Vec<ValidationError>,
) {
    // Verificar rango físico
    if value < min {
        errors.push(ValidationError {
            field: name.to_string(),
            message: format!("{} está por debajo del rango físico posible", name),
            value,
        });
        return;
    }

    if value > max {
        errors.push(ValidationError {
            field: name.to_string(),
            message: format!("{} excede el valor máximo posible", name),
            value,
        });
        return;
    }

    // Verificar valores críticos
    if let Some(crit_low) = critical_low {
        if value < crit_low {
            warnings.push(ValidationWarning {
                field: name.to_string(),
                message: format!("{} está en rango crítico bajo", name),
                value,
            });
        }
    }

    if let Some(crit_high) = critical_high {
        if value > crit_high {
            warnings.push(ValidationWarning {
                field: name.to_string(),
                message: format!("{} está en rango crítico alto", name),
                value,
            });
        }
    }
}

/// Obtiene una descripción del rango válido para una variable
pub fn get_range_description(name: &str) -> Option<String> {
    APACHE_RANGES.iter().find(|r| r.name == name).map(|r| {
        format!(
            "{}: {} - {} {} (crítico: {:?} - {:?})",
            r.name, r.min, r.max, r.unit, r.critical_low, r.critical_high
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_patient() {
        let data = ApacheIIData {
            temperatura: 37.0,
            presion_arterial_media: 80.0,
            frecuencia_cardiaca: 75.0,
            frecuencia_respiratoria: 14.0,
            fio2: 0.21,
            pao2: Some(85.0),
            a_ado2: None,
            ph_arterial: 7.40,
            sodio_serico: 140.0,
            potasio_serico: 4.0,
            creatinina: 1.0,
            falla_renal_aguda: false,
            hematocrito: 42.0,
            leucocitos: 7.0,
            gcs_total: 15,
            edad: 40,
            ..Default::default()
        };

        let result = validate_apache_measurement(&data);
        assert!(result.valid, "Paciente válido no debe tener errores");
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_invalid_temperatura() {
        let data = ApacheIIData {
            temperatura: 50.0, // Físicamente imposible
            ..Default::default()
        };

        let result = validate_apache_measurement(&data);
        assert!(!result.valid);
        assert!(!result.errors.is_empty());
    }

    #[test]
    fn test_warning_temperatura() {
        let data = ApacheIIData {
            temperatura: 29.0, // Crítico bajo pero posible
            ..Default::default()
        };

        let result = validate_apache_measurement(&data);
        assert!(result.valid); // Todavía válido
        assert!(!result.warnings.is_empty()); // Pero con warnings
    }

    #[test]
    fn test_valid_gcs() {
        let gcs = GcsData {
            apertura_ocular: 4,
            respuesta_verbal: 5,
            respuesta_motora: 6,
        };

        let result = validate_gcs_measurement(&gcs);
        assert!(result.valid);
    }

    #[test]
    fn test_invalid_gcs() {
        let gcs = GcsData {
            apertura_ocular: 5, // Inválido (máximo es 4)
            respuesta_verbal: 5,
            respuesta_motora: 6,
        };

        let result = validate_gcs_measurement(&gcs);
        assert!(!result.valid);
    }

    #[test]
    fn test_fio2_validation() {
        let mut data = ApacheIIData::default();

        // FiO2 válido
        data.fio2 = 0.5;
        let result = validate_apache_measurement(&data);
        assert!(result.errors.is_empty());

        // FiO2 inválido (menor que aire ambiente)
        data.fio2 = 0.1;
        let result = validate_apache_measurement(&data);
        assert!(!result.errors.is_empty());
    }
}
