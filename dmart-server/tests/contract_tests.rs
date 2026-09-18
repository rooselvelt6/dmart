//! Golden JSON contract tests for API stability
//! Ensures API request/response schemas don't change unexpectedly

#![recursion_limit = "256"]

use dmart_shared::models::*;
use serde_json::json;

/// Test that Patient model serializes to expected JSON structure
#[test]
fn test_patient_golden_json() {
    let patient = Patient::new();
    let json = serde_json::to_value(&patient).expect("serialize patient");

    // Required fields must be present
    assert!(json.get("patient_id").is_some());
    assert!(json.get("tenant_id").is_some());
    assert!(json.get("nombre").is_some());
    assert!(json.get("apellido").is_some());
    assert!(json.get("sexo").is_some());
    assert!(json.get("cedula").is_some());
    assert!(json.get("color_piel").is_some());
    assert!(json.get("historia_clinica").is_some());
    assert!(json.get("nacionalidad").is_some());
    assert!(json.get("pais").is_some());
    assert!(json.get("estado").is_some());
    assert!(json.get("ciudad").is_some());
    assert!(json.get("lugar_nacimiento").is_some());
    assert!(json.get("direccion").is_some());
    assert!(json.get("fecha_nacimiento").is_some());
    assert!(json.get("edad").is_some());
    assert!(json.get("familiar_encargado").is_some());
    assert!(json.get("fecha_ingreso_hospital").is_some());
    assert!(json.get("fecha_ingreso_uci").is_some());
    assert!(json.get("fecha_egreso_uci").is_some());
    assert!(json.get("desenlace_uci").is_some());
    assert!(json.get("descripcion_ingreso").is_some());
    assert!(json.get("antecedentes").is_some());
    assert!(json.get("resumen_ingreso").is_some());
    assert!(json.get("diagnostico_hospital").is_some());
    assert!(json.get("diagnostico_uci").is_some());
    assert!(json.get("examen_fisico_hospital").is_some());
    assert!(json.get("examen_fisico_uci").is_some());
    assert!(json.get("tipo_admision").is_some());
    assert!(json.get("migracion_otro_centro").is_some());
    assert!(json.get("centro_origen").is_some());
    assert!(json.get("ventilacion_mecanica").is_some());
    assert!(json.get("procesos_invasivos").is_some());
    assert!(json.get("cama_id").is_some());
    assert!(json.get("cama_numero").is_some());
    assert!(json.get("estado_gravedad").is_some());
    assert!(json.get("ultimo_apache_score").is_some());
    assert!(json.get("ultimo_gcs_score").is_some());
    assert!(json.get("ultimo_sofa_score").is_some());
    assert!(json.get("ultimo_saps3_score").is_some());
    assert!(json.get("ultimo_news2_score").is_some());
    assert!(json.get("mortality_risk").is_some());
    assert!(json.get("created_at").is_some());
    assert!(json.get("updated_at").is_some());

    // Check types
    assert!(json["patient_id"].is_string());
    assert!(json["tenant_id"].is_string());
    assert!(json["nombre"].is_string());
    assert!(json["apellido"].is_string());
    assert!(json["sexo"].is_string());
    assert!(json["cedula"].is_string());
    assert!(json["color_piel"].is_string());
    assert!(json["historia_clinica"].is_string());
    assert!(json["nacionalidad"].is_string());
    assert!(json["pais"].is_string());
    assert!(json["estado"].is_string());
    assert!(json["ciudad"].is_string());
    assert!(json["lugar_nacimiento"].is_string());
    assert!(json["direccion"].is_string());
    assert!(json["fecha_nacimiento"].is_string());
    assert!(json["edad"].is_number());
    assert!(json["familiar_encargado"].is_string());
    assert!(json["fecha_ingreso_hospital"].is_string());
    assert!(json["fecha_ingreso_uci"].is_string());
    assert!(json["cama_id"].is_null() || json["cama_id"].is_string());
    assert!(json["cama_numero"].is_null() || json["cama_numero"].is_number());
    assert!(json["procesos_invasivos"].is_array());
    assert!(json["diagnostico_hospital"].is_string());
    assert!(json["diagnostico_uci"].is_string());
    assert!(json["estado_gravedad"].is_string());
    assert!(json["created_at"].is_string());
    assert!(json["updated_at"].is_string());
}

/// Test that Measurement model serializes correctly
#[test]
fn test_measurement_golden_json() {
    let apache = ApacheIIData::default();
    let gcs = GcsData::default();
    let measurement = Measurement::new("patient-123", apache, gcs);
    let json = serde_json::to_value(&measurement).expect("serialize measurement");

    assert!(json.get("measurement_id").is_some());
    assert!(json.get("patient_id").is_some());
    assert!(json.get("timestamp").is_some());
    assert!(json.get("apache_data").is_some());
    assert!(json.get("gcs_data").is_some());
    assert!(json.get("apache_score").is_some());
    assert!(json.get("gcs_score").is_some());
    assert!(json.get("severity").is_some());
    assert!(json.get("mortality_risk").is_some());
    assert!(json.get("saps3_score").is_some());
    assert!(json.get("saps3_mortality").is_some());
    assert!(json.get("news2_score").is_some());
    assert!(json.get("news2_level").is_some());
    assert!(json.get("sofa_score").is_some());
    assert!(json.get("sofa_mortality").is_some());
    assert!(json.get("algorithm_version").is_some());
    assert!(json.get("fingerprint").is_some());
    assert!(json.get("notas").is_some());
    assert!(json.get("tenant_id").is_some());

    assert!(json["measurement_id"].is_string());
    assert!(json["patient_id"].is_string());
    assert!(json["timestamp"].is_string());
    assert!(json["apache_score"].is_number());
    assert!(json["gcs_score"].is_number());
    assert!(json["severity"].is_string());
    assert!(json["mortality_risk"].is_number());
    assert!(json["algorithm_version"].is_string());
    assert!(json["fingerprint"].is_string());
    assert!(json["tenant_id"].is_string());
}

/// Test that ApiResponse envelope is consistent
#[test]
fn test_api_response_golden_json() {
    let response = ApiResponse::ok("test data");
    let json = serde_json::to_value(&response).expect("serialize ApiResponse");

    assert!(json.get("success").is_some());
    assert!(json.get("data").is_some());
    assert!(json.get("error").is_some());
    // timestamp may not be present in all versions

    assert_eq!(json["success"], true);
    assert_eq!(json["data"], "test data");
    assert_eq!(json["error"], serde_json::Value::Null);

    // Error response
    let error_response = ApiResponse::<String>::err("error message");
    let json = serde_json::to_value(&error_response).expect("serialize error response");

    assert_eq!(json["success"], false);
    assert_eq!(json["data"], serde_json::Value::Null);
    assert_eq!(json["error"], "error message");
}

/// Test ForecastRequest/Response golden JSON
#[test]
fn test_forecast_golden_json() {
    let req = ForecastRequest {
        series_id: "vitals:MAP:patient-123".to_string(),
        values: vec![80.0, 81.0, 79.0, 82.0, 80.0],
        horizon: 6,
        quantiles: vec![0.1, 0.5, 0.9],
        covariates: None,
    };
    let json = serde_json::to_value(&req).expect("serialize forecast request");

    assert_eq!(json["series_id"], "vitals:MAP:patient-123");
    assert!(json["values"].is_array());
    assert_eq!(json["values"].as_array().unwrap().len(), 5);
    assert_eq!(json["horizon"], 6);
    assert!(json["quantiles"].is_array());
    assert_eq!(json["quantiles"].as_array().unwrap().len(), 3);
    assert!(json["covariates"].is_null());

    // Response
    let resp = ForecastResponse {
        series_id: "vitals:MAP:patient-123".to_string(),
        horizon: 6,
        points: vec![
            ForecastPoint {
                step: 1,
                point: 80.0,
                quantiles: vec![78.0, 80.0, 82.0],
            },
            ForecastPoint {
                step: 2,
                point: 80.5,
                quantiles: vec![78.5, 80.5, 82.5],
            },
        ],
        model: "naive".to_string(),
        model_version: "v1.0.0".to_string(),
        latency_ms: 1.5,
    };
    let json = serde_json::to_value(&resp).expect("serialize forecast response");

    assert_eq!(json["series_id"], "vitals:MAP:patient-123");
    assert_eq!(json["horizon"], 6);
    assert!(json["points"].is_array());
    assert_eq!(json["points"].as_array().unwrap().len(), 2);
    assert_eq!(json["model"], "naive");
    assert_eq!(json["model_version"], "v1.0.0");
    assert!(json["latency_ms"].is_number());
}

/// Test FeatureFlag golden JSON
#[test]
fn test_feature_flag_golden_json() {
    let flag = FeatureFlag {
        key: "nueva_ui".to_string(),
        description: "Nueva interfaz de usuario".to_string(),
        enabled: true,
        tenant_id: Some("hospital-a".to_string()),
        created_at: "2026-01-01T00:00:00Z".to_string(),
        updated_at: "2026-01-01T00:00:00Z".to_string(),
    };
    let json = serde_json::to_value(&flag).expect("serialize feature flag");

    assert_eq!(json["key"], "nueva_ui");
    assert_eq!(json["description"], "Nueva interfaz de usuario");
    assert_eq!(json["enabled"], true);
    assert_eq!(json["tenant_id"], "hospital-a");
    assert_eq!(json["created_at"], "2026-01-01T00:00:00Z");
    assert_eq!(json["updated_at"], "2026-01-01T00:00:00Z");

    // Test without tenant_id (global flag)
    let global_flag = FeatureFlag {
        tenant_id: None,
        ..flag
    };
    let json = serde_json::to_value(&global_flag).expect("serialize global flag");
    assert!(json["tenant_id"].is_null());
}

/// Test that deserialization works with extra fields ignored (forward compatibility)
#[test]
fn test_forward_compatibility() {
    // Add extra field that doesn't exist in struct
    let json = json!({
        "patient_id": "test-123",
        "tenant_id": "default",
        "nombre": "Juan",
        "apellido": "Perez",
        "sexo": "Masculino",
        "cedula": "12345678",
        "color_piel": "Tipo3",
        "historia_clinica": "HC-001",
        "nacionalidad": "Venezolano",
        "pais": "Venezuela",
        "estado": "Miranda",
        "ciudad": "Caracas",
        "lugar_nacimiento": "",
        "direccion": "",
        "fecha_nacimiento": "1990-01-01",
        "edad": 0,
        "familiar_encargado": "",
        "fecha_ingreso_hospital": "2026-01-01T00:00:00Z",
        "fecha_ingreso_uci": "2026-01-01T01:00:00Z",
        "fecha_egreso_uci": "",
        "desenlace_uci": "",
        "descripcion_ingreso": "",
        "antecedentes": "",
        "resumen_ingreso": "",
        "diagnostico_hospital": "",
        "diagnostico_uci": "",
        "examen_fisico_hospital": "",
        "examen_fisico_uci": "",
        "tipo_admision": "Urgente",
        "migracion_otro_centro": false,
        "centro_origen": null,
        "ventilacion_mecanica": false,
        "procesos_invasivos": [],
        "cama_id": null,
        "cama_numero": null,
        "estado_gravedad": "Bajo",
        "ultimo_apache_score": null,
        "ultimo_sofa_score": null,
        "ultimo_gcs_score": null,
        "ultimo_saps3_score": null,
        "ultimo_news2_score": null,
        "mortality_risk": null,
        "created_at": "2026-01-01T00:00:00Z",
        "updated_at": "2026-01-01T00:00:00Z",
        "extra_field_that_does_not_exist": "should_be_ignored"
    });

    // Should deserialize successfully ignoring unknown field
    let patient: Result<Patient, _> = serde_json::from_value(json);
    assert!(
        patient.is_ok(),
        "Failed to deserialize with extra field: {:?}",
        patient.err()
    );
}

/// Test round-trip serialization/deserialization
#[test]
fn test_round_trip_serialization() {
    let original = Patient::new();
    let json = serde_json::to_string(&original).expect("serialize");
    let deserialized: Patient = serde_json::from_str(&json).expect("deserialize");
    let json2 = serde_json::to_string(&deserialized).expect("re-serialize");

    // Should be identical (order of fields may differ but content same)
    let v1: serde_json::Value = serde_json::from_str(&json).expect("parse 1");
    let v2: serde_json::Value = serde_json::from_str(&json2).expect("parse 2");
    assert_eq!(v1, v2);
}

/// Test enum serialization consistency
#[test]
fn test_enum_serialization() {
    let sexo = Sexo::Masculino;
    let json = serde_json::to_value(sexo).expect("serialize sexo");
    assert_eq!(json, "Masculino");

    let gravedad = SeverityLevel::Critico;
    let json = serde_json::to_value(gravedad).expect("serialize gravedad");
    assert_eq!(json, "Critico");

    let cama_tipo = TipoCama::General;
    let json = serde_json::to_value(cama_tipo).expect("serialize tipo_cama");
    assert_eq!(json, "General");
}
