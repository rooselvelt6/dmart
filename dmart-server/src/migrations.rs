//! Migraciones de esquema reproducibles.
//!
//! Cada cambio de esquema vive en `migrations/00N_nombre.surql` (SQL semi-
//! embarrado en tiempo de compilación vía `include_str!`, de modo que el
//! binario es autosuficiente y los tests usan el mismo catálogo). Las
//! migraciones se aplican en orden al arranque y quedan registradas en la
//! tabla `schema_migrations`, por lo que el proceso es idempotente.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use surrealdb::Surreal;
use surrealdb::engine::local::Db;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaMigration {
    pub version: u64,
    pub name: String,
    pub applied_at: String,
}

const MIGRATIONS: &[(&str, &str)] = &[
    (
        "001_baseline_indexes",
        include_str!("../migrations/001_baseline_indexes.surql"),
    ),
    (
        "002_mfa_users_indexes",
        include_str!("../migrations/002_mfa_users_indexes.surql"),
    ),
    (
        "003_refresh_tokens",
        include_str!("../migrations/003_refresh_tokens.surql"),
    ),
    (
        "015_patient_events",
        include_str!("../migrations/015_patient_events.surql"),
    ),
    (
        "016_cds_plans",
        include_str!("../migrations/016_cds_plans.surql"),
    ),
    (
        "017_device_registry",
        include_str!("../migrations/017_device_registry.surql"),
    ),
    (
        "018_data_quality",
        include_str!("../migrations/018_data_quality.surql"),
    ),
    (
        "019_alert_escalation",
        include_str!("../migrations/019_alert_escalation.surql"),
    ),
    (
        "020_teleicu",
        include_str!("../migrations/020_teleicu.surql"),
    ),
    (
        "029_measurement_fingerprint",
        include_str!("../migrations/029_measurement_fingerprint.surql"),
    ),
    (
        "030_measurements_downsample",
        include_str!("../migrations/030_measurements_downsample.surql"),
    ),
];

pub async fn applied_versions(db: &Surreal<Db>) -> Result<Vec<u64>> {
    let versions: Vec<serde_json::Value> = db
        .query("SELECT version FROM schema_migrations")
        .await?
        .take(0)?;
    Ok(versions
        .into_iter()
        .filter_map(|v| v.get("version").and_then(|x| x.as_u64()))
        .collect())
}

/// Aplica las migraciones pendientes (en orden de versión) y devuelve los
/// nombres de las que se ejecutaron.
pub async fn run_migrations(db: &Surreal<Db>) -> Result<Vec<String>> {
    let applied = applied_versions(db).await?;
    let mut ran = Vec::new();

    for (idx, (name, sql)) in MIGRATIONS.iter().enumerate() {
        let version = (idx + 1) as u64;
        if applied.contains(&version) {
            continue;
        }
        db.query(*sql).await?;
        let record = SchemaMigration {
            version,
            name: name.to_string(),
            applied_at: chrono::Utc::now().to_rfc3339(),
        };
        let _: Option<SchemaMigration> = db
            .create(("schema_migrations", version.to_string()))
            .content(record)
            .await?;
        ran.push(name.to_string());
    }

    Ok(ran)
}

#[cfg(test)]
mod tests {
    use super::*;
    use surrealdb::engine::local::SurrealKv;

    async fn test_db() -> (Surreal<Db>, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("mig.db");
        let db = Surreal::new::<SurrealKv>(path.to_str().unwrap())
            .await
            .expect("connect");
        db.use_ns("dmart").use_db("icu").await.expect("ns");
        (db, dir)
    }

    #[tokio::test]
    async fn runs_only_pending_migrations() {
        let (db, _dir) = test_db().await;
        let first = run_migrations(&db).await.expect("first run");
        assert_eq!(first.len(), MIGRATIONS.len());
        let mut applied = applied_versions(&db).await.expect("applied");
        applied.sort_unstable();
        assert_eq!(applied, vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]);

        let second = run_migrations(&db).await.expect("second run");
        assert!(second.is_empty(), "no pending migrations after first run");
    }

    #[tokio::test]
    async fn baseline_indices_are_created() {
        let (db, _dir) = test_db().await;
        run_migrations(&db).await.expect("migrate");
        let indices: Vec<serde_json::Value> = db
            .query("INFO FOR TABLE patients")
            .await
            .expect("info")
            .take(0)
            .expect("take");
        let json = serde_json::to_string(&indices).expect("json");
        assert!(
            json.contains("idx_patients_created_at"),
            "index not defined: {}",
            json
        );
    }
}
