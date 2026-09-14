//! Data Quality validation para HL7 ingest
//! Validación de rangos clínicos, muestras inválidas, métricas de calidad

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Rangos válidos para signos vitales (configurables vía env en futuro)
#[derive(Debug, Clone)]
pub struct VitalRanges {
    pub hr: (i32, i32),     // Heart Rate: 20-300 bpm
    pub spo2: (i32, i32),   // SpO2: 50-100%
    pub bp_sys: (i32, i32), // BP Systolic: 40-300 mmHg
    pub bp_dia: (i32, i32), // BP Diastolic: 20-200 mmHg
    pub rr: (i32, i32),     // Respiratory Rate: 2-100 rpm
    pub temp: (f64, f64),   // Temperature: 30.0-45.0 °C
    pub etco2: (i32, i32),  // EtCO2: 10-100 mmHg
    pub cvp: (i32, i32),    // CVP: -10-50 mmHg
    pub icp: (i32, i32),    // ICP: 0-100 mmHg
}

impl Default for VitalRanges {
    fn default() -> Self {
        Self {
            hr: (20, 300),
            spo2: (50, 100),
            bp_sys: (40, 300),
            bp_dia: (20, 200),
            rr: (2, 100),
            temp: (30.0, 45.0),
            etco2: (10, 100),
            cvp: (-10, 50),
            icp: (0, 100),
        }
    }
}

/// Resultado de validación de una muestra
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationResult {
    Valid,
    Invalid {
        vital: String,
        value: String,
        reason: ValidationReason,
    },
}

/// Razones de invalidación
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationReason {
    OutOfRange,
    MissingRequired,
    Malformed,
    ParseError,
}

/// Validador de calidad de datos HL7
#[derive(Debug, Clone)]
pub struct QualityValidator {
    ranges: VitalRanges,
    // Contadores por device/vital/reason
    invalid_counts: HashMap<String, u64>,
}

impl QualityValidator {
    pub fn new() -> Self {
        Self {
            ranges: VitalRanges::default(),
            invalid_counts: HashMap::new(),
        }
    }

    /// Valida un valor vital contra rangos clínicos
    pub fn validate_vital(
        &mut self,
        device_id: &str,
        vital: &str,
        value: &str,
    ) -> ValidationResult {
        let key = format!("{}:{}:out_of_range", device_id, vital);

        let parsed: f64 = match value.parse() {
            Ok(v) => v,
            Err(_) => {
                *self
                    .invalid_counts
                    .entry(format!("{}:{}:parse_error", device_id, vital))
                    .or_insert(0) += 1;
                return ValidationResult::Invalid {
                    vital: vital.to_string(),
                    value: value.to_string(),
                    reason: ValidationReason::ParseError,
                };
            }
        };

        let (min, max) = match vital.to_uppercase().as_str() {
            "HR" => (self.ranges.hr.0 as f64, self.ranges.hr.1 as f64),
            "SPO2" => (self.ranges.spo2.0 as f64, self.ranges.spo2.1 as f64),
            "BP_SYS" | "SBP" => (self.ranges.bp_sys.0 as f64, self.ranges.bp_sys.1 as f64),
            "BP_DIA" | "DBP" => (self.ranges.bp_dia.0 as f64, self.ranges.bp_dia.1 as f64),
            "RR" => (self.ranges.rr.0 as f64, self.ranges.rr.1 as f64),
            "TEMP" => (self.ranges.temp.0, self.ranges.temp.1),
            "ETCO2" => (self.ranges.etco2.0 as f64, self.ranges.etco2.1 as f64),
            "CVP" => (self.ranges.cvp.0 as f64, self.ranges.cvp.1 as f64),
            "ICP" => (self.ranges.icp.0 as f64, self.ranges.icp.1 as f64),
            _ => {
                // Vital no reconocido: aceptar pero loggear
                return ValidationResult::Valid;
            }
        };

        if parsed < min || parsed > max {
            *self.invalid_counts.entry(key).or_insert(0) += 1;
            ValidationResult::Invalid {
                vital: vital.to_string(),
                value: value.to_string(),
                reason: ValidationReason::OutOfRange,
            }
        } else {
            ValidationResult::Valid
        }
    }

    /// Valida campos requeridos en mensaje HL7
    pub fn validate_required_fields(
        &mut self,
        device_id: &str,
        fields: &HashMap<String, String>,
    ) -> Vec<ValidationResult> {
        let required = ["MSH.3", "MSH.4", "MSH.7", "MSH.9", "MSH.10", "PID.3"];
        let mut results = Vec::new();

        for field in required {
            if fields.get(field).is_none_or(|v| v.is_empty()) {
                let key = format!("{}:{}:missing_required", device_id, field);
                *self.invalid_counts.entry(key).or_insert(0) += 1;
                results.push(ValidationResult::Invalid {
                    vital: field.to_string(),
                    value: "".to_string(),
                    reason: ValidationReason::MissingRequired,
                });
            }
        }

        results
    }

    /// Obtiene contadores de inválidos para métricas
    pub fn invalid_counts(&self) -> &HashMap<String, u64> {
        &self.invalid_counts
    }

    /// Reset contadores (para testing o rotación)
    pub fn reset_counts(&mut self) {
        self.invalid_counts.clear();
    }
}

impl Default for QualityValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Métricas agregadas de calidad para exportación Prometheus
#[derive(Debug, Default, Clone)]
pub struct QualityMetrics {
    pub invalid_total: u64,
    pub by_device_vital_reason: HashMap<String, u64>,
}

impl QualityMetrics {
    pub fn from_validator(validator: &QualityValidator) -> Self {
        let mut by_device_vital_reason = HashMap::new();
        let mut invalid_total = 0;

        for (key, count) in &validator.invalid_counts {
            by_device_vital_reason.insert(key.clone(), *count);
            invalid_total += count;
        }

        Self {
            invalid_total,
            by_device_vital_reason,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_vital_out_of_range() {
        let mut qv = QualityValidator::new();

        // HR = 9999 (imposible)
        let result = qv.validate_vital("MON-001", "HR", "9999");
        assert!(matches!(
            result,
            ValidationResult::Invalid {
                reason: ValidationReason::OutOfRange,
                ..
            }
        ));

        // SpO2 = 150% (imposible)
        let result = qv.validate_vital("MON-001", "SPO2", "150");
        assert!(matches!(
            result,
            ValidationResult::Invalid {
                reason: ValidationReason::OutOfRange,
                ..
            }
        ));

        // Temp = 50°C (imposible)
        let result = qv.validate_vital("MON-001", "TEMP", "50");
        assert!(matches!(
            result,
            ValidationResult::Invalid {
                reason: ValidationReason::OutOfRange,
                ..
            }
        ));
    }

    #[test]
    fn test_validate_vital_valid_range() {
        let mut qv = QualityValidator::new();

        assert_eq!(
            qv.validate_vital("MON-001", "HR", "80"),
            ValidationResult::Valid
        );
        assert_eq!(
            qv.validate_vital("MON-001", "SPO2", "98"),
            ValidationResult::Valid
        );
        assert_eq!(
            qv.validate_vital("MON-001", "BP_SYS", "120"),
            ValidationResult::Valid
        );
        assert_eq!(
            qv.validate_vital("MON-001", "BP_DIA", "80"),
            ValidationResult::Valid
        );
        assert_eq!(
            qv.validate_vital("MON-001", "RR", "16"),
            ValidationResult::Valid
        );
        assert_eq!(
            qv.validate_vital("MON-001", "TEMP", "36.5"),
            ValidationResult::Valid
        );
    }

    #[test]
    fn test_validate_vital_parse_error() {
        let mut qv = QualityValidator::new();

        let result = qv.validate_vital("MON-001", "HR", "abc");
        assert!(matches!(
            result,
            ValidationResult::Invalid {
                reason: ValidationReason::ParseError,
                ..
            }
        ));
    }

    #[test]
    fn test_validate_vital_unknown_vital() {
        let mut qv = QualityValidator::new();

        // Vital no en lista: se acepta (no bloquear)
        assert_eq!(
            qv.validate_vital("MON-001", "UNKNOWN_VITAL", "999"),
            ValidationResult::Valid
        );
    }

    #[test]
    fn test_validate_required_fields() {
        let mut qv = QualityValidator::new();
        let mut fields = HashMap::new();

        fields.insert("MSH.3".to_string(), "PHILIPS".to_string());
        fields.insert("MSH.4".to_string(), "MX800".to_string());
        // MSH.7 faltante
        fields.insert("MSH.9".to_string(), "ORU^R01".to_string());
        fields.insert("MSH.10".to_string(), "12345".to_string());
        fields.insert("PID.3".to_string(), "PATIENT123".to_string());

        let results = qv.validate_required_fields("MON-001", &fields);
        assert_eq!(results.len(), 1);
        assert!(matches!(
            &results[0],
            ValidationResult::Invalid { vital, reason: ValidationReason::MissingRequired, .. } if vital == "MSH.7"
        ));
    }

    #[test]
    fn test_invalid_counts_aggregation() {
        let mut qv = QualityValidator::new();

        qv.validate_vital("MON-001", "HR", "9999");
        qv.validate_vital("MON-001", "HR", "10000");
        qv.validate_vital("MON-002", "SPO2", "150");

        let metrics = QualityMetrics::from_validator(&qv);
        assert_eq!(metrics.invalid_total, 3);
        assert_eq!(
            metrics
                .by_device_vital_reason
                .get("MON-001:HR:out_of_range"),
            Some(&2)
        );
        assert_eq!(
            metrics
                .by_device_vital_reason
                .get("MON-002:SPO2:out_of_range"),
            Some(&1)
        );
    }

    #[test]
    fn test_quality_validator_reset() {
        let mut qv = QualityValidator::new();
        qv.validate_vital("MON-001", "HR", "9999");
        qv.reset_counts();

        let metrics = QualityMetrics::from_validator(&qv);
        assert_eq!(metrics.invalid_total, 0);
    }
}
