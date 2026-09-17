//! SPEC-030: Retención y downsampling de mediciones (crecimiento controlado).
//!
//! Capas de datos:
//! - [`measurements`] raw → granularidad 1 min, se purga tras `raw_days`.
//! - [`measurements_hourly`] → AVG/MIN/MAX por hora (truncada a la hora exacta),
//!   se purga tras `hourly_months`.
//! - [`measurements_daily`] → AVG/MIN/MAX por día, se conservan indefinido.
//!
//! Los agregados NO guardan PHI: solo scores + `patient_id` + timestamp del
//! bucket + `count`. El upsert usa una clave determinista por bucket
//! `patient_id#timestamp`, por lo que correr el job dos veces es idempotente
//! (recalcula el mismo bucket, no duplica filas).

use anyhow::{Result, anyhow};
use chrono::{DateTime, Duration as ChronoDuration, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use surrealdb::Surreal;
use surrealdb::engine::local::Db;

use crate::db::Database;

/// Tamaño de lote para agregar/purgar filas sin transacciones gigantes.
pub const AGGREGATE_BATCH: usize = 10_000;
/// Tabla de agregados por hora.
pub const TABLE_HOURLY: &str = "measurements_hourly";
/// Tabla de agregados por día.
pub const TABLE_DAILY: &str = "measurements_daily";
/// Tabla de mediciones raw (granularidad fina).
const TABLE_RAW: &str = "measurements";
/// Antigüedad mínima de un bucket para considerarse cerrado (grace 1 h).
const BUCKET_GRACE_HOURS: i64 = 1;
/// Umbral de disco crítico: free space < 10%.
const DISK_CRITICAL_FREE_RATIO: f64 = 0.10;

// ─── Configuración de retención ───────────────────────────────────────────

/// Política de retención activa.
///
/// - `raw_days`: mediciones raw se purgan después de N días.
/// - `hourly_months`: se mantienen agregados por hora durante M meses; luego se
///   purgan (el agregado diario ya cubre ese histórico).
/// - `daily_forever`: los agregados diarios se conservan indefinido.
/// - `enabled`: si es `false`, el job automático no arranca (default).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetentionConfig {
    pub raw_days: u32,
    pub hourly_months: u32,
    pub daily_forever: bool,
    pub enabled: bool,
}

impl Default for RetentionConfig {
    fn default() -> Self {
        Self {
            raw_days: 7,
            hourly_months: 6,
            daily_forever: true,
            enabled: false,
        }
    }
}

/// Lee los overrides de entorno (`RETENTION_RAW_DAYS`, `RETENTION_HOURLY_MONTHS`,
/// `RETENTION_ENABLED`). Con `RETENTION_ENABLED` ausente o distinto de
/// `true`/`1`, el job automático queda deshabilitado (cero riesgo en tests).
pub fn config_from_env() -> RetentionConfig {
    let mut cfg = RetentionConfig::default();
    if let Ok(v) = std::env::var("RETENTION_RAW_DAYS")
        && let Ok(d) = v.trim().parse::<u32>()
    {
        cfg.raw_days = d;
    }
    if let Ok(v) = std::env::var("RETENTION_HOURLY_MONTHS")
        && let Ok(m) = v.trim().parse::<u32>()
    {
        cfg.hourly_months = m;
    }
    if let Ok(v) = std::env::var("RETENTION_ENABLED") {
        cfg.enabled = v.trim().eq_ignore_ascii_case("true") || v.trim() == "1";
    }
    cfg
}

// ─── Reporte y status ─────────────────────────────────────────────────────

/// Reporte de una ejecución de downsampling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionRunReport {
    /// Filas raw eliminadas por retención.
    pub raw_deleted: u64,
    /// Buckets horarios insertados/actualizados (upsert).
    pub hourly_upserted: u64,
    /// Buckets horarios purgados (más viejos que `hourly_months`).
    pub hourly_deleted: u64,
    /// Buckets diarios insertados/actualizados (upsert).
    pub daily_upserted: u64,
    /// Total de buckets procesados (hourly + daily upserted).
    pub buckets: u64,
    /// Momento de la ejecución (RFC3339 UTC).
    pub run_at: String,
}

impl RetentionRunReport {
    fn new() -> Self {
        Self {
            raw_deleted: 0,
            hourly_upserted: 0,
            hourly_deleted: 0,
            daily_upserted: 0,
            buckets: 0,
            run_at: Utc::now().to_rfc3339(),
        }
    }
}

/// Estado consultable por `GET /admin/retention/status`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionStatus {
    pub raw_count: u64,
    pub hourly_count: u64,
    pub daily_count: u64,
    pub last_run: Option<RetentionRunReport>,
    pub config: RetentionConfig,
}

// ─── Agregación por bucket ────────────────────────────────────────────────

/// Agregados de un solo score dentro de un bucket.
#[derive(Debug, Clone, Default)]
struct ScoreAgg {
    sum: f64,
    n: u64,
    min: Option<f64>,
    max: Option<f64>,
}

impl ScoreAgg {
    fn add(&mut self, value: f64) {
        self.sum += value;
        self.n += 1;
        self.min = Some(self.min.map_or(value, |m| m.min(value)));
        self.max = Some(self.max.map_or(value, |m| m.max(value)));
    }

    fn avg(&self) -> f64 {
        if self.n == 0 {
            0.0
        } else {
            self.sum / self.n as f64
        }
    }

    fn minv(&self) -> f64 {
        self.min.unwrap_or(0.0)
    }

    fn maxv(&self) -> f64 {
        self.max.unwrap_or(0.0)
    }
}

/// Todos los agregados de un bucket (patient_id + timestamp-hora/día).
#[derive(Debug, Clone, Default)]
struct BucketAgg {
    count: u64,
    apache: ScoreAgg,
    gcs: ScoreAgg,
    saps3: ScoreAgg,
    news2: ScoreAgg,
    sofa: ScoreAgg,
}

/// Fila raw proyectada para agregación (sin datos PHI).
#[derive(Debug, Deserialize)]
struct RawRow {
    measurement_id: String,
    patient_id: String,
    timestamp: String,
    apache_score: Option<u32>,
    gcs_score: Option<u32>,
    saps3_score: Option<u32>,
    news2_score: Option<u32>,
    sofa_score: Option<u32>,
}

/// Clave del bucket dentro de los mapas: `(patient_id, timestamp-bucket)`.
type BucketKey = (String, String);

/// Trunca a la hora exacta (UTC).
fn truncate_hour(dt: DateTime<Utc>) -> DateTime<Utc> {
    let minute_zero = dt
        .naive_utc()
        .with_minute(0)
        .and_then(|t| t.with_second(0))
        .and_then(|t| t.with_nanosecond(0))
        .expect("hora válida");
    DateTime::from_naive_utc_and_offset(minute_zero, Utc)
}

/// Trunca al día (medianoche UTC).
fn truncate_day(dt: DateTime<Utc>) -> DateTime<Utc> {
    let midnight = dt
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .expect("medianoche válida");
    DateTime::from_naive_utc_and_offset(midnight, Utc)
}

/// Clave global del bucket para el RecordId determinista.
fn bucket_key(patient_id: &str, bucket_ts: &str) -> String {
    format!("{patient_id}#{bucket_ts}")
}

fn accumulate(
    map: &mut HashMap<BucketKey, BucketAgg>,
    patient_id: &str,
    bucket: DateTime<Utc>,
    row: &RawRow,
) {
    let entry = map
        .entry((patient_id.to_string(), bucket.to_rfc3339()))
        .or_default();
    entry.count += 1;
    if let Some(v) = row.apache_score {
        entry.apache.add(f64::from(v));
    }
    if let Some(v) = row.gcs_score {
        entry.gcs.add(f64::from(v));
    }
    if let Some(v) = row.saps3_score {
        entry.saps3.add(f64::from(v));
    }
    if let Some(v) = row.news2_score {
        entry.news2.add(f64::from(v));
    }
    if let Some(v) = row.sofa_score {
        entry.sofa.add(f64::from(v));
    }
}

/// Lee las mediciones raw por lotes (keyset por `timestamp, measurement_id`) y
/// las agrega en memoria por bucket horario y diario. Solo se agregan buckets
/// cerrados (`bucket_end <= closed_before`, con grace de 1 h).
async fn aggregate_raw(
    db: &Surreal<Db>,
    closed_before: &DateTime<Utc>,
) -> Result<(HashMap<BucketKey, BucketAgg>, HashMap<BucketKey, BucketAgg>)> {
    const SQL: &str = "\
        SELECT measurement_id, patient_id, timestamp, apache_score, gcs_score, \
               saps3_score, news2_score, sofa_score \
        FROM measurements \
        WHERE timestamp < $before \
          AND (timestamp > $last_ts OR (timestamp = $last_ts AND measurement_id > $last_id)) \
        ORDER BY timestamp ASC, measurement_id ASC \
        LIMIT $limit";

    let before = closed_before.to_rfc3339();
    let mut hourly: HashMap<BucketKey, BucketAgg> = HashMap::new();
    let mut daily: HashMap<BucketKey, BucketAgg> = HashMap::new();
    let mut last_ts = String::new();
    let mut last_id = String::new();

    loop {
        let rows: Vec<RawRow> = db
            .query(SQL)
            .bind(("before", before.clone()))
            .bind(("last_ts", last_ts.clone()))
            .bind(("last_id", last_id.clone()))
            .bind(("limit", AGGREGATE_BATCH as i64))
            .await?
            .take(0)?;

        let batch_len = rows.len();
        if batch_len == 0 {
            break;
        }

        for row in rows {
            let Ok(parsed) = DateTime::parse_from_rfc3339(&row.timestamp) else {
                continue;
            };
            let dt = parsed.with_timezone(&Utc);

            let hb = truncate_hour(dt);
            if hb + ChronoDuration::hours(BUCKET_GRACE_HOURS) <= *closed_before {
                accumulate(&mut hourly, &row.patient_id, hb, &row);
            }

            let dbucket = truncate_day(dt);
            if dbucket + ChronoDuration::hours(BUCKET_GRACE_HOURS + 24) <= *closed_before {
                accumulate(&mut daily, &row.patient_id, dbucket, &row);
            }

            last_ts = row.timestamp;
            last_id = row.measurement_id;
        }

        if batch_len < AGGREGATE_BATCH {
            break;
        }
    }

    Ok((hourly, daily))
}

// ─── Escritura idempotente de buckets ─────────────────────────────────────

async fn upsert_bucket(
    db: &Surreal<Db>,
    table: &str,
    patient_id: &str,
    bucket_ts: &str,
    agg: &BucketAgg,
) -> Result<()> {
    // `table` proviene de constantes internas, nunca de entrada del usuario.
    let sql = format!("UPSERT type::thing('{table}', $key) CONTENT $data");

    let data = serde_json::json!({
        "patient_id": patient_id,
        "timestamp": bucket_ts,
        "count": agg.count,
        "apache_avg": agg.apache.avg(),
        "apache_min": agg.apache.minv(),
        "apache_max": agg.apache.maxv(),
        "gcs_avg": agg.gcs.avg(),
        "gcs_min": agg.gcs.minv(),
        "gcs_max": agg.gcs.maxv(),
        "saps3_avg": agg.saps3.avg(),
        "saps3_min": agg.saps3.minv(),
        "saps3_max": agg.saps3.maxv(),
        "news2_avg": agg.news2.avg(),
        "news2_min": agg.news2.minv(),
        "news2_max": agg.news2.maxv(),
        "sofa_avg": agg.sofa.avg(),
        "sofa_min": agg.sofa.minv(),
        "sofa_max": agg.sofa.maxv(),
    });

    let mut res = db
        .query(sql)
        .bind(("key", bucket_key(patient_id, bucket_ts)))
        .bind(("data", data))
        .await?;
    if let Some((_, err)) = res.take_errors().into_iter().next() {
        return Err(anyhow!(
            "upsert bucket {table}/{patient_id}/{bucket_ts}: {err}"
        ));
    }
    Ok(())
}

// ─── Purgas por retención ─────────────────────────────────────────────────

/// Borra filas por lotes usando la clave pública de cada fila.
async fn delete_rows(db: &Surreal<Db>, table: &str, ids: &[String]) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    let sql = format!("FOR $id IN $ids {{ DELETE type::thing('{table}', $id); }}");
    let mut res = db.query(sql).bind(("ids", ids.to_vec())).await?;
    if let Some((_, err)) = res.take_errors().into_iter().next() {
        return Err(anyhow!("delete {table}: {err}"));
    }
    Ok(())
}

/// Purga de `measurements` el raw más antiguo que `raw_days`.
async fn purge_raw(db: &Surreal<Db>, before: &DateTime<Utc>) -> Result<u64> {
    let before = before.to_rfc3339();
    let mut total: u64 = 0;
    loop {
        let ids: Vec<String> = db
            .query("SELECT VALUE measurement_id FROM measurements WHERE timestamp < $before LIMIT $limit")
            .bind(("before", before.clone()))
            .bind(("limit", AGGREGATE_BATCH as i64))
            .await?
            .take(0)?;
        if ids.is_empty() {
            break;
        }
        delete_rows(db, TABLE_RAW, &ids).await?;
        total += ids.len() as u64;
    }
    Ok(total)
}

/// Purga de `measurements_hourly` los buckets más antiguos que `hourly_months`.
async fn purge_hourly(db: &Surreal<Db>, before: &DateTime<Utc>) -> Result<u64> {
    const SQL: &str = "\
        SELECT patient_id, timestamp \
        FROM measurements_hourly \
        WHERE timestamp < $before \
        LIMIT $limit";
    #[derive(Debug, Deserialize)]
    struct HourlyKey {
        patient_id: String,
        timestamp: String,
    }

    let before = before.to_rfc3339();
    let mut total: u64 = 0;
    loop {
        let rows: Vec<HourlyKey> = db
            .query(SQL)
            .bind(("before", before.clone()))
            .bind(("limit", AGGREGATE_BATCH as i64))
            .await?
            .take(0)?;
        if rows.is_empty() {
            break;
        }
        let keys: Vec<String> = rows
            .iter()
            .map(|r| bucket_key(&r.patient_id, &r.timestamp))
            .collect();
        delete_rows(db, TABLE_HOURLY, &keys).await?;
        total += keys.len() as u64;
    }
    Ok(total)
}

// ─── Durante el job diario ────────────────────────────────────────────────

fn db_path() -> String {
    std::env::var("DMART_DB_PATH").unwrap_or_else(|_| "/".to_string())
}

/// Detecta disco crítico (free space < 10%) vía `statvfs` sobre la rama del
/// path de datos (o `/` si no está configurado). Si no se puede determinar,
/// devuelve `false`: el orden de agregación ya prioriza el hourly.
fn disk_critical(path: &str) -> bool {
    let Ok(cpath) = std::ffi::CString::new(path) else {
        return false;
    };
    let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::statvfs(cpath.as_ptr(), &mut st) };
    if rc != 0 {
        return false;
    }
    let total = (st.f_blocks as f64) * (st.f_frsize as f64);
    let available = (st.f_bavail as f64) * (st.f_frsize as f64);
    total > 0.0 && available / total < DISK_CRITICAL_FREE_RATIO
}

async fn persist_last_run(db: &Surreal<Db>, report: &RetentionRunReport) -> Result<()> {
    let data = serde_json::json!({
        "run_at": report.run_at,
        "report": report,
    });
    db.query("UPSERT type::thing('retention_jobs', 'last') CONTENT $data")
        .bind(("data", data))
        .await?;
    Ok(())
}

/// Ejecuta el downsampling con la política activa (entorno). Es la función
/// invocada por la API y por el job diario.
pub async fn run_downsample(db: &Surreal<Db>) -> Result<RetentionRunReport> {
    run_downsample_with(db, &config_from_env()).await
}

/// Ejecuta el downsampling con una política explícita (idempotente):
/// 1. asegura las tablas de agregados (DEFINE ... IF NOT EXISTS);
/// 2. agrega los buckets horarios y diarios cerrados desde `measurements`;
/// 3. purga raw más antiguo que `raw_days` y horarios más antiguos que
///    `hourly_months`. El agregado horario siempre ocurre antes del purge raw
///    (si el disco está crítico, esa prioridad es la que se documenta);
/// 4. guarda el reporte en `retention_jobs` para `GET /status`.
pub async fn run_downsample_with(
    db: &Surreal<Db>,
    cfg: &RetentionConfig,
) -> Result<RetentionRunReport> {
    ensure_tables(db).await?;

    let now = Utc::now();
    // Solo se agregan buckets completos (grace 1 h).
    let closed_before = now - ChronoDuration::hours(BUCKET_GRACE_HOURS);
    let raw_cutoff = now - ChronoDuration::days(i64::from(cfg.raw_days));
    // Aproximación: meses de 30 días (documentado en el reporte).
    let hourly_cutoff = now - ChronoDuration::days(i64::from(cfg.hourly_months) * 30);

    let (hourly, daily) = aggregate_raw(db, &closed_before).await?;

    // Si el disco está crítico, lo registramos: el hourly ya se agrega antes
    // del purge raw, que es la prioridad exigida por la spec.
    if disk_critical(&db_path()) {
        tracing::warn!(
            "disco crítico (<10% free): retención prioriza agregado horario antes del purge raw"
        );
    }

    let mut report = RetentionRunReport::new();

    for ((patient_id, ts), agg) in &hourly {
        upsert_bucket(db, TABLE_HOURLY, patient_id, ts, agg).await?;
        report.hourly_upserted += 1;
    }
    for ((patient_id, ts), agg) in &daily {
        upsert_bucket(db, TABLE_DAILY, patient_id, ts, agg).await?;
        report.daily_upserted += 1;
    }
    report.buckets = report.hourly_upserted + report.daily_upserted;

    report.raw_deleted = purge_raw(db, &raw_cutoff).await?;
    report.hourly_deleted = purge_hourly(db, &hourly_cutoff).await?;

    persist_last_run(db, &report).await?;
    Ok(report)
}

/// Asegura la existencia de las tablas de agregados (mismo SQL idempotente que
/// la migración `030_measurements_downsample.surql`).
async fn ensure_tables(db: &Surreal<Db>) -> Result<()> {
    db.query(include_str!(
        "../migrations/030_measurements_downsample.surql"
    ))
    .await?;
    Ok(())
}

// ─── Status / consultas ───────────────────────────────────────────────────

async fn count_table(db: &Surreal<Db>, table: &str) -> Result<u64> {
    let sql = format!("SELECT count() AS n FROM {table} GROUP BY count");
    let rows: Vec<serde_json::Value> = db.query(sql).await?.take(0)?;
    Ok(rows
        .first()
        .and_then(|v| v.get("n").and_then(|x| x.as_u64()))
        .unwrap_or(0))
}

pub async fn last_run(db: &Surreal<Db>) -> Result<Option<RetentionRunReport>> {
    let rows: Vec<RetentionRunReport> = db
        .query("SELECT VALUE report FROM retention_jobs WHERE id = type::thing('retention_jobs', 'last')")
        .await?
        .take(0)?;
    Ok(rows.into_iter().next())
}

/// Counts actuales por tabla + última ejecución, para `GET /admin/retention/status`.
pub async fn retention_status(db: &Surreal<Db>) -> Result<RetentionStatus> {
    Ok(RetentionStatus {
        raw_count: count_table(db, TABLE_RAW).await?,
        hourly_count: count_table(db, TABLE_HOURLY).await?,
        daily_count: count_table(db, TABLE_DAILY).await?,
        last_run: last_run(db).await?,
        config: config_from_env(),
    })
}

// ─── Job automático ───────────────────────────────────────────────────────

/// Programa el job diario de retención si `RETENTION_ENABLED=true`. Con el
/// default (`false`) no lanza nada. El wiring debe invocarla con el handle de
/// DB del proceso (p.ej. en `main` tras `connect`); la primera ejecución ocurre
/// pasadas 24 h.
pub async fn spawn_retention_job(db: Database) {
    let cfg = config_from_env();
    if !cfg.enabled {
        tracing::info!(
            raw_days = cfg.raw_days,
            hourly_months = cfg.hourly_months,
            daily_forever = cfg.daily_forever,
            "retention job DISABLED (RETENTION_ENABLED false/ausente) — no se programa"
        );
        return;
    }
    tracing::info!(
        raw_days = cfg.raw_days,
        hourly_months = cfg.hourly_months,
        daily_forever = cfg.daily_forever,
        "retention job ENABLED — planificado cada 24 h"
    );
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(86_400));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        // Descarta el primer tick inmediato: primera corrida real tras 24 h.
        interval.tick().await;
        loop {
            interval.tick().await;
            match run_downsample(&db).await {
                Ok(report) => {
                    tracing::info!(
                        raw_deleted = report.raw_deleted,
                        hourly_upserted = report.hourly_upserted,
                        hourly_deleted = report.hourly_deleted,
                        daily_upserted = report.daily_upserted,
                        "retention job completed"
                    );
                }
                Err(err) => {
                    tracing::error!("retention job failed: {err}");
                }
            }
        }
    });
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use dmart_shared::models::{ApacheIIData, GcsData, Measurement};
    use surrealdb::engine::local::SurrealKv;

    /// DB aislada por test (TempDir), mismo patrón que api_tests.rs.
    async fn test_db() -> (Surreal<Db>, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("retention.db");
        let db = Surreal::new::<SurrealKv>(path.to_str().unwrap())
            .await
            .expect("connect");
        db.use_ns("dmart").use_db("icu").await.expect("ns/db");
        (db, dir)
    }

    /// Construye una medición con valores controlados (el job agrega por scores).
    #[allow(clippy::too_many_arguments)]
    fn measurement(
        patient_id: &str,
        timestamp: &str,
        apache: u32,
        gcs: u32,
        saps3: u32,
        news2: u32,
        sofa: u32,
    ) -> Measurement {
        let mut m = Measurement::new(patient_id, ApacheIIData::default(), GcsData::default());
        m.timestamp = timestamp.to_string();
        m.apache_score = apache;
        m.gcs_score = gcs as u8;
        m.saps3_score = Some(saps3);
        m.news2_score = Some(news2);
        m.sofa_score = Some(sofa);
        m.notas = "PHI: historia clinica del paciente".to_string();
        m
    }

    async fn insert(db: &Surreal<Db>, m: Measurement) {
        crate::db::create_measurement(db, m)
            .await
            .expect("insert measurement");
    }

    fn cfg() -> RetentionConfig {
        RetentionConfig {
            raw_days: 30,
            hourly_months: 6,
            daily_forever: true,
            enabled: false,
        }
    }

    fn rfc(dt: DateTime<Utc>) -> String {
        dt.to_rfc3339()
    }

    /// Ingesta controlada: 2 viejas en la misma hora (90 d), 1 en la hora
    /// siguiente (90 d), 1 ancient (200 d) y 2 recientes (bucket abierto).
    async fn seed(db: &Surreal<Db>) -> (String, String) {
        let now = Utc::now();
        let day90 = (now - ChronoDuration::days(90))
            .with_hour(10)
            .expect("hora")
            .with_minute(0)
            .expect("minuto")
            .with_second(0)
            .expect("segundo")
            .with_nanosecond(0)
            .expect("nanosegundo");

        // Bucket horario A: dos lecturas (10:10 y 10:50).
        insert(
            db,
            measurement(
                "p1",
                &rfc(day90 + ChronoDuration::minutes(10)),
                25,
                10,
                60,
                5,
                4,
            ),
        )
        .await;
        insert(
            db,
            measurement(
                "p1",
                &rfc(day90 + ChronoDuration::minutes(50)),
                35,
                12,
                50,
                6,
                2,
            ),
        )
        .await;
        // Bucket horario B: una lectura (11:05).
        insert(
            db,
            measurement(
                "p1",
                &rfc(day90 + ChronoDuration::hours(1) + ChronoDuration::minutes(5)),
                40,
                8,
                70,
                3,
                7,
            ),
        )
        .await;
        // Bucket ancient (200 d): se agrega y el hourly se purga.
        let ancient = (now - ChronoDuration::days(200))
            .with_hour(15)
            .expect("hora")
            .with_minute(0)
            .expect("minuto")
            .with_second(0)
            .expect("segundo")
            .with_nanosecond(0)
            .expect("nanosegundo");
        insert(db, measurement("p1", &rfc(ancient), 55, 7, 80, 4, 9)).await;
        // Recientes: bucket horario abierto → no se agregan y no se purgan.
        insert(db, measurement("p1", &rfc(now), 10, 15, 30, 0, 0)).await;
        insert(
            db,
            measurement(
                "p1",
                &rfc(now + ChronoDuration::minutes(1)),
                12,
                13,
                32,
                1,
                1,
            ),
        )
        .await;

        let bucket_a_hour = truncate_hour(day90 + ChronoDuration::minutes(10)).to_rfc3339();
        let bucket_day = truncate_day(day90).to_rfc3339();
        (bucket_a_hour, bucket_day)
    }

    #[tokio::test]
    async fn downsample_aggregates_and_purges() {
        let (db, _dir) = test_db().await;
        let (bucket_a, bucket_day) = seed(&db).await;

        let report = run_downsample_with(&db, &cfg()).await.expect("run 1");

        // 3 buckets horarios (A:2, B:1, ancient:1) + 2 diarios (día90, día200).
        assert_eq!(report.hourly_upserted, 3, "buckets horarios upserted");
        assert_eq!(report.daily_upserted, 2, "buckets diarios upserted");
        assert_eq!(report.buckets, 5);
        // Raw purgado: los 4 antiguos (90d×3 + 200d×1); recientes quedan.
        assert_eq!(report.raw_deleted, 4);
        assert_eq!(
            count_table(&db, "measurements").await.unwrap(),
            2,
            "raw recientes conservados"
        );
        // El hourly antiguo (200d > 6 meses) se purga; el de 90d se conserva.
        assert_eq!(report.hourly_deleted, 1);
        assert_eq!(
            count_table(&db, TABLE_HOURLY).await.unwrap(),
            2,
            "hourly 90d conservado"
        );
        // Diarios se conservan siempre (daily_forever).
        assert_eq!(count_table(&db, TABLE_DAILY).await.unwrap(), 2);

        // ── Agregados correctos en el bucket horario A (count=2) ──
        let rows: Vec<serde_json::Value> = db
            .query("SELECT patient_id, timestamp, count, apache_avg, apache_min, apache_max, gcs_avg, gcs_min, gcs_max, saps3_avg, news2_avg, sofa_avg FROM measurements_hourly WHERE patient_id = $p AND timestamp = $ts")
            .bind(("p", "p1"))
            .bind(("ts", bucket_a.clone()))
            .await
            .unwrap()
            .take(0)
            .unwrap();
        assert_eq!(rows.len(), 1, "un solo bucket A");
        let row = &rows[0];
        assert_eq!(row["count"].as_u64(), Some(2));
        approx(row["apache_avg"].as_f64(), 30.0);
        approx(row["apache_min"].as_f64(), 25.0);
        approx(row["apache_max"].as_f64(), 35.0);
        approx(row["gcs_avg"].as_f64(), 11.0);
        approx(row["saps3_avg"].as_f64(), 55.0);
        approx(row["news2_avg"].as_f64(), 5.5);
        approx(row["sofa_avg"].as_f64(), 3.0);
        // PHI no viaja a los agregados.
        assert!(row.get("notas").is_none(), "notas no debe persistirse");
        assert!(
            row.get("apache_data").is_none(),
            "apache_data no debe persistirse"
        );

        // ── Agregados correctos en el bucket diario (count=3) ──
        let rows: Vec<serde_json::Value> = db
            .query("SELECT patient_id, timestamp, count, apache_avg, apache_min, apache_max FROM measurements_daily WHERE patient_id = $p AND timestamp = $ts")
            .bind(("p", "p1"))
            .bind(("ts", bucket_day.clone()))
            .await
            .unwrap()
            .take(0)
            .unwrap();
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(row["count"].as_u64(), Some(3));
        approx(row["apache_avg"].as_f64(), 100.0 / 3.0);
        approx(row["apache_min"].as_f64(), 25.0);
        approx(row["apache_max"].as_f64(), 40.0);
        assert!(row.get("notas").is_none());

        // ── Idempotencia: 2ª corrida no duplica, no borra más ──
        let report2 = run_downsample_with(&db, &cfg()).await.expect("run 2");
        assert_eq!(
            report2.hourly_upserted, 0,
            "no quedan buckets cerrados por agregar"
        );
        assert_eq!(report2.daily_upserted, 0);
        assert_eq!(report2.raw_deleted, 0);
        assert_eq!(report2.hourly_deleted, 0);
        assert_eq!(count_table(&db, TABLE_HOURLY).await.unwrap(), 2);
        assert_eq!(count_table(&db, TABLE_DAILY).await.unwrap(), 2);

        // ── Los agregados sobreviven más allá de raw_days ──
        let still: Vec<serde_json::Value> = db
            .query("SELECT patient_id, timestamp, count FROM measurements_hourly WHERE patient_id = $p AND timestamp = $ts")
            .bind(("p", "p1"))
            .bind(("ts", bucket_a))
            .await
            .unwrap()
            .take(0)
            .unwrap();
        assert_eq!(
            still.len(),
            1,
            "bucket horario conservado aunque el raw fue purgado"
        );
    }

    #[tokio::test]
    async fn upsert_by_bucket_does_not_duplicate() {
        let (db, _dir) = test_db().await;
        // raw_days alto para que ambas lecturas del bucket sobrevivan al purge
        // entre las dos corridas (la 2ª debe recalcular el bucket con AVG de 2).
        let raw_keep = RetentionConfig {
            raw_days: 400,
            hourly_months: 6,
            daily_forever: true,
            enabled: false,
        };
        let now = Utc::now();
        let t = (now - ChronoDuration::days(90))
            .with_hour(10)
            .expect("hora")
            .with_minute(0)
            .expect("minuto")
            .with_second(0)
            .expect("segundo")
            .with_nanosecond(0)
            .expect("nanosegundo");
        let bucket_ts = truncate_hour(t).to_rfc3339();

        insert(
            &db,
            measurement(
                "p2",
                &rfc(t + ChronoDuration::minutes(10)),
                25,
                10,
                60,
                5,
                4,
            ),
        )
        .await;
        let r1 = run_downsample_with(&db, &raw_keep).await.expect("run 1");
        assert_eq!(r1.hourly_upserted, 1);
        let rows: Vec<serde_json::Value> = db
            .query("SELECT patient_id, timestamp, count, apache_avg, apache_min, apache_max FROM measurements_hourly WHERE patient_id = $p AND timestamp = $ts")
            .bind(("p", "p2"))
            .bind(("ts", bucket_ts.clone()))
            .await
            .unwrap()
            .take(0)
            .unwrap();
        assert_eq!(rows.len(), 1, "1 fila tras el primer upsert");
        approx(rows[0]["apache_avg"].as_f64(), 25.0);
        assert_eq!(rows[0]["count"].as_u64(), Some(1));

        // Nueva lectura en el MISMO bucket ya procesado → se recalcula, no duplica.
        insert(
            &db,
            measurement(
                "p2",
                &rfc(t + ChronoDuration::minutes(40)),
                45,
                14,
                70,
                7,
                6,
            ),
        )
        .await;
        let r2 = run_downsample_with(&db, &raw_keep).await.expect("run 2");
        assert_eq!(
            r2.hourly_upserted, 1,
            "solo el bucket actualizado se reprocesa"
        );

        let rows: Vec<serde_json::Value> = db
            .query("SELECT patient_id, timestamp, count, apache_avg, apache_min, apache_max FROM measurements_hourly WHERE patient_id = $p AND timestamp = $ts")
            .bind(("p", "p2"))
            .bind(("ts", bucket_ts.clone()))
            .await
            .unwrap()
            .take(0)
            .unwrap();
        assert_eq!(rows.len(), 1, "update-in-place: sin duplicar el bucket");
        approx(rows[0]["apache_avg"].as_f64(), 35.0);
        assert_eq!(rows[0]["count"].as_u64(), Some(2));
        approx(rows[0]["apache_max"].as_f64(), 45.0);
    }

    #[tokio::test]
    async fn config_defaults_keep_job_disabled() {
        let cfg = config_from_env();
        // Sin overrides de entorno: 7 días raw, 6 meses hourly, job apagado.
        assert_eq!(cfg.raw_days, 7);
        assert_eq!(cfg.hourly_months, 6);
        assert!(cfg.daily_forever);
        assert!(!cfg.enabled);
    }

    #[tokio::test]
    async fn ensure_tables_is_idempotent() {
        let (db, _dir) = test_db().await;
        ensure_tables(&db).await.expect("first");
        ensure_tables(&db).await.expect("second");
        let n = count_table(&db, TABLE_HOURLY).await.unwrap();
        assert_eq!(n, 0);
    }

    fn approx(actual: Option<f64>, expected: f64) {
        let actual = actual.unwrap_or_else(|| panic!("valor ausente (esperaba {expected})"));
        assert!(
            (actual - expected).abs() < 1e-9,
            "esperaba {expected}, obtuve {actual}"
        );
    }
}
