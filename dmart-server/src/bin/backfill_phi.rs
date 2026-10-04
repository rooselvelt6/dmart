//! Backfill idempotente del cifrado de PHI en reposo (SPEC-052 / P0.2).
//!
//! Capa fina de CLI: parseo de flags, conexión y exit code. Toda la lógica
//! (filtro de filas legacy, cursor, métricas y política de clave) vive en
//! [`dmart_server::phi_backfill`], que es lo que ejercitan los tests.
//!
//! Uso:
//!   cargo run -p dmart-server --bin backfill_phi -- --dry-run
//!   cargo run -p dmart-server --bin backfill_phi -- --table patients --tenant hosp-a
//!   cargo run -p dmart-server --bin backfill_phi
//!
//! Sale con 0 si no hubo errores, 1 si alguna fila quedó sin cifrar, 2 si se
//! cortó por `--max-errors` y 3 si el backfill no pudo ni empezar (falta
//! `DMART_MASTER_KEY`, base ilegible, error de conexión).

use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use dmart_server::db;
use dmart_server::phi_backfill::{self, BackfillConfig, Target};
use surrealdb::Surreal;
use surrealdb::engine::local::Db;

const EXIT_OK: i32 = 0;
const EXIT_ERRORES: i32 = 1;
const EXIT_ABORTADO: i32 = 2;
const EXIT_FALLO: i32 = 3;

#[derive(Parser, Debug)]
#[command(
    name = "backfill_phi",
    about = "Cifra la PHI legacy que quedó en claro (patients, measurements, push_subscription)"
)]
struct Args {
    #[arg(long, help = "Solo cuenta las filas pendientes, sin modificar nada")]
    dry_run: bool,

    #[arg(long, default_value_t = 100, help = "Tamaño de lote")]
    batch_size: usize,

    #[arg(
        long,
        help = "Ruta SurrealKV. Si se omite usa DMART_DB_PATH y, en su defecto, ./data/dmart.db"
    )]
    db_path: Option<String>,

    #[arg(
        long,
        help = "Limita el backfill a un tenant (no aplica a push_subscription)"
    )]
    tenant: Option<String>,

    #[arg(
        long,
        value_enum,
        default_value = "all",
        help = "Tabla a procesar: patients | measurements | push_subscription | camas | all"
    )]
    table: SelectionArg,

    #[arg(
        long,
        default_value_t = 100,
        help = "Corta la ejecución al llegar a este número de errores (0 = sin límite)"
    )]
    max_errors: u64,
}

/// `--table patients|measurements|push_subscription|camas|all`.
///
/// El enum vive en la lib (para que los tests puedan usar la misma definición),
/// así que aquí se reexporta el valor por defecto y se mapea el caso `all` a
/// la ausencia de filtro.
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
enum SelectionArg {
    Patients,
    Measurements,
    PushSubscription,
    Camas,
    All,
}

impl From<SelectionArg> for Option<Target> {
    fn from(value: SelectionArg) -> Self {
        match value {
            SelectionArg::Patients => Some(Target::Patients),
            SelectionArg::Measurements => Some(Target::Measurements),
            SelectionArg::PushSubscription => Some(Target::PushSubscription),
            SelectionArg::Camas => Some(Target::Camas),
            SelectionArg::All => None,
        }
    }
}

/// Misma precedencia que el servidor (`main.rs`): flag explícito, `DMART_DB_PATH`
/// y, por último, la ruta por defecto relativa al directorio de trabajo.
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

    // El servidor carga el `.env` antes de leer cualquier variable; sin esto,
    // un `DMART_MASTER_KEY` que sólo existe en `.env` parecería ausente y el
    // backfill sellaría con clave efímera (PHI ilegible al primer reinicio).
    dotenvy::dotenv().ok();

    let db_path = resolve_db_path(args.db_path.clone());
    let cfg = BackfillConfig {
        targets: phi_backfill::selection(Option::<Target>::from(args.table)),
        dry_run: args.dry_run,
        batch_size: args.batch_size.max(1),
        tenant: args.tenant.clone(),
        max_errors: args.max_errors,
    };

    tracing::info!(
        dry_run = cfg.dry_run,
        batch_size = cfg.batch_size,
        db_path,
        tenant = cfg.tenant.as_deref().unwrap_or("<todos>"),
        tables = ?cfg.targets.iter().map(|t| t.name()).collect::<Vec<_>>(),
        "backfill_phi"
    );

    let db: Arc<Surreal<Db>> = match db::connect(&db_path).await {
        Ok(db) => db,
        Err(e) => {
            tracing::error!(error = %format!("{e:#}"), "no se pudo abrir la base");
            std::process::exit(EXIT_FALLO);
        }
    };
    let report = match phi_backfill::run(&db, &cfg).await {
        Ok(report) => report,
        Err(e) => {
            // Fallo duro: incluye "falta DMART_MASTER_KEY", que es el caso en el
            // que no se debe escribir nada. `main` devolvería `Err` y el runtime
            // lo mapearía a un código ambiguo, así que se sale explícitamente.
            tracing::error!(error = %format!("{e:#}"), "backfill abortado sin escribir");
            std::process::exit(EXIT_FALLO);
        }
    };

    if let Ok(json) = serde_json::to_string(&report) {
        tracing::info!(report = json, "métricas del backfill");
    }
    for table in &report.tables {
        println!("{}", table.summary());
    }
    println!("{}", report.summary());

    let code = if report.aborted {
        EXIT_ABORTADO
    } else if report.is_clean() {
        EXIT_OK
    } else {
        EXIT_ERRORES
    };
    std::process::exit(code);
}
