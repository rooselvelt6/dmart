//! SPEC-044: endpoints HTTP de la Consola Técnica de Soporte.
//!
//! Rutas (bajo `/admin/support`, se resuelven en `auth` → permisos
//! `support:read`/`support:act`):
//!   - `GET  /admin/support/systems`        → estado vivo por subsistema
//!   - `GET  /admin/support/diagnostics`    → SLIs + runbook sugerido
//!   - `POST /admin/support/actions/{action}` → ejecuta una corrección
//!   - `GET  /admin/support/history`        → historial de eventos

use std::net::SocketAddr;
use std::time::Instant;

use axum::Json;
use axum::extract::{ConnectInfo, Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

use crate::auth::Claims;
use crate::db::Database;
use crate::support::{self, ACTIONS, ActionRequest, SupportEvent, SupportService};
use dmart_shared::models::ApiResponse;

// ─── Modelos de respuesta ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SupportSystem {
    pub key: String,
    pub name: String,
    /// "ok" | "degraded" | "error"
    pub status: String,
    pub latency_ms: Option<f64>,
    pub error_count: u64,
    pub ok_count: u64,
    pub freshness_secs: Option<f64>,
    pub details: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SliIndicator {
    pub key: String,
    pub label: String,
    pub value: String,
    pub target: String,
    pub ok: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Diagnostic {
    pub system: String,
    pub indicators: Vec<SliIndicator>,
    /// Acciones del runbook sugeridas (keys de `ACTIONS`, o "none").
    pub suggested_actions: Vec<String>,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ActionResponse {
    pub action: String,
    pub success: bool,
    pub message: String,
    pub details: Value,
    pub event: Option<SupportEvent>,
}

fn system_name(key: &str) -> &'static str {
    match key {
        "db" => "Base de datos (SurrealDB)",
        "ingest" => "Ingest MLLP/HL7",
        "realtime" => "Realtime SSE",
        "ml" => "Serving ML",
        "monitores" => "Registro de monitores",
        "audit" => "Auditoría",
        "backup" => "Backup",
        _ => "Desconocido",
    }
}

fn action_subsystem(action: &str) -> &'static str {
    match action {
        "ingest_retry" | "circuit_reset" => "ingest",
        "backup" => "backup",
        "audit_retention" => "audit",
        "model_swap" => "ml",
        "verify_fingerprints" => "audit",
        _ => "support",
    }
}

async fn probe_system(db: &Database, key: &str) -> SupportSystem {
    let errs = support::error_count(key);
    let oks = support::ok_count(key);
    let freshness = support::freshness_secs(key);

    match key {
        "db" => {
            let start = Instant::now();
            let res = db.query("RETURN 1").await;
            let latency = start.elapsed().as_secs_f64() * 1000.0;
            let (status, details) = match res {
                Ok(_) => {
                    crate::metrics::support_system_status(key, 2);
                    support::note("db", true);
                    ("ok".to_string(), format!("latencia {latency:.1} ms"))
                }
                Err(e) => {
                    crate::metrics::support_system_status(key, 0);
                    support::note("db", false);
                    let msg = crate::security::sanitize_internal_error(&e.to_string());
                    ("error".to_string(), format!("query falló: {msg}"))
                }
            };
            SupportSystem {
                key: key.to_string(),
                name: system_name(key).to_string(),
                status,
                latency_ms: Some(latency),
                error_count: errs,
                ok_count: oks,
                freshness_secs: freshness,
                details,
            }
        }
        "ingest" => {
            let fault_devices = match crate::ingest::global_ingest() {
                Some(state) => state.metrics().await.fault_devices,
                None => 0,
            };
            let status = if fault_devices > 0 || errs > 0 {
                crate::metrics::support_system_status(key, 1);
                "degraded"
            } else {
                crate::metrics::support_system_status(key, 2);
                "ok"
            };
            SupportSystem {
                key: key.to_string(),
                name: system_name(key).to_string(),
                status: status.to_string(),
                latency_ms: None,
                error_count: errs,
                ok_count: oks,
                freshness_secs: freshness,
                details: format!("{fault_devices} dispositivo(s) en fault"),
            }
        }
        "realtime" => {
            let active = crate::metrics::sse_active();
            let status = if errs > 0 {
                crate::metrics::support_system_status(key, 1);
                "degraded"
            } else {
                crate::metrics::support_system_status(key, 2);
                "ok"
            };
            SupportSystem {
                key: key.to_string(),
                name: system_name(key).to_string(),
                status: status.to_string(),
                latency_ms: None,
                error_count: errs,
                ok_count: oks,
                freshness_secs: freshness,
                details: format!("{active} suscriptor(es) SSE activos"),
            }
        }
        "ml" => {
            let registry = crate::ml_serving::server().registry.clone();
            let active = registry.active_model();
            let status = if active.is_some() {
                crate::metrics::support_system_status(key, 2);
                "ok"
            } else {
                crate::metrics::support_system_status(key, 1);
                "degraded"
            };
            let details = match &active {
                Some(m) => format!("modelo activo: {}@{}", m.name, m.version),
                None => "sin modelo activo".to_string(),
            };
            SupportSystem {
                key: key.to_string(),
                name: system_name(key).to_string(),
                status: status.to_string(),
                latency_ms: None,
                error_count: errs,
                ok_count: oks,
                freshness_secs: freshness,
                details,
            }
        }
        "monitores" => match crate::device_registry::status_summary(db).await {
            Ok(summary) => {
                let offline = summary
                    .por_estado
                    .iter()
                    .find(|s| s.estado == crate::device_registry::ESTADO_OFFLINE)
                    .map(|s| s.total)
                    .unwrap_or(0);
                let status = if offline > 0 {
                    crate::metrics::support_system_status(key, 1);
                    "degraded"
                } else {
                    crate::metrics::support_system_status(key, 2);
                    "ok"
                };
                SupportSystem {
                    key: key.to_string(),
                    name: system_name(key).to_string(),
                    status: status.to_string(),
                    latency_ms: None,
                    error_count: errs,
                    ok_count: oks,
                    freshness_secs: freshness,
                    details: format!("{}/{} en línea", summary.total - offline, summary.total),
                }
            }
            Err(e) => {
                crate::metrics::support_system_status(key, 0);
                let msg = crate::security::sanitize_internal_error(&e.to_string());
                SupportSystem {
                    key: key.to_string(),
                    name: system_name(key).to_string(),
                    status: "error".to_string(),
                    latency_ms: None,
                    error_count: errs,
                    ok_count: oks,
                    freshness_secs: freshness,
                    details: msg,
                }
            }
        },
        "audit" => {
            let status = if errs > 0 {
                crate::metrics::support_system_status(key, 1);
                "degraded"
            } else {
                crate::metrics::support_system_status(key, 2);
                "ok"
            };
            SupportSystem {
                key: key.to_string(),
                name: system_name(key).to_string(),
                status: status.to_string(),
                latency_ms: None,
                error_count: errs,
                ok_count: oks,
                freshness_secs: freshness,
                details: format!("retención >{} años", support::AUDIT_RETENTION_YEARS),
            }
        }
        "backup" => {
            let status = if errs > 0 {
                crate::metrics::support_system_status(key, 1);
                "degraded"
            } else {
                crate::metrics::support_system_status(key, 2);
                "ok"
            };
            SupportSystem {
                key: key.to_string(),
                name: system_name(key).to_string(),
                status: status.to_string(),
                latency_ms: None,
                error_count: errs,
                ok_count: oks,
                freshness_secs: freshness,
                details: "último backup: ver historial".to_string(),
            }
        }
        _ => SupportSystem {
            key: key.to_string(),
            name: system_name(key).to_string(),
            status: "error".to_string(),
            latency_ms: None,
            error_count: errs,
            ok_count: oks,
            freshness_secs: freshness,
            details: "subsistema desconocido".to_string(),
        },
    }
}

/// GET /admin/support/systems — estado vivo de todos los subsistemas.
pub async fn get_systems(State(db): State<Database>, _claims: Claims) -> Response {
    let mut systems = Vec::with_capacity(support::SUBSYSTEMS.len());
    for key in support::SUBSYSTEMS {
        systems.push(probe_system(&db, key).await);
    }
    (StatusCode::OK, Json(ApiResponse::ok(systems))).into_response()
}

async fn build_diagnostics(db: &Database) -> Vec<Diagnostic> {
    let systems = {
        let mut v = Vec::new();
        for key in support::SUBSYSTEMS {
            v.push(probe_system(db, key).await);
        }
        v
    };

    let mut out = Vec::new();

    for s in &systems {
        let mut indicators = Vec::new();
        let mut suggested = Vec::new();

        match s.key.as_str() {
            "db" => {
                indicators.push(SliIndicator {
                    key: "db_latency".into(),
                    label: "Latencia de query".into(),
                    value: format!("{:.1} ms", s.latency_ms.unwrap_or(0.0)),
                    target: "< 1000 ms".into(),
                    ok: s.status == "ok",
                });
                if s.status != "ok" {
                    suggested.push("none".into());
                }
            }
            "ingest" => {
                let metrics = match crate::ingest::global_ingest() {
                    Some(state) => state.metrics().await,
                    None => crate::ingest::IngestMetrics::default(),
                };
                indicators.push(SliIndicator {
                    key: "gaps".into(),
                    label: "Lagunas de datos (gaps)".into(),
                    value: metrics.gaps_total.to_string(),
                    target: "0".into(),
                    ok: metrics.gaps_total == 0,
                });
                indicators.push(SliIndicator {
                    key: "fault_devices".into(),
                    label: "Dispositivos en fault".into(),
                    value: metrics.fault_devices.to_string(),
                    target: "0".into(),
                    ok: metrics.fault_devices == 0,
                });
                indicators.push(SliIndicator {
                    key: "error_avg".into(),
                    label: "Tasa media de error".into(),
                    value: format!("{:.2}", metrics.error_avg),
                    target: "< 0.05".into(),
                    ok: metrics.error_avg < 0.05,
                });
                if metrics.gaps_total > 0 {
                    suggested.push("ingest_retry".into());
                }
                if metrics.fault_devices > 0 {
                    suggested.push("circuit_reset".into());
                }
            }
            "realtime" => {
                indicators.push(SliIndicator {
                    key: "sse_active".into(),
                    label: "Suscriptores SSE".into(),
                    value: crate::metrics::sse_active().to_string(),
                    target: ">= 0".into(),
                    ok: true,
                });
                if s.status != "ok" {
                    suggested.push("none".into());
                }
            }
            "ml" => {
                let active = crate::ml_serving::server().registry.clone().active_model();
                let active_desc = active
                    .as_ref()
                    .map(|m| format!("{} @ {}", m.name, m.version))
                    .unwrap_or_else(|| "ninguno".into());
                indicators.push(SliIndicator {
                    key: "active_model".into(),
                    label: "Modelo activo".into(),
                    value: active_desc,
                    target: "un modelo cargado".into(),
                    ok: active.is_some(),
                });
                if active.is_none() {
                    suggested.push("model_swap".into());
                }
            }
            "monitores" => {
                if let Ok(summary) = crate::device_registry::status_summary(db).await {
                    let off = summary
                        .por_estado
                        .iter()
                        .find(|st| st.estado == crate::device_registry::ESTADO_OFFLINE)
                        .map(|st| st.total)
                        .unwrap_or(0);
                    indicators.push(SliIndicator {
                        key: "offline".into(),
                        label: "Dispositivos offline".into(),
                        value: off.to_string(),
                        target: "0".into(),
                        ok: off == 0,
                    });
                }
                if s.status == "error" {
                    suggested.push("none".into());
                }
            }
            "audit" => {
                indicators.push(SliIndicator {
                    key: "retention_years".into(),
                    label: "Retención de auditoría".into(),
                    value: format!("{} años", support::AUDIT_RETENTION_YEARS),
                    target: ">= 6 años".into(),
                    ok: true,
                });
                if s.status != "ok" {
                    suggested.push("audit_retention".into());
                }
            }
            "backup" => {
                indicators.push(SliIndicator {
                    key: "last_backup".into(),
                    label: "Último backup".into(),
                    value: s
                        .freshness_secs
                        .map(|f| format!("hace {:.0} min", f / 60.0))
                        .unwrap_or_else(|| "sin registro".into()),
                    target: "< 24 h".into(),
                    ok: s.freshness_secs.map(|f| f < 86400.0).unwrap_or(true),
                });
                if s.status != "ok" {
                    suggested.push("backup".into());
                }
            }
            _ => {}
        }

        if suggested.is_empty() {
            suggested.push("none".into());
        }

        out.push(Diagnostic {
            system: s.key.clone(),
            indicators,
            suggested_actions: suggested,
            notes: s.details.clone(),
        });
    }

    out
}

/// GET /admin/support/diagnostics — SLIs por subsistema con acciones sugeridas.
pub async fn get_diagnostics(State(db): State<Database>, _claims: Claims) -> Response {
    let diagnostics = build_diagnostics(&db).await;
    (StatusCode::OK, Json(ApiResponse::ok(diagnostics))).into_response()
}

/// POST /admin/support/actions/{action} — ejecuta, persiste y audita una acción.
pub async fn run_action(
    State(db): State<Database>,
    Path(action): Path<String>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    claims: Claims,
    Json(req): Json<ActionRequest>,
) -> Response {
    if !ACTIONS.contains(&action.as_str()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<Value>::err(format!(
                "acción no soportada: {action} (esperado: {})",
                ACTIONS.join(", ")
            ))),
        )
            .into_response();
    }

    let client_ip = addr.ip().to_string();
    let subsystem = action_subsystem(&action);
    let result = support::execute_action(&action, &req, db.as_ref()).await;

    let event = SupportService::new(db.as_ref().clone())
        .log_event(
            support::EventOrigin::Manual,
            subsystem,
            &action,
            Some(&claims.sub),
            Some(&claims.username),
            Some(&client_ip),
            result.success,
            &result.message,
            result.details.clone(),
        )
        .await;

    if let Some(audit) = crate::audit::audit() {
        let _ = audit
            .log_system_event(
                &claims.sub,
                &claims.username,
                "support.action",
                &format!(
                    "action={action} subsystem={subsystem} success={}",
                    result.success
                ),
            )
            .await;
    }

    let body = ActionResponse {
        action: result.action,
        success: result.success,
        message: result.message,
        details: result.details,
        event: event.ok(),
    };

    let status = if body.success {
        StatusCode::OK
    } else {
        StatusCode::CONFLICT
    };
    (status, Json(ApiResponse::ok(body))).into_response()
}

#[derive(Debug, Deserialize)]
pub struct HistoryQuery {
    #[serde(default)]
    pub limit: Option<usize>,
}

/// GET /admin/support/history — historial de eventos (acciones + self-healing).
pub async fn get_history(
    State(db): State<Database>,
    _claims: Claims,
    Query(q): Query<HistoryQuery>,
) -> Response {
    let limit = q.limit.unwrap_or(50).min(500);
    match SupportService::new(db.as_ref().clone())
        .history(limit)
        .await
    {
        Ok(events) => (StatusCode::OK, Json(ApiResponse::ok(events))).into_response(),
        Err(e) => {
            let msg = crate::security::sanitize_internal_error(&e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<Vec<SupportEvent>>::err(msg)),
            )
                .into_response()
        }
    }
}
