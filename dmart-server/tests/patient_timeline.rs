//! SPEC-015 — Patient Timeline API conformance.
//! Gate: timeline query with filters, cursor pagination, FHIR bundle output.
//! clippy -D 0/0; reusa patient_timeline module.

use dmart_server::db;
use dmart_server::patient_timeline::{
    EventSeverity, EventType, PatientEvent, TimelineQuery, query_timeline, to_fhir_history_bundle,
};
use std::sync::Arc;
use tempfile::TempDir;

async fn make_test_db() -> (Arc<db::Database>, TempDir) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.db");
    let db = db::connect(path.to_str().unwrap()).await.unwrap();
    dmart_server::migrations::run_migrations(&db).await.unwrap();
    (Arc::new(db), dir)
}

#[tokio::test]
async fn timeline_query_basic() {
    let (db, _tmp) = make_test_db().await;
    let patient_id = "TEST-PAT-001";

    // Insert some events
    let event1 = PatientEvent::new(
        patient_id.to_string(),
        EventType::Admission,
        serde_json::json!({"type": "emergency"}),
        EventSeverity::Info,
        "admission".to_string(),
    );
    event1.store(&db).await.unwrap();

    let event2 = PatientEvent::new(
        patient_id.to_string(),
        EventType::VitalSigns,
        serde_json::json!({"hr": 90}),
        EventSeverity::Info,
        "monitor".to_string(),
    );
    event2.store(&db).await.unwrap();

    let query = TimelineQuery {
        since: None,
        until: None,
        event_type: None,
        severity: None,
        cursor: None,
        limit: Some(10),
    };

    let response = query_timeline(&db, patient_id, query).await.unwrap();
    assert_eq!(response.events.len(), 2);
    assert!(response.next_cursor.is_some());
    assert!(!response.has_more);
}

#[tokio::test]
async fn timeline_query_with_filters() {
    let (db, _tmp) = make_test_db().await;
    let patient_id = "TEST-PAT-002";

    let event1 = PatientEvent::new(
        patient_id.to_string(),
        EventType::Admission,
        serde_json::json!({}),
        EventSeverity::Info,
        "admission".to_string(),
    );
    event1.store(&db).await.unwrap();

    let event2 = PatientEvent::new(
        patient_id.to_string(),
        EventType::Alert,
        serde_json::json!({"msg": "high HR"}),
        EventSeverity::Critical,
        "alerting".to_string(),
    );
    event2.store(&db).await.unwrap();

    // Filter by severity
    let query = TimelineQuery {
        since: None,
        until: None,
        event_type: None,
        severity: Some(dmart_server::patient_timeline::EventSeverity::Critical),
        cursor: None,
        limit: Some(10),
    };

    let response = query_timeline(&db, patient_id, query).await.unwrap();
    assert_eq!(response.events.len(), 1);
    assert_eq!(
        response.events[0].severity,
        dmart_server::patient_timeline::EventSeverity::Critical
    );
}

#[tokio::test]
async fn timeline_cursor_pagination() {
    let (db, _tmp) = make_test_db().await;
    let patient_id = "TEST-PAT-003";

    for i in 0..15 {
        let event = PatientEvent::new(
            patient_id.to_string(),
            EventType::VitalSigns,
            serde_json::json!({"seq": i}),
            EventSeverity::Info,
            "monitor".to_string(),
        );
        event.store(&db).await.unwrap();
    }

    let query1 = TimelineQuery {
        since: None,
        until: None,
        event_type: None,
        severity: None,
        cursor: None,
        limit: Some(5),
    };
    let resp1 = query_timeline(&db, patient_id, query1).await.unwrap();
    assert_eq!(resp1.events.len(), 5);
    assert!(resp1.has_more);
    let cursor = resp1.next_cursor.unwrap();

    let query2 = TimelineQuery {
        since: None,
        until: None,
        event_type: None,
        severity: None,
        cursor: Some(cursor),
        limit: Some(5),
    };
    let resp2 = query_timeline(&db, patient_id, query2).await.unwrap();
    assert_eq!(resp2.events.len(), 5);
}

#[tokio::test]
async fn timeline_many_events_filter_and_cursor() {
    let (db, _tmp) = make_test_db().await;
    let patient_id = "TEST-PAT-004";

    for i in 0..12 {
        let event = PatientEvent::new(
            patient_id.to_string(),
            if i % 3 == 0 {
                EventType::Alert
            } else {
                EventType::VitalSigns
            },
            serde_json::json!({ "seq": i }),
            if i % 3 == 0 {
                EventSeverity::Critical
            } else {
                EventSeverity::Info
            },
            "monitor".to_string(),
        );
        event.store(&db).await.unwrap();
    }

    // Todos los eventos: 12
    let all = query_timeline(
        &db,
        patient_id,
        TimelineQuery {
            since: None,
            until: None,
            event_type: None,
            severity: None,
            cursor: None,
            limit: Some(50),
        },
    )
    .await
    .unwrap();
    assert_eq!(all.events.len(), 12);

    // Filtro por severidad Critical + tipo Alert: 4
    let critical = query_timeline(
        &db,
        patient_id,
        TimelineQuery {
            since: None,
            until: None,
            event_type: Some(EventType::Alert),
            severity: Some(EventSeverity::Critical),
            cursor: None,
            limit: Some(50),
        },
    )
    .await
    .unwrap();
    assert_eq!(critical.events.len(), 4);
    assert!(
        critical
            .events
            .iter()
            .all(|e| e.severity == EventSeverity::Critical)
    );

    // Paginación: página 1 de 5, luego recorrer hasta agotar (12)
    let mut seen = 0usize;
    let mut cursor = None;
    loop {
        let page = query_timeline(
            &db,
            patient_id,
            TimelineQuery {
                since: None,
                until: None,
                event_type: None,
                severity: None,
                cursor,
                limit: Some(5),
            },
        )
        .await
        .unwrap();
        let n = page.events.len();
        assert!(n <= 5);
        seen += n;
        cursor = page.next_cursor.clone();
        if !page.has_more {
            break;
        }
        assert!(cursor.is_some(), "cursor debe avanzar con has_more");
    }
    assert_eq!(seen, 12, "la paginación debe recorrer los 12 eventos");
}

#[tokio::test]
async fn timeline_fhir_history_bundle() {
    let (db, _tmp) = make_test_db().await;
    let patient_id = "TEST-PAT-005";

    let event1 = PatientEvent::new(
        patient_id.to_string(),
        EventType::Admission,
        serde_json::json!({"type": "emergency"}),
        EventSeverity::Info,
        "admission".to_string(),
    );
    event1.store(&db).await.unwrap();

    let event2 = PatientEvent::new(
        patient_id.to_string(),
        EventType::ScoreCalculated,
        serde_json::json!({"scale": "news2", "score": 7}),
        EventSeverity::Critical,
        "scales".to_string(),
    );
    event2.store(&db).await.unwrap();

    let response = query_timeline(
        &db,
        patient_id,
        TimelineQuery {
            since: None,
            until: None,
            event_type: None,
            severity: None,
            cursor: None,
            limit: Some(10),
        },
    )
    .await
    .unwrap();

    let bundle = to_fhir_history_bundle(&response.events);
    assert_eq!(bundle["resourceType"], "Bundle");
    assert_eq!(bundle["type"], "history");
    assert_eq!(bundle["total"], 2);

    let entries = bundle["entry"].as_array().unwrap();
    for entry in entries {
        let resource = &entry["resource"];
        assert!(
            resource["subject"]["reference"]
                .as_str()
                .unwrap()
                .contains(patient_id)
        );
        assert!(
            !resource["meta"]["extension"][0]["valueString"]
                .as_str()
                .unwrap()
                .is_empty(),
            "fingerprint SPEC-029 presente"
        );
        assert!(!resource["resourceType"].as_str().unwrap().is_empty());
    }

    // Admission -> Encounter, ScoreCalculated -> Observation
    let types: Vec<&str> = entries
        .iter()
        .map(|e| e["resource"]["resourceType"].as_str().unwrap())
        .collect();
    assert!(types.contains(&"Encounter"));
    assert!(types.contains(&"Observation"));
}
