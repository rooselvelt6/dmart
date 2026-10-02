//! Backfill idempotente para cifrar filas legacy de `patients` (PHI).
//!
//! Uso:
//!   cargo run --bin backfill_phi_patients -- --dry-run    # solo muestra stats
//!   cargo run --bin backfill_phi_patients                 # ejecuta backfill real
//!
//! Procesa en lotes de 100, re-sella cada paciente con `seal_patient`
//! (que ya maneja idempotencia: si ya tiene `phi`, lo re-cifra).

use anyhow::{Context, Result};
use clap::Parser;
use dmart_server::{db, phi_store};
use serde_json::Value;
use std::sync::Arc;
use surrealdb::Surreal;
use surrealdb::engine::local::Db;
use tracing::{info, warn};

#[derive(Parser, Debug)]
#[command(name = "backfill_phi_patients", about = "Cifra PHI legacy en patients")]
struct Args {
    #[arg(long, help = "Solo muestra cuántas filas faltan, sin modificar")]
    dry_run: bool,

    #[arg(long, default_value = "100", help = "Tamaño de lote")]
    batch_size: usize,

    #[arg(long, default_value = "./data/dmart.db", help = "Ruta SurrealKV")]
    db_path: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();
    info!(
        "backfill_phi_patients: dry_run={}, batch_size={}, db_path={}",
        args.dry_run, args.batch_size, args.db_path
    );

    let db: Arc<Surreal<Db>> = db::connect(&args.db_path).await?;
    phi_store::ensure_cipher_initialized();

    // Cuenta total de pacientes sin phi (legacy)
    let total_legacy: u64 = {
        let mut res = db
            .query("SELECT count() as c FROM patients WHERE phi IS NONE OR phi = '' GROUP BY c")
            .await?;
        let rows: Vec<Value> = res.take(0)?;
        rows.first().and_then(|v| v["c"].as_u64()).unwrap_or(0)
    };

    info!("Pacientes legacy (sin phi): {}", total_legacy);
    if total_legacy == 0 {
        info!("Nada que hacer: todas las filas ya tienen phi");
        return Ok(());
    }

    if args.dry_run {
        info!(
            "DRY-RUN: se procesarían {} pacientes en lotes de {}",
            total_legacy, args.batch_size
        );
        return Ok(());
    }

    let mut processed = 0u64;
    let mut errors = 0u64;

    loop {
        // Toma un lote de pacientes sin phi
        let rows: Vec<serde_json::Value> = db
            .query("SELECT * OMIT id FROM patients WHERE phi IS NONE OR phi = '' LIMIT $limit")
            .bind(("limit", args.batch_size as i64))
            .await?
            .take(0)?;

        if rows.is_empty() {
            break;
        }

        for row in rows {
            let patient_id = row
                .get("patient_id")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();

            // Deserializa el paciente legacy (open_patient maneja filas sin phi)
            match phi_store::open_patient(row) {
                Ok(patient) => {
                    // Re-sella (escribe phi + índices ciegos)
                    let sealed_row = match phi_store::seal_patient(&patient) {
                        Ok(r) => r,
                        Err(e) => {
                            warn!("patient {}: seal falló: {}", patient_id, e);
                            errors += 1;
                            continue;
                        }
                    };

                    // Actualiza en BD
                    let _: Option<phi_store::PatientRow> = db
                        .update(("patients", patient_id.clone()))
                        .content(sealed_row)
                        .await
                        .context("actualizando paciente cifrado")?;

                    processed += 1;
                }
                Err(e) => {
                    warn!("patient {}: open falló: {}", patient_id, e);
                    errors += 1;
                }
            }
        }

        info!(
            "Progreso: {}/{} ({} errores)",
            processed, total_legacy, errors
        );
    }

    info!(
        "Backfill completado: {} procesados, {} errores",
        processed, errors
    );
    Ok(())
}
