//! SPEC-018 — Data Quality conformance.
//! Gate: validadores de calidad, persistencia en `quality_events`, reporte y
//! agregación; endpoints HTTP validate/report/summary.

use axum::Router;
use axum::body::Body;
use axum::http::{Method, StatusCode, header};
use axum::routing::{get, post};
use dmart_server::api::quality;
use dmart_server::data_quality::{
    ReportQuery, quality_report, quality_summary, validate_and_store, validate_vitals_message,
};
use dmart_server::db;
use dmart_server::hl7::parser::VitalsMessage;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::sync::Arc;
use tempfile::TempDir;
use tower::ServiceExt;

async fn make_test_db() -> (Arc<db::Database>, TempDir) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.db");
    let db = db::connect(path.to_str().unwrap()).await.unwrap();
    (Arc::new(db), dir)
}

fn msg_json(patient_ref: &str, timestamp: &str, sender: &str, seq: Option<u32>) -> Value {
    json!({
        "message_id": "MSG-DQ",
        "sender": sender,
        "patient_ref": patient_ref,
        "patient_ref_is_uuid": false,
        "timestamp": timestamp,
        "vitals": [
            {"loinc": "8867-4", "name": "Heart rate", "value": 90.0, "unit": "bpm"},
            {"loinc": "2708-6", "name": "Oxygen saturation", "value": 97.0, "unit": "%"},
            {"loinc": "8310-5", "name": "Temperature", "value": 36.8, "unit": "Cel"},
            {"loinc": "9279-1", "name": "Respiratory rate", "value": 18.0, "unit": "/min"},
            {"loinc": "8480-6", "name": "Systolic BP", "value": 120.0, "unit": "mmHg"}
        ],
        "source": "Mindray",
        "sequence_number": seq
    })
}

fn now_ts() -> String {
    chrono::Utc::now().to_rfc3339()
}

// ─── Validadores de calidad ────────────────────────────────────────────

#[tokio::test]
async fn valid_message_produces_no_issues() {
    let (db, _tmp) = make_test_db().await;
    let msg: VitalsMessage =
        serde_json::from_value(msg_json("PAT-001", &now_ts(), "MON-VALID", Some(1))).unwrap();
    let issues = validate_and_store(&db, &msg).await.unwrap();
    assert!(issues.is_empty(), "mensaje válido sin issues");
    let summary = quality_summary(&db).await.unwrap();
    assert_eq!(summary.total, 0);
}

#[tokio::test]
async fn out_of_range_vital_yields_high_and_persists() {
    let (db, _tmp) = make_test_db().await;
    let mut body = msg_json("PAT-002", &now_ts(), "MON-RANGE", Some(1));
    body["vitals"][0]["value"] = json!(15.0); // HR por debajo de 20 bpm
    let msg: VitalsMessage = serde_json::from_value(body).unwrap();
    let issues = validate_and_store(&db, &msg).await.unwrap();
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].code, "VITAL_OUT_OF_RANGE");
    assert_eq!(issues[0].severity, "high");

    let report = quality_report(&db, ReportQuery::default()).await.unwrap();
    assert_eq!(report.len(), 1);
    assert_eq!(report[0].patient_ref, "PAT-002");
    assert_eq!(report[0].message_id, "MSG-DQ");
}

#[tokio::test]
async fn non_finite_vital_value_yields_high() {
    let (db, _tmp) = make_test_db().await;
    let mut msg: VitalsMessage =
        serde_json::from_value(msg_json("PAT-003", &now_ts(), "MON-FINITE", Some(1))).unwrap();
    msg.vitals[0].value = f32::NAN;
    let issues = validate_and_store(&db, &msg).await.unwrap();
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].code, "VITAL_NON_FINITE");
    assert_eq!(issues[0].severity, "high");
}

#[tokio::test]
async fn future_timestamp_yields_medium() {
    let (db, _tmp) = make_test_db().await;
    let future = (chrono::Utc::now() + chrono::Duration::minutes(5)).to_rfc3339();
    let msg: VitalsMessage =
        serde_json::from_value(msg_json("PAT-004", &future, "MON-FUTURE", Some(1))).unwrap();
    let issues = validate_and_store(&db, &msg).await.unwrap();
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].code, "TIMESTAMP_FUTURE");
    assert_eq!(issues[0].severity, "medium");
}

#[tokio::test]
async fn stale_timestamp_yields_medium() {
    let (db, _tmp) = make_test_db().await;
    let stale = (chrono::Utc::now() - chrono::Duration::minutes(11)).to_rfc3339();
    let msg: VitalsMessage =
        serde_json::from_value(msg_json("PAT-005", &stale, "MON-STALE", Some(1))).unwrap();
    let issues = validate_and_store(&db, &msg).await.unwrap();
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].code, "TIMESTAMP_STALE");
    assert_eq!(issues[0].severity, "medium");
}

#[tokio::test]
async fn empty_patient_ref_yields_high() {
    let (db, _tmp) = make_test_db().await;
    let msg: VitalsMessage =
        serde_json::from_value(msg_json("", &now_ts(), "MON-REF", Some(1))).unwrap();
    assert!(msg.patient_ref.is_empty());
    let issues = validate_and_store(&db, &msg).await.unwrap();
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].code, "PATIENT_REF_EMPTY");
    assert_eq!(issues[0].severity, "high");
}

#[tokio::test]
async fn sequence_duplicate_or_decreasing_yields_medium() {
    dmart_server::data_quality::reset_sequence_tracker();
    let (_db, _tmp) = make_test_db().await;
    let ts = now_ts();

    let first: VitalsMessage =
        serde_json::from_value(msg_json("PAT-SEQ", &ts, "MON-SEQ-1", Some(10))).unwrap();
    assert!(
        validate_vitals_message(&first).is_empty(),
        "primer seq del sender no genera issue"
    );

    let increasing: VitalsMessage =
        serde_json::from_value(msg_json("PAT-SEQ", &ts, "MON-SEQ-1", Some(12))).unwrap();
    assert!(
        validate_vitals_message(&increasing).is_empty(),
        "secuencia creciente no genera issue"
    );

    let duplicate: VitalsMessage =
        serde_json::from_value(msg_json("PAT-SEQ", &ts, "MON-SEQ-1", Some(12))).unwrap();
    let issues = validate_vitals_message(&duplicate);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].code, "SEQUENCE_OUT_OF_ORDER");
    assert_eq!(issues[0].severity, "medium");

    let decreasing: VitalsMessage =
        serde_json::from_value(msg_json("PAT-SEQ", &ts, "MON-SEQ-1", Some(9))).unwrap();
    let issues = validate_vitals_message(&decreasing);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].code, "SEQUENCE_OUT_OF_ORDER");
    assert_eq!(issues[0].severity, "medium");
}

// ─── Reporte y agregación sobre `quality_events` ──────────────────────

#[tokio::test]
async fn report_orders_latest_first_and_respects_limit() {
    let (db, _tmp) = make_test_db().await;
    let ts = now_ts();

    let mut bad = msg_json("PAT-R1", &ts, "MON-RPT", Some(1));
    bad["vitals"][1]["value"] = json!(150.0); // SpO2 fuera de rango → high
    let m1: VitalsMessage = serde_json::from_value(bad).unwrap();
    validate_and_store(&db, &m1).await.unwrap();

    let m2: VitalsMessage = serde_json::from_value(msg_json("", &ts, "MON-RPT", Some(2))).unwrap();
    validate_and_store(&db, &m2).await.unwrap();

    let all = quality_report(&db, ReportQuery::default()).await.unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].code, "PATIENT_REF_EMPTY", "el más reciente primero");

    let limited = quality_report(
        &db,
        ReportQuery {
            limit: Some(1),
            offset: Some(0),
            severity: None,
            code: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(limited.len(), 1);
    assert_eq!(limited[0].code, "PATIENT_REF_EMPTY");

    let all_ok = quality_report(
        &db,
        ReportQuery {
            limit: Some(5),
            offset: Some(0),
            severity: None,
            code: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(all_ok.len(), 2);
}

#[tokio::test]
async fn report_filters_by_severity_and_code() {
    let (db, _tmp) = make_test_db().await;

    let mut bad = msg_json("PAT-F1", &now_ts(), "MON-FLT", Some(1));
    bad["vitals"][1]["value"] = json!(150.0); // high VITAL_OUT_OF_RANGE
    let m1: VitalsMessage = serde_json::from_value(bad).unwrap();
    validate_and_store(&db, &m1).await.unwrap();

    let m2: VitalsMessage = serde_json::from_value(msg_json(
        "PAT-F2",
        &(chrono::Utc::now() - chrono::Duration::minutes(11)).to_rfc3339(),
        "MON-FLT",
        Some(2),
    ))
    .unwrap();
    validate_and_store(&db, &m2).await.unwrap();

    let high = quality_report(
        &db,
        ReportQuery {
            limit: None,
            offset: None,
            severity: Some("high".to_string()),
            code: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(high.len(), 1);
    assert_eq!(high[0].code, "VITAL_OUT_OF_RANGE");

    let out_of_range = quality_report(
        &db,
        ReportQuery {
            limit: None,
            offset: None,
            severity: None,
            code: Some("VITAL_OUT_OF_RANGE".to_string()),
        },
    )
    .await
    .unwrap();
    assert_eq!(out_of_range.len(), 1);
    assert_eq!(out_of_range[0].severity, "high");

    let medium = quality_report(
        &db,
        ReportQuery {
            limit: None,
            offset: None,
            severity: Some("medium".to_string()),
            code: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(medium.len(), 1);
    assert_eq!(medium[0].code, "TIMESTAMP_STALE");
}

#[tokio::test]
async fn summary_groups_by_severity_and_code() {
    let (db, _tmp) = make_test_db().await;

    let mut b1 = msg_json("PAT-S1", &now_ts(), "MON-SUM", Some(1));
    b1["vitals"][0]["value"] = json!(15.0); // high VITAL_OUT_OF_RANGE
    let m1: VitalsMessage = serde_json::from_value(b1).unwrap();
    validate_and_store(&db, &m1).await.unwrap();

    let mut b2 = msg_json("PAT-S2", &now_ts(), "MON-SUM", Some(2));
    b2["vitals"][0]["value"] = json!(400.0); // high VITAL_OUT_OF_RANGE
    let m2: VitalsMessage = serde_json::from_value(b2).unwrap();
    validate_and_store(&db, &m2).await.unwrap();

    let m3: VitalsMessage = serde_json::from_value(msg_json(
        "PAT-S3",
        &(chrono::Utc::now() - chrono::Duration::minutes(11)).to_rfc3339(),
        "MON-SUM",
        Some(3),
    ))
    .unwrap();
    validate_and_store(&db, &m3).await.unwrap();

    let summary = quality_summary(&db).await.unwrap();
    assert_eq!(summary.total, 3);

    let high = summary
        .by_severity
        .iter()
        .find(|s| s.severity == "high")
        .map(|s| s.count)
        .unwrap_or(0);
    let medium = summary
        .by_severity
        .iter()
        .find(|s| s.severity == "medium")
        .map(|s| s.count)
        .unwrap_or(0);
    assert_eq!(high, 2);
    assert_eq!(medium, 1);
    assert_eq!(
        summary.by_severity[0].severity, "high",
        "orden por count desc"
    );

    let oor = summary
        .by_code
        .iter()
        .find(|c| c.code == "VITAL_OUT_OF_RANGE")
        .map(|c| c.count)
        .unwrap_or(0);
    let stale = summary
        .by_code
        .iter()
        .find(|c| c.code == "TIMESTAMP_STALE")
        .map(|c| c.count)
        .unwrap_or(0);
    assert_eq!(oor, 2);
    assert_eq!(stale, 1);
}

// ─── Endpoints HTTP ────────────────────────────────────────────────────

fn http_app(db: &db::Database) -> Router {
    Router::new()
        .route("/data-quality/validate", post(quality::validate_message))
        .route("/data-quality/report", get(quality::quality_report))
        .route("/data-quality/summary", get(quality::quality_summary))
        .with_state(db.clone())
}

async fn send_request(
    app: &Router,
    method: Method,
    path: &str,
    body: Value,
) -> (StatusCode, Value) {
    let request = axum::http::Request::builder()
        .method(method)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

#[tokio::test]
async fn http_validate_parses_and_persists() {
    let (db, _tmp) = make_test_db().await;
    let app = http_app(&db);
    let mut body = msg_json("PAT-HTTP", &now_ts(), "MON-HTTP", Some(1));
    body["vitals"][0]["value"] = json!(15.0);

    let (status, json) = send_request(&app, Method::POST, "/data-quality/validate", body).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["success"], true);
    let issues = json["data"].as_array().unwrap();
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0]["code"], "VITAL_OUT_OF_RANGE");
    assert_eq!(issues[0]["severity"], "high");
}

#[tokio::test]
async fn http_validate_rejects_unparseable_body() {
    let (db, _tmp) = make_test_db().await;
    let app = http_app(&db);
    let (status, json) = send_request(
        &app,
        Method::POST,
        "/data-quality/validate",
        json!({"garbage": 1}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["success"], false);
}

#[tokio::test]
async fn http_report_and_summary_endpoints() {
    let (db, _tmp) = make_test_db().await;
    let app = http_app(&db);

    let mut bad = msg_json("PAT-H2", &now_ts(), "MON-H2", Some(1));
    bad["vitals"][2]["value"] = json!(50.0); // Temp fuera de rango
    let msg: VitalsMessage = serde_json::from_value(bad).unwrap();
    validate_and_store(&db, &msg).await.unwrap();

    let (status, json) = send_request(&app, Method::GET, "/data-quality/report", Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"].as_array().unwrap().len(), 1);
    assert_eq!(json["data"][0]["patient_ref"], "PAT-H2");

    let (status, json) =
        send_request(&app, Method::GET, "/data-quality/summary", Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["total"].as_u64(), Some(1));
    assert_eq!(json["data"]["by_severity"][0]["severity"], "high");
}
