//! Métricas de negocio, auth, ingestión y sistema (Prometheus).
//!
//! Envuelve el crate `metrics` centralizando nombres, _describe_ y helpers.
//! Las metricas "de evento" (auth, scales, HL7...) se emiten en los puntos de
//! instrumentación; las "de estado" (`patients_total`, `measurements_total`…)
//! se recalculan en el scrape (`survey_db`) para no desincronizarse con la BD.

use crate::db::Database;
use metrics::{
    Unit, counter, describe_counter, describe_gauge, describe_histogram, gauge, histogram,
};

/// Descripción global de todas las métricas. Se llama una vez en `init_metrics`.
pub fn register() {
    // HTTP
    describe_counter!(
        "http_requests_total",
        Unit::Count,
        "HTTP requests by method/status"
    );
    describe_histogram!(
        "http_request_duration_seconds",
        Unit::Seconds,
        "HTTP request latency (p50/p95/p99 via histogram)"
    );
    describe_counter!(
        "http_requests_errors_total",
        Unit::Count,
        "HTTP error responses (4xx/5xx)"
    );

    // Auth
    describe_counter!("auth_login_total", Unit::Count, "Login attempts by result");
    describe_counter!(
        "auth_refresh_total",
        Unit::Count,
        "Refresh-token rotations by result"
    );
    describe_counter!(
        "auth_failures_total",
        Unit::Count,
        "Failed authentication attempts by reason"
    );
    describe_counter!(
        "rbac_denials_total",
        Unit::Count,
        "Authorization denials (403) by permission"
    );

    // Base de datos
    describe_histogram!(
        "surreal_query_duration_seconds",
        Unit::Seconds,
        "SurrealDB query latency (survey)"
    );
    describe_gauge!(
        "surreal_connection_pool",
        Unit::Count,
        "Active SurrealDB connections"
    );

    // Negocio
    describe_gauge!("patients_total", Unit::Count, "Patients by status");
    describe_counter!("patients_created_total", Unit::Count, "Patients created");
    describe_counter!(
        "patients_deleted_total",
        Unit::Count,
        "Patients deleted/egreso"
    );
    describe_gauge!("measurements_total", Unit::Count, "Measurements persisted");
    describe_counter!(
        "measurements_created_total",
        Unit::Count,
        "Measurements created"
    );
    describe_counter!(
        "scales_calculated_total",
        Unit::Count,
        "Clinical scale scores by scale"
    );

    // ML
    describe_counter!(
        "ml_predictions_total",
        Unit::Count,
        "ML mortality-risk predictions by model"
    );
    describe_histogram!(
        "ml_model_load_duration_seconds",
        Unit::Seconds,
        "ML model load latency"
    );
    describe_gauge!(
        "ml_accuracy_gauge",
        Unit::Count,
        "Reported model accuracy per model/phase"
    );

    // Realtime / señales
    describe_gauge!(
        "sse_connections_active",
        Unit::Count,
        "Active SSE connections"
    );
    describe_counter!(
        "hl7_messages_processed_total",
        Unit::Count,
        "HL7 messages ingested by source"
    );
    describe_counter!(
        "hl7_messages_errors_total",
        Unit::Count,
        "HL7 messages failed by source"
    );

    // Ingest data-quality (SPEC-031). Expuesta y a 0 hasta que SPEC-031 pueble
    // los eventos de gap/fault/throttling.
    describe_counter!(
        "ingest_gap_total",
        Unit::Count,
        "Monitor data gaps (SPEC-031)"
    );
    describe_counter!(
        "ingest_invalid_total",
        Unit::Count,
        "Invalid monitor samples (SPEC-031)"
    );
    describe_gauge!(
        "ingest_fault_devices",
        Unit::Count,
        "Devices in fault state (SPEC-031)"
    );
    describe_counter!(
        "ingest_throttled_total",
        Unit::Count,
        "Throttled ingest events (SPEC-031)"
    );
    describe_gauge!(
        "ingest_error_avg",
        Unit::Count,
        "Average ingest error rate (SPEC-031)"
    );

    // Infra
    describe_gauge!(
        "db_connections_active",
        Unit::Count,
        "Active database connections"
    );
    describe_gauge!(
        "cache_connected",
        Unit::Count,
        "Cache connection status (1=connected, 0=disconnected)"
    );
    describe_gauge!("uptime_seconds", Unit::Seconds, "Server uptime in seconds");
    describe_gauge!(
        "process_cpu_seconds_total",
        Unit::Seconds,
        "Total user+system CPU time used by this process"
    );
    describe_gauge!(
        "process_resident_memory_bytes",
        Unit::Bytes,
        "Resident memory (RSS) of this process"
    );
}

// ─── Auth ────────────────────────────────────────────────────────────────

pub fn auth_login(result: &str) {
    counter!("auth_login_total", "result" => result.to_string()).increment(1);
}

pub fn auth_failure(reason: &str) {
    counter!("auth_failures_total", "reason" => reason.to_string()).increment(1);
}

pub fn auth_refresh(result: &str) {
    counter!("auth_refresh_total", "result" => result.to_string()).increment(1);
}

pub fn rbac_denial(permission: &str) {
    counter!("rbac_denials_total", "permission" => permission.to_string()).increment(1);
}

/// Errores HTTP 4xx/5xx con el path normalizado al primer segmento para
/// mantener la cardinalidad acotada.
pub fn http_error(path: &str) {
    let group = path
        .trim_matches('/')
        .split('/')
        .next()
        .map(|s| if s.is_empty() { "root" } else { s })
        .unwrap_or("root");
    counter!("http_requests_errors_total", "route" => group.to_string()).increment(1);
}

// ─── Negocio ─────────────────────────────────────────────────────────────

pub fn patient_created() {
    counter!("patients_created_total").increment(1);
}

pub fn patient_deleted() {
    counter!("patients_deleted_total").increment(1);
}

pub fn measurement_created() {
    counter!("measurements_created_total").increment(1);
}

pub fn scale_calculated(scale: &str) {
    counter!("scales_calculated_total", "scale" => scale.to_string()).increment(1);
}

// ─── ML ──────────────────────────────────────────────────────────────────

pub fn ml_prediction(model: &str) {
    counter!("ml_predictions_total", "model" => model.to_string()).increment(1);
}

// ─── Realtime / HL7 ──────────────────────────────────────────────────────

use std::sync::atomic::{AtomicI64, Ordering};

static SSE_ACTIVE: AtomicI64 = AtomicI64::new(0);

pub fn sse_connect() {
    SSE_ACTIVE.fetch_add(1, Ordering::Relaxed);
}

pub fn sse_disconnect() {
    SSE_ACTIVE.fetch_sub(1, Ordering::Relaxed);
}

pub fn sse_active() -> i64 {
    SSE_ACTIVE.load(Ordering::Relaxed)
}

pub fn hl7_processed(source: &str) {
    counter!("hl7_messages_processed_total", "source" => source.to_string()).increment(1);
}

pub fn hl7_error(source: &str) {
    counter!("hl7_messages_errors_total", "source" => source.to_string()).increment(1);
}

// ─── Ingest Hardening (SPEC-031) ──────────────────────────────────────────

pub fn ingest_throttled(device: &str) {
    counter!("ingest_throttled_total", "device" => device.to_string()).increment(1);
}

pub fn ingest_gap(device: &str, count: u64) {
    counter!("ingest_gap_total", "device" => device.to_string()).increment(count);
}

pub fn ingest_invalid(device: &str, vital: &str, reason: &str) {
    counter!("ingest_invalid_total", "device" => device.to_string(), "vital" => vital.to_string(), "reason" => reason.to_string()).increment(1);
}

pub fn ingest_message(device: &str, result: &str) {
    counter!("ingest_messages_total", "device" => device.to_string(), "result" => result.to_string()).increment(1);
}

pub fn ingest_message_size(device: &str, size: usize) {
    histogram!("ingest_message_size_bytes", "device" => device.to_string()).record(size as f64);
}

pub fn ingest_fault_devices_set(count: u64) {
    gauge!("ingest_fault_devices").set(count as f64);
}

pub fn ingest_circuit_state(device: &str, state: u8) {
    gauge!("ingest_circuit_state", "device" => device.to_string()).set(state as f64);
}

pub fn ingest_error_avg_set(rate: f64) {
    gauge!("ingest_error_avg").set(rate);
}

pub fn ingest_rate_limit_current(device: &str, tokens: f64) {
    gauge!("ingest_rate_limit_current", "device" => device.to_string()).set(tokens);
}

// ─── Sistema (Linux) ─────────────────────────────────────────────────────

/// Devuelve (cpu_user+sys en segundos, rss en bytes) leyendo /proc/self.
fn proc_self() -> (f64, i64) {
    let Ok(stat) = std::fs::read_to_string("/proc/self/stat") else {
        return (0.0, 0);
    };
    // Campo 14 (utime) y 15 (stime), en ticks de reloj.
    let mut it = stat.split_whitespace();
    for _ in 0..11 {
        let _ = it.next();
    }
    let _comm = it.next(); // (pid) comm
    let mut nums: Vec<f64> = Vec::with_capacity(16);
    for tok in it {
        match tok.parse::<f64>() {
            Ok(n) => nums.push(n),
            Err(_) => break,
        }
    }
    let utime = nums.first().copied().unwrap_or(0.0);
    let stime = nums.get(1).copied().unwrap_or(0.0);
    let hz = 100.0; // USER_HZ estándar en Linux (x86/ARM).
    let cpu = (utime + stime) / hz;

    let rss = std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|status| {
            status.lines().find_map(|l| {
                l.strip_prefix("VmRSS:").map(|v| {
                    v.trim()
                        .trim_end_matches(" kB")
                        .trim()
                        .parse::<i64>()
                        .unwrap_or(0)
                        * 1024
                })
            })
        })
        .unwrap_or(0);

    (cpu, rss)
}

// ─── Survey en scrape ────────────────────────────────────────────────────

/// Feature flag de rollout (SPEC-005): `METRICS_EXTENDED` (default ON).
/// Cuando es `false` el endpoint solo expone métricas de sistema/HTTP y se
/// ocultan los KPIs de negocio, ML, interop HL7 e ingest.
pub fn extended_enabled() -> bool {
    extended_flag(std::env::var("METRICS_EXTENDED").ok().as_deref())
}

/// Lógica pura del flag, testeable sin tocar el entorno.
fn extended_flag(value: Option<&str>) -> bool {
    match value {
        None => true,
        Some(v) => {
            let v = v.trim().to_ascii_lowercase();
            !(v == "false" || v == "0" || v == "no" || v == "off")
        }
    }
}

/// Ejecuta un `SELECT count() AS count FROM <tabla> [WHERE ...]` y devuelve el
/// número, o `None` si la consulta falla (el scrape nunca debe romperse).
async fn survey_count(db: &Database, table: &str, where_clause: &str) -> Option<f64> {
    let query = if where_clause.is_empty() {
        format!("SELECT count() AS count FROM {table} GROUP BY count")
    } else {
        format!("SELECT count() AS count FROM {table} WHERE {where_clause} GROUP BY count")
    };
    let rows: Vec<serde_json::Value> = db.query(query).await.ok()?.take(0).ok()?;
    // Tabla vacía o sin coincidencias devuelve [] → el total real es 0.
    Some(
        rows.first()
            .and_then(|v| v["count"].as_f64())
            .unwrap_or(0.0),
    )
}

/// Recalcula métricas de estado consultando la BD y leyendo /proc. Lo ejecuta
/// el handler de `/metrics` en cada scrape para no depender del signo de vida
/// de contadores en memoria (evita desincronización tras reinicios).
pub async fn survey_db(db: &Database, extended: bool) {
    let start = std::time::Instant::now();

    if extended {
        if let Some(n) = survey_count(db, "patients", "").await {
            gauge!("patients_total", "status" => "all").set(n);
        }
        if let Some(n) = survey_count(
            db,
            "patients",
            "(fecha_egreso_uci IS NONE OR fecha_egreso_uci = '')",
        )
        .await
        {
            gauge!("patients_total", "status" => "active").set(n);
        }
        if let Some(n) = survey_count(db, "measurements", "").await {
            gauge!("measurements_total").set(n);
        }
        if let Some(n) = survey_count(db, "camas", "").await {
            gauge!("camas_total").set(n);
        }
        if let Some(n) = survey_count(db, "camas", "estado = 'Ocupada'").await {
            gauge!("camas_ocupadas").set(n);
        }

        histogram!("surreal_query_duration_seconds").record(start.elapsed().as_secs_f64());
        gauge!("surreal_connection_pool").set(1.0);
    }

    let (cpu, rss) = proc_self();
    gauge!("process_cpu_seconds_total").set(cpu);
    gauge!("process_resident_memory_bytes").set(rss as f64);
    gauge!("sse_connections_active").set(sse_active() as f64);
}

/// Garantiza que las métricas de evento existan en la primera exposición
/// (Prometheus solo emite series que se han registrado al menos una vez).
/// `extended=false` deja fuera los KPIs de negocio/ML/HL7/ingest para que el
/// flag `METRICS_EXTENDED` controle la superficie expuesta.
pub fn touch_zero_counters(extended: bool) {
    // `.increment(0)` garantiza que la serie exista en la primera exposición
    // sin resetear contadores que ya hayan acumulado eventos.
    for result in ["success", "failure"] {
        counter!("auth_login_total", "result" => result.to_string()).increment(0);
        counter!("auth_refresh_total", "result" => result.to_string()).increment(0);
    }
    counter!("auth_failures_total", "reason" => "none".to_string()).increment(0);
    counter!("rbac_denials_total", "permission" => "none".to_string()).increment(0);
    counter!("http_requests_errors_total").increment(0);

    if extended {
        counter!("patients_created_total").increment(0);
        counter!("patients_deleted_total").increment(0);
        counter!("measurements_created_total").increment(0);
        for scale in ["apache", "gcs", "news2", "sofa", "saps3"] {
            counter!("scales_calculated_total", "scale" => scale.to_string()).increment(0);
        }
        counter!("ml_predictions_total", "model" => "apache_mortality_v1".to_string()).increment(0);
        histogram!("ml_model_load_duration_seconds").record(0.0);
        gauge!("ml_accuracy_gauge", "model" => "apache_mortality_v1".to_string()).set(0.0);
        for source in ["Mindray", "Philips", "Genérico", "unknown"] {
            counter!("hl7_messages_processed_total", "source" => source.to_string()).increment(0);
            counter!("hl7_messages_errors_total", "source" => source.to_string()).increment(0);
        }
        counter!("ingest_gap_total").increment(0);
        counter!("ingest_invalid_total").increment(0);
        gauge!("ingest_fault_devices").set(0.0);
        counter!("ingest_throttled_total").increment(0);
        gauge!("ingest_error_avg").set(0.0);
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extended_flag_defaults_to_true() {
        assert!(extended_flag(None));
        assert!(extended_flag(Some("")));
        assert!(extended_flag(Some("true")));
        assert!(extended_flag(Some("1")));
        assert!(extended_flag(Some(" TRUE ")));
        assert!(extended_flag(Some("yes")));
    }

    #[test]
    fn extended_flag_off_values() {
        assert!(!extended_flag(Some("false")));
        assert!(!extended_flag(Some("0")));
        assert!(!extended_flag(Some("no")));
        assert!(!extended_flag(Some("off")));
        assert!(!extended_flag(Some(" False ")));
    }
}
