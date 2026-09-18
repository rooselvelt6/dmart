//! SLIs/SLOs y error budgets (SPEC-050, tarea 3.5).
//!
//! Calcula disponibilidad, latencia p95 y frescura SSE sobre ventanas en
//! memoria y los expone como gauges Prometheus + `GET /obs/slo`. Los umbrales
//! provienen del roadmap: 99.5 % disponibilidad, p95 < 100 ms, freshness SSE
//! < 5 s. El budget se mide sobre una ventana de 28 días.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Objetivo de disponibilidad (99.5 %).
pub const SLO_AVAILABILITY_TARGET: f64 = 0.995;
/// Objetivo de latencia p95 en milisegundos.
pub const SLO_LATENCY_TARGET_MS: f64 = 100.0;
/// Objetivo de frescura del stream SSE en segundos.
pub const SLO_SSE_FRESHNESS_TARGET_S: f64 = 5.0;
/// Ventana de cumplimiento del error budget.
pub const ERROR_BUDGET_WINDOW_DAYS: u32 = 28;
/// Fracción permitida de requests por encima del p95 objetivo.
const LATENCY_BAD_RATIO_ALLOWED: f64 = 0.05;

const LATENCY_BUCKETS_MS: [f64; 11] = [
    5.0, 10.0, 25.0, 50.0, 75.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0,
];

static REQUESTS_TOTAL: AtomicU64 = AtomicU64::new(0);
static REQUESTS_5XX: AtomicU64 = AtomicU64::new(0);
static LATENCY_BUCKETS: [AtomicU64; LATENCY_BUCKETS_MS.len()] =
    [const { AtomicU64::new(0) }; LATENCY_BUCKETS_MS.len()];
static SSE_LAST_PUBLISH_MS: AtomicU64 = AtomicU64::new(0);

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Registra una petición HTTP para los SLIs de disponibilidad y latencia.
pub fn record_request(status: u16, latency_ms: f64) {
    REQUESTS_TOTAL.fetch_add(1, Ordering::Relaxed);
    if status >= 500 {
        REQUESTS_5XX.fetch_add(1, Ordering::Relaxed);
    }
    for (i, bound) in LATENCY_BUCKETS_MS.iter().enumerate() {
        if latency_ms <= *bound {
            LATENCY_BUCKETS[i].fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// Marca la publicación de un evento SSE (frescura del stream).
pub fn record_sse_publish() {
    SSE_LAST_PUBLISH_MS.store(now_ms(), Ordering::Relaxed);
}

/// Error budget restante (1.0 = intacto, 0.0 = agotado).
pub fn error_budget_remaining(observed_bad_ratio: f64, allowed_bad_ratio: f64) -> f64 {
    if allowed_bad_ratio <= 0.0 {
        return if observed_bad_ratio <= 0.0 { 1.0 } else { 0.0 };
    }
    (1.0 - observed_bad_ratio / allowed_bad_ratio).clamp(0.0, 1.0)
}

fn p95_latency_ms() -> f64 {
    let total = REQUESTS_TOTAL.load(Ordering::Relaxed);
    if total == 0 {
        return 0.0;
    }
    let target = ((total as f64) * 0.95).ceil() as u64;
    let mut cumulative = 0u64;
    for (i, bound) in LATENCY_BUCKETS_MS.iter().enumerate() {
        cumulative += LATENCY_BUCKETS[i].load(Ordering::Relaxed);
        if cumulative >= target {
            return *bound;
        }
    }
    *LATENCY_BUCKETS_MS.last().unwrap_or(&100.0)
}

fn latency_bad_ratio() -> f64 {
    let total = REQUESTS_TOTAL.load(Ordering::Relaxed);
    if total == 0 {
        return 0.0;
    }
    let at_or_below = LATENCY_BUCKETS[5].load(Ordering::Relaxed); // <= 100 ms
    let bad = total.saturating_sub(at_or_below);
    bad as f64 / total as f64
}

/// Un indicador SLO evaluado.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SloItem {
    pub key: String,
    pub label: String,
    pub unit: String,
    pub value: f64,
    pub target: f64,
    pub ok: bool,
    pub error_budget_remaining: f64,
}

/// Reporte consolidado de SLOs.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SloReport {
    pub window_days: u32,
    pub requests_total: u64,
    pub requests_5xx: u64,
    pub availability: f64,
    pub latency_p95_ms: f64,
    pub sse_freshness_seconds: f64,
    pub items: Vec<SloItem>,
    pub overall_ok: bool,
}

/// Calcula el reporte de SLO actual (sin efectos secundarios).
pub fn snapshot() -> SloReport {
    let total = REQUESTS_TOTAL.load(Ordering::Relaxed);
    let errors = REQUESTS_5XX.load(Ordering::Relaxed);
    let bad_ratio = if total == 0 {
        0.0
    } else {
        errors as f64 / total as f64
    };
    let availability = if total == 0 {
        1.0
    } else {
        (1.0 - bad_ratio).clamp(0.0, 1.0)
    };
    let p95 = p95_latency_ms();
    let latency_bad = latency_bad_ratio();

    let last_sse = SSE_LAST_PUBLISH_MS.load(Ordering::Relaxed);
    let sse_freshness = if last_sse == 0 {
        0.0
    } else {
        now_ms().saturating_sub(last_sse) as f64 / 1000.0
    };

    let availability_budget = error_budget_remaining(bad_ratio, 1.0 - SLO_AVAILABILITY_TARGET);
    let latency_budget = error_budget_remaining(latency_bad, LATENCY_BAD_RATIO_ALLOWED);
    let sse_ok = sse_freshness <= SLO_SSE_FRESHNESS_TARGET_S;
    let sse_budget = if sse_ok { 1.0 } else { 0.0 };

    let items = vec![
        SloItem {
            key: "availability".into(),
            label: "Disponibilidad API".into(),
            unit: "ratio".into(),
            value: availability,
            target: SLO_AVAILABILITY_TARGET,
            ok: availability >= SLO_AVAILABILITY_TARGET,
            error_budget_remaining: availability_budget,
        },
        SloItem {
            key: "latency_p95".into(),
            label: "Latencia p95".into(),
            unit: "ms".into(),
            value: p95,
            target: SLO_LATENCY_TARGET_MS,
            ok: p95 <= SLO_LATENCY_TARGET_MS,
            error_budget_remaining: latency_budget,
        },
        SloItem {
            key: "sse_freshness".into(),
            label: "Frescura SSE".into(),
            unit: "s".into(),
            value: sse_freshness,
            target: SLO_SSE_FRESHNESS_TARGET_S,
            ok: sse_ok,
            error_budget_remaining: sse_budget,
        },
    ];
    let overall_ok = items.iter().all(|i| i.ok);

    SloReport {
        window_days: ERROR_BUDGET_WINDOW_DAYS,
        requests_total: total,
        requests_5xx: errors,
        availability,
        latency_p95_ms: p95,
        sse_freshness_seconds: sse_freshness,
        items,
        overall_ok,
    }
}

/// Publica los gauges SLO en el registro Prometheus.
pub fn export_metrics() {
    let report = snapshot();
    metrics::gauge!("slo_availability_ratio").set(report.availability);
    metrics::gauge!("slo_requests_total").set(report.requests_total as f64);
    metrics::gauge!("slo_requests_5xx_total").set(report.requests_5xx as f64);
    metrics::gauge!("slo_latency_p95_ms").set(report.latency_p95_ms);
    metrics::gauge!("slo_sse_freshness_seconds").set(report.sse_freshness_seconds);
    for item in &report.items {
        metrics::gauge!("slo_error_budget_remaining", "slo" => item.key.clone())
            .set(item.error_budget_remaining);
    }
}

/// Declara las descripciones de las métricas SLO (llamado desde `metrics::register`).
pub fn describe_metrics() {
    use metrics::{Unit, describe_gauge};
    describe_gauge!(
        "slo_availability_ratio",
        Unit::Count,
        "SLI de disponibilidad (1 - 5xx/total)"
    );
    describe_gauge!(
        "slo_requests_total",
        Unit::Count,
        "Total de requests considerados en el SLI"
    );
    describe_gauge!(
        "slo_requests_5xx_total",
        Unit::Count,
        "Requests 5xx observados en la ventana"
    );
    describe_gauge!(
        "slo_latency_p95_ms",
        Unit::Milliseconds,
        "Latencia p95 observada (ms)"
    );
    describe_gauge!(
        "slo_sse_freshness_seconds",
        Unit::Seconds,
        "Segundos desde el último evento SSE publicado"
    );
    describe_gauge!(
        "slo_error_budget_remaining",
        Unit::Count,
        "Error budget restante por SLO (1=lleno, 0=agotado)"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_budget_full_and_exhausted() {
        assert_eq!(error_budget_remaining(0.0, 0.005), 1.0);
        assert_eq!(error_budget_remaining(0.005, 0.005), 0.0);
        assert!((error_budget_remaining(0.0025, 0.005) - 0.5).abs() < 1e-9);
        assert_eq!(error_budget_remaining(0.01, 0.005), 0.0);
    }

    #[test]
    fn snapshot_has_three_items_and_bounded_values() {
        record_request(200, 12.0);
        record_request(500, 250.0);
        let report = snapshot();
        assert_eq!(report.items.len(), 3);
        assert!((0.0..=1.0).contains(&report.availability));
        assert!((0.0..=1.0).contains(&report.items[0].error_budget_remaining));
        assert!(report.latency_p95_ms >= 0.0);
        assert_eq!(report.window_days, ERROR_BUDGET_WINDOW_DAYS);
    }

    #[test]
    fn sse_freshness_updates_on_publish() {
        record_sse_publish();
        let report = snapshot();
        assert!(report.sse_freshness_seconds < SLO_SSE_FRESHNESS_TARGET_S);
        assert!(report.items[2].ok);
    }
}
