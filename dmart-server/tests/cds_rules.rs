//! SPEC-016 — Clinical Decision Support Rules Engine conformance.
//! Gate: 3 seeded plans (Sepsis, ARDS, Anticoag) evaluate correctly.
//! clippy -D 0/0; reusa cds_rules module + patient_timeline.

use dmart_server::cds_rules::EvaluationContext;
use dmart_server::db;
use std::sync::Arc;
use tempfile::TempDir;

async fn make_test_db() -> (Arc<db::Database>, TempDir) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.db");
    let db = db::connect(path.to_str().unwrap()).await.unwrap();
    // Run migrations
    dmart_server::migrations::run_migrations(&*db)
        .await
        .unwrap();
    (Arc::new(db), dir)
}

#[tokio::test]
async fn cds_engine_loads_seeded_plans() {
    let (db, _tmp) = make_test_db().await;
    let engine = dmart_server::cds_rules::CdsEngine::new(db.clone());

    // Seed plans

    let plans = engine.load_active_plans().await.unwrap();
    assert_eq!(plans.len(), 3);

    let plan_ids: Vec<&str> = plans.iter().map(|p| p.plan_id.as_str()).collect();
    assert!(plan_ids.contains(&"sepsis-3-bundle"));
    assert!(plan_ids.contains(&"ardsnet-ventilation"));
    assert!(plan_ids.contains(&"anticoagulation-icu"));
}

#[tokio::test]
async fn cds_evaluate_sepsis_plan_triggers() {
    let (db, _tmp) = make_test_db().await;
    let engine = dmart_server::cds_rules::CdsEngine::new(db.clone());

    engine.load_active_plans().await.unwrap();

    // Create context with high NEWS2 score AND required vitals (sepsis criteria)
    let mut scores = std::collections::HashMap::new();
    scores.insert("news2".to_string(), 7.0); // Above threshold 5
    scores.insert("sofa".to_string(), 1.0); // Below ARDS threshold

    let vitals = dmart_server::hl7::parser::VitalsMessage {
        message_id: uuid::Uuid::new_v4().to_string(),
        sender: "TEST".into(),
        patient_ref: "SEPSIS-TEST-001".into(),
        patient_ref_is_uuid: false,
        timestamp: chrono::Utc::now().to_rfc3339(),
        vitals: vec![dmart_server::hl7::parser::Vital {
            loinc: Some("2708-6".into()),
            name: "SpO2".into(),
            value: 85.0,
            unit: "%".into(),
        }],
        source: dmart_server::hl7::parser::MonitorSource::Generic,
        sequence_number: None,
    };

    let ctx = EvaluationContext {
        patient_id: "SEPSIS-TEST-001".to_string(),
        patient: None,
        current_vitals: Some(vitals),
        current_scores: scores,
        recent_events: vec![],
        timestamp: chrono::Utc::now().timestamp_millis(),
    };

    let results = engine.evaluate(ctx).await.unwrap();

    let sepsis_result = results
        .iter()
        .find(|r| r.plan_id == "sepsis-3-bundle")
        .unwrap();
    assert!(
        sepsis_result.triggered,
        "Sepsis plan should trigger with NEWS2 >= 5"
    );
    assert!(!sepsis_result.actions.is_empty());
}

#[tokio::test]
async fn cds_evaluate_ards_plan_triggers() {
    let (db, _tmp) = make_test_db().await;
    let engine = dmart_server::cds_rules::CdsEngine::new(db.clone());

    engine.load_active_plans().await.unwrap();

    // Create context with low SpO2 and SOFA >= 2 (ARDS criteria) - also provide news2 for sepsis plan
    let mut scores = std::collections::HashMap::new();
    scores.insert("sofa".to_string(), 3.0);
    scores.insert("news2".to_string(), 2.0); // Below sepsis threshold

    let vitals = dmart_server::hl7::parser::VitalsMessage {
        message_id: uuid::Uuid::new_v4().to_string(),
        sender: "TEST".into(),
        patient_ref: "ARDS-TEST-001".into(),
        patient_ref_is_uuid: false,
        timestamp: chrono::Utc::now().to_rfc3339(),
        vitals: vec![dmart_server::hl7::parser::Vital {
            loinc: Some("2708-6".into()),
            name: "SpO2".into(),
            value: 85.0,
            unit: "%".into(),
        }],
        source: dmart_server::hl7::parser::MonitorSource::Generic,
        sequence_number: None,
    };

    let ctx = EvaluationContext {
        patient_id: "ARDS-TEST-001".to_string(),
        patient: None,
        current_vitals: Some(vitals),
        current_scores: scores,
        recent_events: vec![],
        timestamp: chrono::Utc::now().timestamp_millis(),
    };

    let results = engine.evaluate(ctx).await.unwrap();

    let ards_result = results
        .iter()
        .find(|r| r.plan_id == "ardsnet-ventilation")
        .unwrap();
    assert!(
        ards_result.triggered,
        "ARDS plan should trigger with SpO2 < 88 and SOFA >= 2"
    );
    assert!(!ards_result.actions.is_empty());
}

#[tokio::test]
async fn cds_evaluate_anticoag_plan_always_triggers() {
    let (db, _tmp) = make_test_db().await;
    let engine = dmart_server::cds_rules::CdsEngine::new(db.clone());

    engine.load_active_plans().await.unwrap();

    // Anticoagulation plan has criteria "true" - should always trigger
    // But other plans also need their variables, so provide all required
    let mut scores = std::collections::HashMap::new();
    scores.insert("news2".to_string(), 2.0); // Below sepsis threshold
    scores.insert("sofa".to_string(), 1.0); // Below ARDS threshold

    let vitals = dmart_server::hl7::parser::VitalsMessage {
        message_id: uuid::Uuid::new_v4().to_string(),
        sender: "TEST".into(),
        patient_ref: "ANTICOAG-TEST-001".into(),
        patient_ref_is_uuid: false,
        timestamp: chrono::Utc::now().to_rfc3339(),
        vitals: vec![dmart_server::hl7::parser::Vital {
            loinc: Some("2708-6".into()),
            name: "SpO2".into(),
            value: 97.0,
            unit: "%".into(),
        }],
        source: dmart_server::hl7::parser::MonitorSource::Generic,
        sequence_number: None,
    };

    let ctx = EvaluationContext {
        patient_id: "ANTICOAG-TEST-001".to_string(),
        patient: None,
        current_vitals: Some(vitals),
        current_scores: scores,
        recent_events: vec![],
        timestamp: chrono::Utc::now().timestamp_millis(),
    };

    let results = engine.evaluate(ctx).await.unwrap();

    let anticoag_result = results
        .iter()
        .find(|r| r.plan_id == "anticoagulation-icu")
        .unwrap();
    assert!(
        anticoag_result.triggered,
        "Anticoagulation plan should always trigger (criteria=true)"
    );
    assert_eq!(anticoag_result.actions.len(), 2); // Alert + Order
}
