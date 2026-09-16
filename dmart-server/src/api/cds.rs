use crate::cds_rules::{CdsEngine, EvaluationContext};
use crate::db::Database;
use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use dmart_shared::models::ApiResponse;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Deserialize)]
pub struct EvaluateRequest {
    pub patient_id: String,
    pub context: Option<serde_json::Value>,
}

pub async fn list_plans(
    State(db): State<Database>,
    _claims: crate::auth::Claims,
) -> impl IntoResponse {
    let engine = CdsEngine::new(Arc::new(db.clone()));
    match engine.load_active_plans().await {
        Ok(plans) => {
            let plans_json: Vec<serde_json::Value> = plans
                .iter()
                .map(|p| {
                    serde_json::json!({
                        "plan_id": p.plan_id,
                        "version": p.version,
                        "active": p.active,
                        "fingerprint": p.fingerprint,
                        "meta": p.meta
                    })
                })
                .collect();
            (
                StatusCode::OK,
                Json(ApiResponse::ok(serde_json::json!({ "plans": plans_json }))),
            )
                .into_response()
        }
        Err(e) => {
            let msg = crate::security::sanitize_internal_error(&e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<serde_json::Value>::err(msg)),
            )
                .into_response()
        }
    }
}

pub async fn evaluate_cds(
    State(db): State<Database>,
    _claims: crate::auth::Claims,
    req: Json<EvaluateRequest>,
) -> impl IntoResponse {
    let engine = CdsEngine::new(Arc::new(db.clone()));
    let _ = engine.load_active_plans().await;

    let ctx = build_context(&req);
    match engine.evaluate(ctx).await {
        Ok(results) => {
            let caret_events = results
                .iter()
                .find(|r| r.triggered)
                .map(|r| r.actions.len());
            let care_plan = if caret_events.unwrap_or(0) > 0 {
                Some(build_care_plan(&req.patient_id, &results))
            } else {
                None
            };

            let results_json: Vec<serde_json::Value> = results
                .iter()
                .map(|r| {
                    serde_json::json!({
                        "plan_id": r.plan_id,
                        "plan_version": r.plan_version,
                        "triggered": r.triggered,
                        "actions": r.actions,
                        "evaluated_at": r.evaluated_at
                    })
                })
                .collect();

            (
                StatusCode::OK,
                Json(ApiResponse::ok(serde_json::json!({
                    "results": results_json,
                    "care_plan": care_plan
                }))),
            )
                .into_response()
        }
        Err(e) => {
            let msg = crate::security::sanitize_internal_error(&e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<serde_json::Value>::err(msg)),
            )
                .into_response()
        }
    }
}

fn build_context(req: &EvaluateRequest) -> EvaluationContext {
    let mut current_scores = HashMap::new();
    let mut current_vitals = None;

    if let Some(context) = &req.context {
        if let Some(scores) = context.get("scores").and_then(|v| v.as_object()) {
            for (k, v) in scores {
                if let Some(n) = v.as_f64() {
                    current_scores.insert(k.clone(), n);
                }
            }
        }
        if let Some(vitals) = context.get("vitals").and_then(|v| v.as_array()) {
            let values: Vec<crate::hl7::parser::Vital> = vitals
                .iter()
                .map(|v| {
                    let loinc = v
                        .get("loinc")
                        .and_then(|x| x.as_str())
                        .map(|s| s.to_string());
                    let name = v
                        .get("name")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string();
                    let value = v.get("value").and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
                    let unit = v
                        .get("unit")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string();
                    crate::hl7::parser::Vital {
                        loinc,
                        name,
                        value,
                        unit,
                    }
                })
                .collect();
            if !values.is_empty() {
                current_vitals = Some(crate::hl7::parser::VitalsMessage {
                    message_id: uuid::Uuid::new_v4().to_string(),
                    sender: "api".to_string(),
                    patient_ref: req.patient_id.clone(),
                    patient_ref_is_uuid: false,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    vitals: values,
                    source: crate::hl7::parser::MonitorSource::Generic,
                    sequence_number: None,
                });
            }
        }
    }

    EvaluationContext {
        patient_id: req.patient_id.clone(),
        patient: None,
        current_vitals,
        current_scores,
        recent_events: vec![],
        timestamp: chrono::Utc::now().timestamp_millis(),
    }
}

fn build_care_plan(
    patient_id: &str,
    results: &[crate::cds_rules::EvaluationResult],
) -> serde_json::Value {
    let activity: Vec<serde_json::Value> = results
        .iter()
        .filter(|r| r.triggered)
        .flat_map(|r| r.actions.iter())
        .map(|a| {
            serde_json::json!({
                "activityId": a.activity_id,
                "kind": a.kind.as_str(),
                "title": a.title,
                "description": a.description,
                "code": a.code
            })
        })
        .collect();

    serde_json::json!({
        "resourceType": "CarePlan",
        "status": "active",
        "intent": "proposal",
        "subject": { "reference": format!("Patient/{}", patient_id) },
        "created": chrono::Utc::now().to_rfc3339(),
        "activity": activity
    })
}
