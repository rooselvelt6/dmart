//! Backfill del índice de trigramas ciegos (`patients.bi_tng`).
//!
//! `backfill_phi` sólo toca filas con `phi IS NONE` (PHI legacy sin cifrar), así
//! que **no** re-sella las filas ya cifradas que se sellaron antes de que
//! existiera el índice de trigramas. Este binario cubre ese hueco: recorre los
//! pacientes, reabre el envelope, re-sella y actualiza sólo las filas cuyo
//! `bi_tng` falta o no coincide con lo que produce el código actual.
//!
//! Es idempotente (recalcular da el mismo resultado) y se salta por
//! comparación la escritura de las filas ya correctas, de modo que repetirlo no
//! reescribe la base entera.

use std::sync::Arc;

use anyhow::{Context, Result, bail};
use clap::Parser;
use dmart_server::db;
use dmart_server::phi_store::{self, PatientRow};
use surrealdb::Surreal;
use surrealdb::engine::local::Db;

const EXIT_OK: i32 = 0;
const EXIT_FALLO: i32 = 3;

#[derive(Parser, Debug)]
#[command(
    name = "backfill_trigrams",
    about = "Rellena patients.bi_tng (HMAC de trigramas) para la búsqueda parcial"
)]
struct Args {
    #[arg(long, help = "Solo informa de cuántas filas faltarían, sin escribir")]
    dry_run: bool,

    #[arg(
        long,
        help = "Ruta SurrealKV. Si se omite usa DMART_DB_PATH y, en su defecto, ./data/dmart.db"
    )]
    db_path: Option<String>,

    #[arg(long, help = "Limita el backfill a un tenant concreto")]
    tenant: Option<String>,
}

fn resolve_db_path(cli: Option<String>) -> String {
    cli.or_else(|| std::env::var("DMART_DB_PATH").ok())
        .unwrap_or_else(|| {
            std::env::current_dir()
                .map(|base| base.join("data/dmart.db").to_string_lossy().into_owned())
                .unwrap_or_else(|_| "./data/dmart.db".to_string())
        })
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();

    // El servidor carga el `.env` antes de leer variables; sin esto, una
    // `DMART_MASTER_KEY` que sólo existe en `.env` parecería ausente y los
    // trigramas se firmarían con la clave efímera, inservibles al reiniciar.
    dotenvy::dotenv().ok();

    let db_path = resolve_db_path(args.db_path.clone());
    tracing::info!(
        dry_run = args.dry_run,
        db_path,
        tenant = args.tenant.as_deref().unwrap_or("<todos>"),
        "backfill_trigrams"
    );

    let db: Arc<Surreal<Db>> = match db::connect(&db_path).await {
        Ok(db) => db,
        Err(e) => {
            tracing::error!(error = %format!("{e:#}"), "no se pudo abrir la base");
            std::process::exit(EXIT_FALLO);
        }
    };

    match run(&db, args.dry_run, args.tenant.as_deref()).await {
        Ok((escritas, sin_cambios)) => {
            tracing::info!(
                escritas,
                sin_cambios,
                dry_run = args.dry_run,
                "backfill_trigrams completado"
            );
            std::process::exit(EXIT_OK);
        }
        Err(e) => {
            tracing::error!(error = %format!("{e:#}"), "backfill_trigrams abortado");
            std::process::exit(EXIT_FALLO);
        }
    }
}

/// Recorre los pacientes y rellena `bi_tng` donde falte.
///
/// Devuelve `(filas escritas, filas ya correctas)`.
async fn run(db: &Surreal<Db>, dry_run: bool, tenant: Option<&str>) -> Result<(u64, u64)> {
    phi_store::ensure_cipher_initialized();

    let mut sql = String::from("SELECT * OMIT id FROM patients WHERE phi IS NOT NONE");
    if tenant.is_some() {
        sql.push_str(" AND tenant_id = $tenant");
    }
    // Por `patient_id`, que es la clave con la que se actualiza la fila. El
    // orden estable hace que dos ejecuciones recorran lo mismo.
    sql.push_str(" ORDER BY patient_id");

    let mut query = db.query(sql);
    if let Some(t) = tenant {
        query = query.bind(("tenant", t.to_string()));
    }
    let rows: Vec<serde_json::Value> = query
        .await
        .context("leyendo pacientes para el backfill de trigramas")?
        .take(0)?;

    let mut escritas = 0u64;
    let mut sin_cambios = 0u64;

    for row in rows {
        // `bi_tng` se lee de la fila en claro **antes** de consumirla: el sobre
        // PHI va por debajo y estas columnas siguen siendo legibles.
        let actual: Option<Vec<String>> = row
            .get("bi_tng")
            .and_then(|v| serde_json::from_value(v.clone()).ok());

        let patient = phi_store::open_patient(row)?;
        let sellado = phi_store::seal_patient(&patient)?;

        // Se re-sella siempre (el sobre AES-GCM tiene nonce aleatorio, así que
        // el ciphertext cambia en cada ejecución), pero sólo se **escribe** si
        // los índices ciegos derivados difieren de los guardados. Sin esta
        // comparación, cada corrida reescribiría la tabla completa.
        if actual.as_deref() == Some(sellado.bi_tng.as_slice()) {
            sin_cambios += 1;
            continue;
        }

        if dry_run {
            escritas += 1;
            continue;
        }

        let id = sellado.patient_id.clone();
        let updated: Option<PatientRow> = db
            .update(("patients", id.clone()))
            .content(sellado)
            .await
            .with_context(|| format!("actualizando paciente {id} con bi_tng"))?;
        if updated.is_none() {
            bail!("no existe el registro patients:{id} (patient_id descuadrado del id)");
        }
        escritas += 1;
    }

    Ok((escritas, sin_cambios))
}
