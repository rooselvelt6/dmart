use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use dashmap::DashMap;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use statrs::distribution::ContinuousCDF;

use dmart_shared::{ForecastPoint, ForecastRequest, ForecastResponse, ForecastStatus};

/// Configuración del forecaster TimesFM (sidecar).
#[derive(Debug, Clone, Deserialize)]
pub struct TimesFmConfig {
    pub url: String,
    pub timeout_secs: u64,
    pub context_length: usize,
    pub max_horizon: usize,
    pub model_version: String,
}

impl Default for TimesFmConfig {
    fn default() -> Self {
        Self {
            url: "http://127.0.0.1:8082".to_string(),
            timeout_secs: 10,
            context_length: 16384,
            max_horizon: 256,
            model_version: "timesfm-2.5-200m".to_string(),
        }
    }
}

/// Forecaster backend trait (pluggable, como Predictor).
pub trait Forecaster: Send + Sync {
    fn forecast(&self, req: &ForecastRequest) -> Result<ForecastResponse>;
    fn name(&self) -> &'static str;
    fn version(&self) -> &str;
    fn max_context(&self) -> usize;
    fn max_horizon(&self) -> usize;
}

/// Request/Response hacia el sidecar (JSON API).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SidecarForecastRequest {
    series_id: String,
    values: Vec<f32>,
    horizon: usize,
    quantiles: Vec<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    covariates: Option<Vec<f32>>,
}

#[derive(Debug, Clone, Deserialize)]
struct SidecarForecastResponse {
    series_id: String,
    horizon: usize,
    points: Vec<SidecarForecastPoint>,
    model: String,
    model_version: String,
    latency_ms: f64,
}

#[derive(Debug, Clone, Deserialize)]
struct SidecarForecastPoint {
    step: usize,
    point: f32,
    quantiles: Vec<f32>,
}

#[derive(Debug, Clone, Deserialize)]
struct SidecarStatusResponse {
    enabled: bool,
    backend: String,
    model_version: String,
    context_length: usize,
    max_horizon: usize,
}

/// Cliente HTTP al sidecar TimesFM.
pub struct TimesFmClient {
    client: Client,
    config: TimesFmConfig,
}

impl TimesFmClient {
    pub fn new(config: TimesFmConfig) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .expect("reqwest client");
        Self { client, config }
    }

    async fn call_forecast(&self, req: &ForecastRequest) -> Result<ForecastResponse> {
        let url = format!("{}/forecast", self.config.url);
        let body = SidecarForecastRequest {
            series_id: req.series_id.clone(),
            values: req.values.clone(),
            horizon: req.horizon,
            quantiles: req.quantiles.clone(),
            covariates: req.covariates.clone(),
        };
        let resp = self.client.post(&url).json(&body).send().await?;
        if !resp.status().is_success() {
            let err = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("sidecar error: {}", err));
        }
        let resp: SidecarForecastResponse = resp.json().await?;
        Ok(ForecastResponse {
            series_id: resp.series_id,
            horizon: resp.horizon,
            points: resp
                .points
                .into_iter()
                .map(|p| ForecastPoint {
                    step: p.step,
                    point: p.point,
                    quantiles: p.quantiles,
                })
                .collect(),
            model: resp.model,
            model_version: resp.model_version,
            latency_ms: resp.latency_ms,
        })
    }

    async fn call_status(&self) -> Result<ForecastStatus> {
        let url = format!("{}/status", self.config.url);
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(anyhow::anyhow!("sidecar status error"));
        }
        let resp: SidecarStatusResponse = resp.json().await?;
        Ok(ForecastStatus {
            enabled: resp.enabled,
            backend: resp.backend,
            model_version: resp.model_version,
            context_length: resp.context_length,
            max_horizon: resp.max_horizon,
        })
    }
}

impl Forecaster for TimesFmClient {
    fn forecast(&self, req: &ForecastRequest) -> Result<ForecastResponse> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.call_forecast(req))
        })
    }

    fn name(&self) -> &'static str {
        "timesfm"
    }

    fn version(&self) -> &str {
        &self.config.model_version
    }

    fn max_context(&self) -> usize {
        self.config.context_length
    }

    fn max_horizon(&self) -> usize {
        self.config.max_horizon
    }
}

/// Fallback determinístico (sin sidecar): último valor + ruido residual simple.
/// Útil para desarrollo/CI y como fallback grácil si el sidecar falla.
pub struct NaiveForecaster;

impl NaiveForecaster {
    fn simple_forecast(&self, values: &[f32], horizon: usize, quantiles: &[f32]) -> Vec<ForecastPoint> {
        if values.is_empty() {
            return vec![];
        }
        let last = values[values.len() - 1];
        let mean = values.iter().sum::<f32>() / values.len() as f32;
        let std = if values.len() > 1 {
            let var = values.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / values.len() as f32;
            var.sqrt()
        } else {
            1.0
        };
        // Cuantiles normales simples
        let z: Vec<f32> = quantiles
            .iter()
            .map(|&q| {
                let dist = statrs::distribution::Normal::new(0.0, 1.0).unwrap();
                dist.inverse_cdf(q as f64) as f32
            })
            .collect();
        (1..=horizon)
            .map(|step| {
                let point = last; // naïve: persist last value
                let quantiles: Vec<f32> = z.iter().map(|&z| point + z * std).collect();
                ForecastPoint {
                    step,
                    point,
                    quantiles,
                }
            })
            .collect()
    }
}

impl Forecaster for NaiveForecaster {
    fn forecast(&self, req: &ForecastRequest) -> Result<ForecastResponse> {
        let start = Instant::now();
        let quantiles = if req.quantiles.is_empty() {
            vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9]
        } else {
            req.quantiles.clone()
        };
        let points = self.simple_forecast(&req.values, req.horizon, &quantiles);
        let latency_ms = start.elapsed().as_secs_f64() * 1000.0;
        Ok(ForecastResponse {
            series_id: req.series_id.clone(),
            horizon: req.horizon,
            points,
            model: "naive".to_string(),
            model_version: "v1.0.0".to_string(),
            latency_ms,
        })
    }

    fn name(&self) -> &'static str {
        "naive"
    }

    fn version(&self) -> &str {
        "v1.0.0"
    }

    fn max_context(&self) -> usize {
        16384
    }

    fn max_horizon(&self) -> usize {
        256
    }
}

/// Registry de forecasters (siguiendo el patrón MlRegistry).
pub struct ForecastRegistry {
    forecasters: Arc<DashMap<String, Arc<dyn Forecaster>>>,
    active: Arc<dashmap::DashMap<String, String>>,
}

impl ForecastRegistry {
    pub fn new() -> Self {
        Self {
            forecasters: Arc::new(DashMap::new()),
            active: Arc::new(dashmap::DashMap::new()),
        }
    }

    pub fn register(&self, key: &str, forecaster: Arc<dyn Forecaster>) {
        self.forecasters.insert(key.to_string(), forecaster);
    }

    pub fn get(&self, key: &str) -> Option<Arc<dyn Forecaster>> {
        self.forecasters.get(key).map(|v| v.value().clone())
    }

    pub fn set_active(&self, key: &str) {
        self.active.insert(key.to_string(), key.to_string());
    }

    pub fn active(&self) -> Option<Arc<dyn Forecaster>> {
        if let Some(key) = self.active.get("default") {
            self.get(key.value())
        } else {
            None
        }
    }
}

impl Default for ForecastRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Servicio de forecasting unificado (elige backend por feature flag / env).
pub struct ForecastService {
    registry: ForecastRegistry,
}

impl ForecastService {
    pub fn new() -> Self {
        let registry = ForecastRegistry::new();

        // Siempre registramos naive como fallback
        registry.register("naive", Arc::new(NaiveForecaster));

        // Si hay URL de sidecar, registramos TimesFM
        let timesfm_url = std::env::var("DMART_TIMESFM_URL").unwrap_or_default();
        if !timesfm_url.is_empty() {
            let mut config = TimesFmConfig::default();
            config.url = timesfm_url;
            if let Ok(v) = std::env::var("DMART_TIMESFM_TIMEOUT") {
                if let Ok(t) = v.parse() {
                    config.timeout_secs = t;
                }
            }
            registry.register("timesfm", Arc::new(TimesFmClient::new(config)));
        }

        // Backend activo por env var, default naive
        let active_backend = std::env::var("DMART_FORECASTER").unwrap_or_else(|_| "naive".to_string());
        registry.set_active(&active_backend);

        Self { registry }
    }

    pub fn forecast(&self, req: &ForecastRequest) -> Result<ForecastResponse> {
        let forecaster = self
            .registry
            .active()
            .ok_or_else(|| anyhow::anyhow!("no active forecaster"))?;
        let start = Instant::now();
        let mut resp = forecaster.forecast(req)?;
        resp.latency_ms = start.elapsed().as_secs_f64() * 1000.0;
        Ok(resp)
    }

    pub fn status(&self) -> ForecastStatus {
        if let Some(f) = self.registry.active() {
            ForecastStatus {
                enabled: true,
                backend: f.name().to_string(),
                model_version: f.version().to_string(),
                context_length: f.max_context(),
                max_horizon: f.max_horizon(),
            }
        } else {
            ForecastStatus {
                enabled: false,
                backend: "none".to_string(),
                model_version: "none".to_string(),
                context_length: 0,
                max_horizon: 0,
            }
        }
    }
}

impl Default for ForecastService {
    fn default() -> Self {
        Self::new()
    }
}

static FORECAST_SERVICE: std::sync::OnceLock<ForecastService> = std::sync::OnceLock::new();

pub fn service() -> &'static ForecastService {
    FORECAST_SERVICE.get_or_init(ForecastService::new)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dmart_shared::ForecastRequest;

    fn dummy_req() -> ForecastRequest {
        ForecastRequest {
            series_id: "test:MAP".to_string(),
            values: (0..100).map(|i| (i as f32 * 0.1).sin() * 10.0 + 80.0).collect(),
            horizon: 6,
            quantiles: vec![0.1, 0.5, 0.9],
            covariates: None,
        }
    }

    #[test]
    fn naive_forecast_returns_points() {
        let f = NaiveForecaster;
        let r = f.forecast(&dummy_req()).expect("forecast");
        assert_eq!(r.horizon, 6);
        assert_eq!(r.points.len(), 6);
        assert!(r.points.iter().all(|p| p.quantiles.len() == 3));
    }

    #[test]
    fn naive_forecast_short_series() {
        let req = ForecastRequest {
            series_id: "short".to_string(),
            values: vec![85.0, 86.0],
            horizon: 2,
            quantiles: vec![0.5],
            covariates: None,
        };
        let f = NaiveForecaster;
        let r = f.forecast(&req).expect("forecast");
        assert_eq!(r.points.len(), 2);
        assert_eq!(r.points[0].quantiles.len(), 1);
    }
}