//! SPEC-019 — Alert Escalation conformance.
//! Gate: políticas configurables con upsert por severidad, ciclo de vida
//! created → acknowledged → escalated, activos ordenados por severidad desc.
//! `cargo test -p dmart-server --test escalation`

use axum::{
    Router,
    body::Body,
    http::{Method, Request, StatusCode},
    routing::{get, post},
};
use dmart_server::api::escalation;
use dmart_server::db::Database;
use dmart_server::escalation::{EscalationStatus, Severity};
use http_body_util::BodyExt;
use tempfile::TempDir;
use tower::ServiceExt;

async fn make_test_db() -> (Database, TempDir) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.db");
    let db = dmart_server::db::connect(path.to_str().unwrap())
        .await
        .unwrap();
    (db, dir)
}

fn escalation_app(db: Database) -> Router {
    Router::new()
        .route(
            "/policies",
            get(escalation::list_policies).post(escalation::set_policy),
        )
        .route("/active", get(escalation::active_escalations))
        .route("/{id}/ack", post(escalation::acknowledge_alert))
        .route("/{id}/escalate", post(escalation::escalate_alert))
        .with_state(db)
}

async fn send(
    app: &Router,
    method: Method,
    path: &str,
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    let builder = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    let payload = body.map(|v| v.to_string()).unwrap_or_default();
    let request = builder.body(Body::from(payload)).expect("build request");
    let response = app.clone().oneshot(request).await.expect("oneshot failed");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("collect body")
        .to_bytes();
    let json = if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
    };
    (status, json)
}

/// Extrae el key crudo (uuid) del RecordId — el Display escaparía el string.
fn bare_id(record_id: &surrealdb::RecordId) -> String {
    String::try_from(record_id.key().clone()).expect("escalation key es string")
}

#[tokio::test]
async fn policies_seeded_and_ordered() {
    let (db, _tmp) = make_test_db().await;
    let policies = dmart_server::escalation::list_policies(&db).await.unwrap();
    assert_eq!(policies.len(), 4, "una política sembrada por severidad");

    let severities: Vec<String> = policies
        .iter()
        .map(|p| p.severity.as_str().to_string())
        .collect();
    assert_eq!(severities, vec!["low", "medium", "high", "critical"]);

    let critical = policies.last().unwrap();
    assert!(critical.target_role == "Dr");
    assert!(critical.enabled);
    assert!(critical.max_response_minutes <= 5);
}

#[tokio::test]
async fn create_escalation_derives_policy() {
    let (db, _tmp) = make_test_db().await;
    let esc = dmart_server::escalation::create_escalation(&db, "PAT-POL", "ews", Severity::High)
        .await
        .unwrap();

    assert_eq!(esc.level, 1, "nivel inicial = 1");
    assert_eq!(esc.status, EscalationStatus::Created);
    assert_eq!(esc.policy_severity, "high", "deriva de la política vigente");
    assert_eq!(esc.alert_type, "ews");
    assert_eq!(esc.patient_id, "PAT-POL");
    assert!(esc.acknowledged_at.is_none());
    assert!(esc.escalated_at.is_none());

    // La escalación queda registrada como evento Alert en la timeline (SPEC-015).
    let query = dmart_server::patient_timeline::TimelineQuery {
        since: None,
        until: None,
        event_type: None,
        severity: None,
        cursor: None,
        limit: Some(10),
    };
    let response = dmart_server::patient_timeline::query_timeline(&db, "PAT-POL", query)
        .await
        .unwrap();
    assert_eq!(response.events.len(), 1);
    assert_eq!(
        response.events[0].event_type,
        dmart_server::patient_timeline::EventType::Alert
    );
}

#[tokio::test]
async fn set_policy_upserts_by_severity() {
    let (db, _tmp) = make_test_db().await;
    let app = escalation_app(db);

    let (status, json) = send(
        &app,
        Method::POST,
        "/policies",
        Some(serde_json::json!({
            "severity": "high",
            "max_response_minutes": 10,
            "timeout_minutes": 45,
            "target_role": "Dr",
            "enabled": true
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["severity"], "high");
    assert_eq!(json["data"]["max_response_minutes"], 10);

    // Upsert con body parcial: conserva el resto y actualiza `enabled`.
    let (status, json) = send(
        &app,
        Method::POST,
        "/policies",
        Some(serde_json::json!({ "severity": "high", "enabled": false })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["enabled"], false);
    assert_eq!(
        json["data"]["max_response_minutes"], 10,
        "conserva el upsert previo"
    );

    // Una única política por severidad.
    let (_, json) = send(&app, Method::GET, "/policies", None).await;
    let highs: Vec<&serde_json::Value> = json["data"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["severity"] == "high")
        .collect();
    assert_eq!(highs.len(), 1);

    // Severidad desconocida → 400.
    let (status, _) = send(
        &app,
        Method::POST,
        "/policies",
        Some(serde_json::json!({ "severity": "urgent" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn escalation_lifecycle_via_http() {
    let (db, _tmp) = make_test_db().await;
    let app = escalation_app(db.clone());

    let (status, _) = send(
        &app,
        Method::POST,
        "/policies",
        Some(serde_json::json!({
            "severity": "critical",
            "max_response_minutes": 5,
            "timeout_minutes": 30,
            "target_role": "Dr"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let esc =
        dmart_server::escalation::create_escalation(&db, "PAT-CRIT-1", "ews", Severity::Critical)
            .await
            .unwrap();
    let id = bare_id(&esc.id);

    // Activo tras crear.
    let (status, json) = send(&app, Method::GET, "/active", None).await;
    assert_eq!(status, StatusCode::OK);
    let data = json["data"].as_array().unwrap();
    assert_eq!(data.len(), 1);
    assert_eq!(data[0]["status"], "created");
    assert_eq!(data[0]["policy_severity"], "critical");
    assert_eq!(data[0]["level"], 1);

    // Acuse.
    let (status, json) = send(&app, Method::POST, &format!("/{id}/ack"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "acknowledged");
    assert!(json["data"]["acknowledged_at"].is_string());
    let first_ack = json["data"]["acknowledged_at"]
        .as_str()
        .unwrap()
        .to_string();

    // Acuse idempotente: no re-marca el timestamp.
    let (status, json) = send(&app, Method::POST, &format!("/{id}/ack"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["acknowledged_at"].as_str().unwrap(), first_ack);

    // Primer escalamiento → nivel 2.
    let (status, json) = send(&app, Method::POST, &format!("/{id}/escalate"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "escalated");
    assert_eq!(json["data"]["level"], 2);
    assert!(json["data"]["escalated_at"].is_string());

    // Segundo escalamiento → nivel 3 (sube cada vez).
    let (status, json) = send(&app, Method::POST, &format!("/{id}/escalate"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["level"], 3);
}

#[tokio::test]
async fn active_escalations_sorted_by_severity() {
    let (db, _tmp) = make_test_db().await;
    let app = escalation_app(db.clone());

    let _low = dmart_server::escalation::create_escalation(&db, "PAT-LO", "ews", Severity::Low)
        .await
        .unwrap();
    let _high = dmart_server::escalation::create_escalation(&db, "PAT-HI", "ews", Severity::High)
        .await
        .unwrap();

    let (status, json) = send(&app, Method::GET, "/active", None).await;
    assert_eq!(status, StatusCode::OK);
    let data = json["data"].as_array().unwrap();
    assert_eq!(data.len(), 2);
    assert_eq!(data[0]["severity"], "high", "severidad desc primero");
    assert_eq!(data[1]["severity"], "low");
}

#[tokio::test]
async fn resolved_excluded_from_active_and_closed() {
    let (db, _tmp) = make_test_db().await;
    let app = escalation_app(db.clone());

    let esc = dmart_server::escalation::create_escalation(&db, "PAT-RES", "cds", Severity::High)
        .await
        .unwrap();
    let id = bare_id(&esc.id);

    let resolved = dmart_server::escalation::resolve_escalation(&db, &id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(resolved.status, EscalationStatus::Resolved);
    assert!(resolved.resolved_at.is_some());

    let (_, json) = send(&app, Method::GET, "/active", None).await;
    assert_eq!(
        json["data"].as_array().unwrap().len(),
        0,
        "resuelta fuera de activas"
    );

    let (status, _) = send(&app, Method::POST, &format!("/{id}/ack"), None).await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, _) = send(&app, Method::POST, &format!("/{id}/escalate"), None).await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn unknown_escalation_returns_404() {
    let (db, _tmp) = make_test_db().await;
    let app = escalation_app(db);

    let (status, _) = send(&app, Method::POST, "/does-not-exist/ack", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = send(&app, Method::POST, "/does-not-exist/escalate", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
