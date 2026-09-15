use crate::db::Database;
use crate::patient_timeline::{EventType, PatientEvent};
use crate::realtime::{ScoreEvent, publish_event};
use cel::{Context, Program};
use chrono::Utc;
use dmart_shared::models::Patient;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanDefinition {
    pub id: surrealdb::RecordId,
    pub plan_id: String,
    pub version: String,
    pub definition: String,
    pub active: bool,
    pub fingerprint: String,
    pub meta: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityDefinition {
    pub id: String,
    pub kind: ActionKind,
    pub title: String,
    pub description: String,
    pub code: Option<serde_json::Value>,
    pub timing: Option<serde_json::Value>,
    #[serde(default)]
    pub participant: Vec<Participant>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActionKind {
    Alert,
    Order,
    Notification,
    Protocol,
    Referral,
}

impl ActionKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ActionKind::Alert => "alert",
            ActionKind::Order => "order",
            ActionKind::Notification => "notification",
            ActionKind::Protocol => "protocol",
            ActionKind::Referral => "referral",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Participant {
    pub role: String,
    pub actor_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationContext {
    pub patient_id: String,
    pub patient: Option<Patient>,
    pub current_vitals: Option<crate::hl7::parser::VitalsMessage>,
    pub current_scores: HashMap<String, f64>,
    pub recent_events: Vec<PatientEvent>,
    pub timestamp: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EvaluationResult {
    pub plan_id: String,
    pub plan_version: String,
    pub triggered: bool,
    pub actions: Vec<ActionResult>,
    pub evaluated_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ActionResult {
    pub activity_id: String,
    pub kind: ActionKind,
    pub title: String,
    pub description: String,
    pub code: Option<serde_json::Value>,
    pub patient_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CarePlan {
    pub id: String,
    pub patient_id: String,
    pub status: String,
    pub intent: String,
    pub activity: Vec<ActivityDefinition>,
    pub created_at: i64,
}

pub struct CdsEngine {
    db: Arc<Database>,
    plan_cache: Arc<tokio::sync::RwLock<HashMap<String, PlanDefinition>>>,
}

impl CdsEngine {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            db,
            plan_cache: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }

    pub async fn load_active_plans(&self) -> Result<Vec<PlanDefinition>, Box<surrealdb::Error>> {
        let sql = "SELECT * FROM plan_definition WHERE active = true";
        let mut result = self.db.query(sql).await?;
        let plans: Vec<PlanDefinition> = result.take(0).unwrap_or_default();

        let mut cache = self.plan_cache.write().await;
        cache.clear();
        for plan in &plans {
            cache.insert(plan.plan_id.clone(), plan.clone());
        }
        Ok(plans.clone())
    }

    pub async fn get_plan(&self, plan_id: &str) -> Option<PlanDefinition> {
        let cache = self.plan_cache.read().await;
        cache.get(plan_id).cloned()
    }

    pub async fn evaluate(&self, ctx: EvaluationContext) -> Result<Vec<EvaluationResult>, String> {
        let plans = {
            let cache = self.plan_cache.read().await;
            cache.values().cloned().collect::<Vec<_>>()
        };

        let mut results = Vec::new();
        for plan in plans {
            let result = self.evaluate_plan(&plan, &ctx).await?;
            results.push(result);
        }
        Ok(results)
    }

    async fn evaluate_plan(
        &self,
        plan: &PlanDefinition,
        ctx: &EvaluationContext,
    ) -> Result<EvaluationResult, String> {
        let def: serde_json::Value =
            serde_json::from_str(&plan.definition).map_err(|e| e.to_string())?;
        let criteria = def.get("criteria").and_then(|v| v.as_array());
        let actions = def.get("action").and_then(|v| v.as_array());

        let mut triggered = false;
        let mut action_results = Vec::new();

        if let Some(criteria) = criteria {
            for criterion in criteria {
                if let Some(expression) = criterion.get("expression").and_then(|v| v.as_str()) {
                    let cel_ctx = self.build_cel_context(ctx);
                    let prog = Program::compile(expression).map_err(|e| e.to_string())?;
                    let result = prog.execute(&cel_ctx).map_err(|e| e.to_string())?;

                    if let cel::Value::Bool(true) = result {
                        triggered = true;
                        break;
                    }
                }
            }
        }

        if triggered && let Some(actions) = actions {
            for action in actions {
                let activity = self.parse_activity(action)?;
                let result = self.execute_action(&activity, ctx).await?;
                action_results.push(result);
            }
        }

        Ok(EvaluationResult {
            plan_id: plan.plan_id.clone(),
            plan_version: plan.version.clone(),
            triggered,
            actions: action_results,
            evaluated_at: Utc::now().timestamp_millis(),
        })
    }

    fn build_cel_context(&self, ctx: &EvaluationContext) -> Context<'_> {
        let mut cel_ctx = Context::default();
        cel_ctx.add_variable_from_value("patient_id", ctx.patient_id.clone());

        if let Some(patient) = &ctx.patient {
            cel_ctx.add_variable_from_value("age", patient.edad as i64);
            cel_ctx.add_variable_from_value("gender", format!("{:?}", patient.sexo).to_lowercase());
        }

        if let Some(vitals) = &ctx.current_vitals {
            for v in &vitals.vitals {
                if let Some(loinc) = &v.loinc {
                    let key = format!("vital_{}", loinc.replace(['.', '-'], "_"));
                    cel_ctx.add_variable_from_value(&key, v.value as i64);
                }
            }
        }

        for (scale, score) in &ctx.current_scores {
            let key = format!("score_{}", scale.replace('.', "_"));
            cel_ctx.add_variable_from_value(&key, *score as i64);
        }

        cel_ctx
    }

    fn parse_activity(&self, value: &serde_json::Value) -> Result<ActivityDefinition, String> {
        serde_json::from_value(value.clone()).map_err(|e| e.to_string())
    }

    async fn execute_action(
        &self,
        activity: &ActivityDefinition,
        ctx: &EvaluationContext,
    ) -> Result<ActionResult, String> {
        let action_result = ActionResult {
            activity_id: activity.id.clone(),
            kind: activity.kind.clone(),
            title: activity.title.clone(),
            description: activity.description.clone(),
            code: activity.code.clone(),
            patient_id: ctx.patient_id.clone(),
        };

        match activity.kind {
            ActionKind::Alert => {
                let event = PatientEvent::new(
                    ctx.patient_id.clone(),
                    EventType::Alert,
                    serde_json::json!({
                        "cds_plan": activity.id,
                        "title": activity.title,
                        "description": activity.description
                    }),
                    crate::patient_timeline::EventSeverity::Warning,
                    "cds".to_string(),
                );
                event.store(&self.db).await.map_err(|e| e.to_string())?;
            }
            ActionKind::Order => {
                // Would create order in DB
            }
            ActionKind::Notification => {
                publish_event(ScoreEvent {
                    patient_id: ctx.patient_id.clone(),
                    apache_score: 0.0,
                    news2_score: 0.0,
                    sofa_score: 0.0,
                    timestamp: Utc::now().to_rfc3339(),
                    severity: crate::metrics::EwsSeverity::High,
                });
            }
            ActionKind::Protocol => {
                // Protocol activation
            }
            ActionKind::Referral => {
                // Referral creation
            }
        }

        Ok(action_result)
    }
}
