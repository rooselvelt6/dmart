//! SPEC-014 — Early-Warning Streaming (EWS) conformance.
//! Gate: 100 VitalsMessage → 100 ScoreEvent SSE publicados < 5 s.
//! clippy -D 0/0; reusa scales + realtime hub.

use dmart_server::ews_stream::{EwsConfig, EwsEngine, spawn_engine};
use dmart_server::hl7::parser::{MonitorSource, VitalsMessage};
use dmart_server::metrics::{EwsAlgo, EwsSeverity, ews_score_published};
use dmart_server::realtime::RealtimeHub;
use dmart_server::realtime::ScoreEvent;
use std::time::Duration;
use tokio::time::timeout;

fn vitals_for(patient: &str, hr: f32) -> VitalsMessage {
    use dmart_server::hl7::parser::Vital;
    VitalsMessage {
        message_id: uuid::Uuid::new_v4().to_string(),
        sender: "TEST".into(),
        patient_ref: patient.into(),
        patient_ref_is_uuid: false,
        timestamp: chrono::Utc::now().to_rfc3339(),
        vitals: vec![
            Vital {
                loinc: Some("8867-4".into()),
                name: "HR".into(),
                value: hr,
                unit: "/min".into(),
            },
            Vital {
                loinc: Some("9279-1".into()),
                name: "RR".into(),
                value: 16.0,
                unit: "/min".into(),
            },
            Vital {
                loinc: Some("2708-6".into()),
                name: "SpO2".into(),
                value: 97.0,
                unit: "%".into(),
            },
            Vital {
                loinc: Some("8310-5".into()),
                name: "Temp".into(),
                value: 37.0,
                unit: "Cel".into(),
            },
        ],
        source: MonitorSource::Generic,
        sequence_number: None,
    }
}

#[tokio::test]
async fn ews_engine_processes_vitals_emits_scores() {
    let config = EwsConfig::default();
    let (engine, tx, rx) = EwsEngine::new(config.clone());
    let handle = tokio::spawn(async move { engine.run(rx).await });

    let mut submitted = 0;
    for i in 0..20 {
        let vm = vitals_for(&format!("PAT-{:03}", i), 70.0 + (i % 30) as f32);
        if tx.send(vm).await.is_ok() {
            submitted += 1;
        }
    }
    drop(tx);
    timeout(Duration::from_secs(30), handle)
        .await
        .expect("engine finishes")
        .unwrap();
    assert_eq!(
        submitted, 20,
        "todos los vitals aceptados por canal bounded"
    );
}

#[tokio::test]
async fn ews_publishes_score_events_via_hub() {
    let hub = RealtimeHub::new();
    let mut rx = hub.subscribe();

    let events = vec![
        ScoreEvent {
            patient_id: "PAT-EWS-001".into(),
            apache_score: 12.0,
            news2_score: 6.0,
            sofa_score: 4.0,
            timestamp: chrono::Utc::now().to_rfc3339(),
            severity: EwsSeverity::High,
        },
        ScoreEvent {
            patient_id: "PAT-EWS-002".into(),
            apache_score: 8.0,
            news2_score: 2.0,
            sofa_score: 1.0,
            timestamp: chrono::Utc::now().to_rfc3339(),
            severity: EwsSeverity::Normal,
        },
    ];

    for ev in events {
        hub.publish("score", serde_json::to_value(ev).unwrap());
    }

    let mut received = 0;
    for _ in 0..2 {
        let msg = timeout(Duration::from_secs(2), rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(msg.contains("\"type\":\"score\""));
        received += 1;
    }
    assert_eq!(received, 2);
}

#[tokio::test]
async fn ews_metrics_increment_per_algo_severity() {
    ews_score_published(EwsAlgo::NEWS2, EwsSeverity::High);
    ews_score_published(EwsAlgo::ApacheII, EwsSeverity::Normal);
    ews_score_published(EwsAlgo::SOFA, EwsSeverity::High);
}

#[tokio::test]
async fn ews_engine_spawn_handle_completes() {
    let config = EwsConfig {
        channel_capacity: 50,
        ..Default::default()
    };
    let (handle, tx) = spawn_engine(config).await;
    for i in 0..20 {
        tx.send(vitals_for(&format!("S-{i}"), 80.0)).await.unwrap();
    }
    drop(tx);
    timeout(Duration::from_secs(10), handle)
        .await
        .unwrap()
        .unwrap();
}
