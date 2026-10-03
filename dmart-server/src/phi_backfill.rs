//! Backfill de cifrado de PHI en reposo (SPEC-052 / auditoría H4, P0.2).
//!
//! Cierra el hueco de las filas escritas **antes** de que existiera la columna
//! `phi`: se leen en claro, se sellan con el cifrador del proceso y se vuelven a
//! escribir cifradas, conservando en claro sólo las columnas necesarias para
//! filtrar. El segundo run es un no-op porque el filtro SQL (`phi IS NONE OR
//! phi = ''`) ya no selecciona nada: **la idempotencia la da ese filtro, no
//! `seal_*`**, que re-sella siempre que se le pase una fila.
//!
//! Garantías que este módulo da y que el binario anterior no daba:
//!
//! - **Nunca acepta clave efímera.** [`require_master_key`] se ejecuta *antes*
//!   de tocar el cifrador del proceso; sin `DMART_MASTER_KEY` fuerte el
//!   backfill aborta en vez de sellar con una clave que el servidor no puede
//!   leer (lo que dejaría la PHI ilegible y los índices ciegos inservibles).
//! - **Siempre termina.** La iteración avanza con un cursor sobre la columna
//!   de id y, si el cursor no puede avanzar, se corta la tabla contando el
//!   error. Una fila legacy ilegible ya no produce un bucle infinito: se
//!   contabiliza y se deja para el siguiente run.
//! - **Métricas y exit code.** Devuelve un [`BackfillReport`] por tabla; el
//!   binario traduce eso a exit code != 0 si hubo errores.
//!
//! Sólo se cubren `patients`, `measurements` y `push_subscription`. Las demás
//! tablas con envelope (`camas`, `care_plan`, `audit_logs`, `device_registry`,
//! `reports`) quedan fuera a propósito: ver
//! `docs/compliance/PHI_BACKFILL.md`.

use std::collections::BTreeSet;

use anyhow::{Context, Result, bail};
use serde::Serialize;
use serde_json::Value;
use surrealdb::Surreal;
use surrealdb::engine::local::Db;
use tracing::{info, warn};

use crate::phi_store;

/// Agregado (tabla) a la que aplicar el backfill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Target {
    /// PHI identificable del paciente + índices ciegos de MRN/cédula/nombre.
    Patients,
    /// Signos vitales, notas y datos clínicos de la medición.
    Measurements,
    /// Endpoint y user agent de la suscripción Web Push.
    PushSubscription,
}

impl Target {
    /// Todas las tablas cubiertas, en el orden en que se procesan.
    pub const ALL: [Target; 3] = [
        Target::Patients,
        Target::Measurements,
        Target::PushSubscription,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Target::Patients => "patients",
            Target::Measurements => "measurements",
            Target::PushSubscription => "push_subscription",
        }
    }

    /// Columna por la que se pagina con el cursor y por la que se identifica la
    /// fila en los avisos.
    ///
    /// En `push_subscription` no sirve `patient_id`-style: la tabla no tiene
    /// `tenant_id` y su clave natural es `(user_id, endpoint)`; se pagina por
    /// `endpoint`, que además forma parte del AAD del envelope.
    fn id_field(self) -> &'static str {
        match self {
            Target::Patients => "patient_id",
            Target::Measurements => "measurement_id",
            Target::PushSubscription => "endpoint",
        }
    }

    /// `push_subscription` no tiene `tenant_id` (el AAD usa `user_id`), así que
    /// `--tenant` no se le puede aplicar.
    fn has_tenant(self) -> bool {
        !matches!(self, Target::PushSubscription)
    }
}

/// Resuelve el valor de `--table` a la lista de tablas a procesar.
pub fn selection(table: Option<Target>) -> Vec<Target> {
    match table {
        None => Target::ALL.to_vec(),
        Some(t) => vec![t],
    }
}

/// Parámetros de una ejecución del backfill.
#[derive(Debug, Clone)]
pub struct BackfillConfig {
    pub targets: Vec<Target>,
    pub dry_run: bool,
    pub batch_size: usize,
    /// Limita a un tenant. Se ignora (con aviso) en las tablas sin `tenant_id`.
    pub tenant: Option<String>,
    /// Aborta la ejecución al alcanzar este número de errores. `0` = sin límite.
    pub max_errors: u64,
}

impl Default for BackfillConfig {
    fn default() -> Self {
        Self {
            targets: Target::ALL.to_vec(),
            dry_run: false,
            batch_size: 100,
            tenant: None,
            max_errors: 100,
        }
    }
}

/// Resultado por tabla.
#[derive(Debug, Clone, Default, Serialize)]
pub struct TableReport {
    pub table: String,
    /// Filas sin envelope al empezar la ejecución.
    pub pending: u64,
    /// Filas selladas en esta ejecución.
    pub sealed: u64,
    /// Filas que fallaron y quedan pendientes para el siguiente run.
    pub errors: u64,
    /// Ids de las filas que fallaron, para poder repararlas a mano.
    pub failed_ids: Vec<String>,
}

impl TableReport {
    /// Línea de métricas de una tabla, con los ids que fallaron.
    pub fn summary(&self) -> String {
        let ids = if self.failed_ids.is_empty() {
            String::new()
        } else {
            format!(" ids={}", self.failed_ids.join(","))
        };
        format!(
            "{}: pendientes={} sellados={} errores={}{ids}",
            self.table, self.pending, self.sealed, self.errors
        )
    }
}

/// Resultado agregado de una ejecución.
#[derive(Debug, Clone, Default, Serialize)]
pub struct BackfillReport {
    pub dry_run: bool,
    pub tenant: Option<String>,
    /// `true` si se cortó por `max_errors` antes de recorrer todas las filas.
    pub aborted: bool,
    pub tables: Vec<TableReport>,
}

impl BackfillReport {
    pub fn sealed(&self) -> u64 {
        self.tables.iter().map(|t| t.sealed).sum()
    }

    pub fn errors(&self) -> u64 {
        self.tables.iter().map(|t| t.errors).sum()
    }

    pub fn pending(&self) -> u64 {
        self.tables.iter().map(|t| t.pending).sum()
    }

    /// Sin errores: el binario puede salir con 0.
    pub fn is_clean(&self) -> bool {
        self.errors() == 0
    }

    /// Línea de métricas para el log final.
    pub fn summary(&self) -> String {
        let modo = if self.dry_run { "dry-run" } else { "apply" };
        format!(
            "{modo}: pendientes={} sellados={} errores={} abortado={}",
            self.pending(),
            self.sealed(),
            self.errors(),
            self.aborted
        )
    }
}

/// Falla si `DMART_MASTER_KEY` no está configurada con suficiente entropía.
///
/// Es la misma política de fail-closed que aplica el servidor al arrancar
/// (`main.rs`): si el servidor no arranca con esa clave, el backfill tampoco
/// debe sellar con ella. `PhiCipher::from_env` acepta una clave efímera fuera
/// de producción, y aquí eso sería destructivo — el envelope se firmaría con
/// una clave que desaparece al salir del proceso y la PHI quedaría ilegible
/// para siempre (y los índices ciegos, que también derivan de la clave, no
/// casarían con nada).
pub fn require_master_key() -> Result<()> {
    crate::crypto::validate_master_key()
        .map_err(|e| anyhow::anyhow!("{e}"))
        .with_context(|| {
            "el backfill de PHI se niega a usar clave efímera: sin DMART_MASTER_KEY los envelopes \
         que escribiera quedarían ilegibles"
                .to_string()
        })
}

/// Ejecuta el backfill sobre las tablas de `cfg.targets`.
pub async fn run(db: &Surreal<Db>, cfg: &BackfillConfig) -> Result<BackfillReport> {
    require_master_key()?;
    phi_store::ensure_cipher_initialized();

    let mut report = BackfillReport {
        dry_run: cfg.dry_run,
        tenant: cfg.tenant.clone(),
        aborted: false,
        tables: Vec::new(),
    };

    for target in &cfg.targets {
        if cfg.tenant.is_some() && !target.has_tenant() {
            warn!(
                table = target.name(),
                tenant = cfg.tenant.as_deref().unwrap_or_default(),
                "--tenant no aplica a esta tabla (no tiene tenant_id): se procesa completa"
            );
        }
        let table_report = run_target(db, *target, cfg).await?;
        info!(table = target.name(), "{}", table_report.summary());
        let aborted = table_report.errors >= cfg.max_errors && cfg.max_errors > 0;
        report.tables.push(table_report);
        if aborted {
            report.aborted = true;
            warn!(
                max_errors = cfg.max_errors,
                "abortado por límite de errores; el resto de filas queda pendiente"
            );
            break;
        }
    }

    Ok(report)
}

async fn run_target(db: &Surreal<Db>, target: Target, cfg: &BackfillConfig) -> Result<TableReport> {
    let pending = count_pending(db, target, cfg).await?;
    let mut report = TableReport {
        table: target.name().to_string(),
        pending,
        ..TableReport::default()
    };

    if pending == 0 {
        info!(
            table = target.name(),
            "nada que hacer: todas las filas tienen phi"
        );
        return Ok(report);
    }
    if cfg.dry_run {
        info!(
            table = target.name(),
            pending,
            batch = cfg.batch_size,
            "DRY-RUN: se sellarían estas filas"
        );
        return Ok(report);
    }

    // Cursor sobre la columna de id: garantiza avance aunque una fila falle,
    // porque la siguiente consulta exige `id > cursor` y la que falló ya quedó
    // atrás. `quarantined` es la red de seguridad para las filas sin id
    // utilizable, que el cursor no puede saltar.
    let mut cursor = String::new();
    let mut quarantined: BTreeSet<String> = BTreeSet::new();

    loop {
        let batch = fetch_batch(db, target, cfg, &cursor).await?;
        if batch.is_empty() {
            break;
        }

        let next_cursor = batch
            .iter()
            .filter_map(|row| cursor_value(target, row))
            .max();

        // Sin avance posible el `WHERE id > cursor` devolvería la misma fila
        // para siempre: se corta la tabla contando el error en vez de girar.
        let no_id = batch.iter().all(|row| cursor_value(target, row).is_none());
        let Some(next) = next_cursor.filter(|next| next.as_str() > cursor.as_str()) else {
            warn!(
                table = target.name(),
                batch = batch.len(),
                sin_id = no_id,
                "sin avance del cursor: se abandona la tabla para no reprocesar las mismas filas"
            );
            report.errors += 1;
            report
                .failed_ids
                .push(format!("<cursor bloqueado en lote de {}>", batch.len()));
            return Ok(report);
        };

        for row in &batch {
            let Some(id) = cursor_value(target, row) else {
                report.errors += 1;
                report.failed_ids.push("<sin id>".to_string());
                warn!(
                    table = target.name(),
                    "fila sin columna de id: no se puede actualizar"
                );
                continue;
            };
            if quarantined.contains(&id) {
                continue;
            }
            match seal_row(db, target, row.clone()).await {
                Ok(()) => report.sealed += 1,
                Err(e) => {
                    quarantined.insert(id.clone());
                    report.errors += 1;
                    report.failed_ids.push(id.clone());
                    warn!(table = target.name(), id, error = %format!("{e:#}"), "fila no sellada");
                }
            }
            if cfg.max_errors > 0 && report.errors >= cfg.max_errors {
                report.errors = report.errors.min(cfg.max_errors);
                return Ok(report);
            }
        }

        info!(
            table = target.name(),
            "progreso: {}/{} sellados, {} errores", report.sealed, report.pending, report.errors
        );
        cursor = next;
    }

    Ok(report)
}

/// Valor de la columna de cursor de una fila, si es utilizable.
fn cursor_value(target: Target, row: &Value) -> Option<String> {
    row.get(target.id_field())
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// Filtro SQL común: sólo filas sin envelope, acotadas al tenant si se pidió.
fn pending_filter(cfg: &BackfillConfig, target: Target) -> String {
    let mut sql = String::from("(phi IS NONE OR phi = '') ");
    if target.has_tenant()
        && let Some(_tenant) = &cfg.tenant
    {
        sql.push_str("AND tenant_id = $tenant ");
    }
    sql
}

async fn count_pending(db: &Surreal<Db>, target: Target, cfg: &BackfillConfig) -> Result<u64> {
    // `GROUP ALL` es obligatorio: sin él SurrealQL agrupa por registro y el
    // `count()` devuelve una fila por fila en vez de un total.
    let sql = format!(
        "SELECT count() AS total FROM {} WHERE {} GROUP ALL",
        target.name(),
        pending_filter(cfg, target)
    );
    let mut res = with_tenant(db.query(sql), cfg, target).await?;
    let rows: Vec<Value> = res.take(0)?;
    Ok(rows
        .first()
        .and_then(|r| r.get("total"))
        .and_then(Value::as_u64)
        .unwrap_or(0))
}

async fn fetch_batch(
    db: &Surreal<Db>,
    target: Target,
    cfg: &BackfillConfig,
    cursor: &str,
) -> Result<Vec<Value>> {
    let sql = format!(
        "SELECT * OMIT id FROM {} WHERE {}AND {} > $cursor ORDER BY {} ASC LIMIT $limit",
        target.name(),
        pending_filter(cfg, target),
        target.id_field(),
        target.id_field()
    );
    let mut res = with_tenant(db.query(sql), cfg, target)
        .bind(("cursor", cursor.to_string()))
        .bind(("limit", cfg.batch_size.max(1) as i64))
        .await?;
    let rows: Vec<Value> = res.take(0)?;
    Ok(rows)
}

fn with_tenant<'a>(
    query: surrealdb::method::Query<'a, Db>,
    cfg: &BackfillConfig,
    target: Target,
) -> surrealdb::method::Query<'a, Db> {
    match (target.has_tenant(), &cfg.tenant) {
        (true, Some(tenant)) => query.bind(("tenant", tenant.clone())),
        _ => query,
    }
}

/// Sella una fila legacy y la vuelve a escribir con la columna `phi` poblada.
async fn seal_row(db: &Surreal<Db>, target: Target, row: Value) -> Result<()> {
    match target {
        Target::Patients => {
            let patient = phi_store::open_patient(row)?;
            let sealed = phi_store::seal_patient(&patient)?;
            let id = sealed.patient_id.clone();
            let updated: Option<phi_store::PatientRow> = db
                .update(("patients", id.clone()))
                .content(sealed)
                .await
                .with_context(|| format!("actualizando paciente {id} cifrado"))?;
            if updated.is_none() {
                bail!("no existe el registro patients:{id} (patient_id descuadrado del id)");
            }
        }
        Target::Measurements => {
            let measurement = phi_store::open_measurement(row)?;
            let sealed = phi_store::seal_measurement(&measurement)?;
            let id = sealed.measurement_id.clone();
            let updated: Option<phi_store::MeasurementRow> = db
                .update(("measurements", id.clone()))
                .content(sealed)
                .await
                .with_context(|| format!("actualizando medición {id} cifrada"))?;
            if updated.is_none() {
                bail!("no existe el registro measurements:{id} (measurement_id descuadrado)");
            }
        }
        Target::PushSubscription => {
            let sub = phi_store::open_push_sub(row)?;
            let sealed = phi_store::seal_push_sub(&sub, &sub.user_id)?;
            // La tabla es SCHEMAFULL y sus records tienen id autogenerado que
            // `SELECT * OMIT id` no puede deserializar, así que se localiza la
            // fila por su índice UNIQUE `(user_id, endpoint)` en vez de por id.
            let mut res = db
                .query(
                    "UPSERT push_subscription CONTENT $row WHERE user_id = $uid AND endpoint = $ep",
                )
                .bind(("row", sealed))
                .bind(("uid", sub.user_id.clone()))
                .bind(("ep", sub.endpoint.clone()))
                .await
                .with_context(|| format!("actualizando suscripción push {}", sub.user_id))?;
            check_statement_errors(&mut res, "UPSERT push_subscription")?;
        }
    }
    Ok(())
}

/// `db.query(..).await` no falla aunque una sentencia tenga errores (por ejemplo
/// una violación de `SCHEMAFULL`): hay que inspeccionarlos o el backfill
/// reportaría como "sellada" una fila que en realidad nunca se escribió.
fn check_statement_errors(res: &mut surrealdb::Response, ctx: &str) -> Result<()> {
    let errors = res.take_errors();
    if errors.is_empty() {
        return Ok(());
    }
    let detalle = errors
        .values()
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join("; ");
    bail!("{ctx}: {detalle}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deployment::{EnvGuard, tests_lock};

    const MASTER_KEY: &str = "9e107d9d372bb6826bd81d3542a419d6c3e2d1b0a4f7c8e5a6b3d9f1c0e2a4b6d";

    /// Sin `DMART_MASTER_KEY` el backfill no debe sellar nada: una clave
    /// efímera dejaría la PHI ilegible para el servidor.
    #[test]
    fn refuses_to_run_without_a_persistent_master_key() {
        let _lock = tests_lock();
        for value in [
            None,
            Some(""),
            Some("   "),
            Some("dmart-default-key-change-me"),
            Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        ] {
            let _env = EnvGuard::new(&[("DMART_MASTER_KEY", value)]);
            assert!(
                require_master_key().is_err(),
                "no debe aceptar una clave efímera o débil: {value:?}"
            );
        }
    }

    /// La misma política que el servidor: una clave fuerte sí se acepta.
    #[test]
    fn accepts_a_strong_master_key() {
        let _lock = tests_lock();
        let _env = EnvGuard::new(&[("DMART_MASTER_KEY", Some(MASTER_KEY))]);
        assert!(require_master_key().is_ok());
    }

    /// `--table` decide qué se recorre; sin `--table`, todas.
    #[test]
    fn table_selection_defaults_to_all() {
        assert_eq!(selection(None), Target::ALL.to_vec());
        assert_eq!(
            selection(Some(Target::Measurements)),
            vec![Target::Measurements]
        );
    }

    /// El filtro de pendientes es el que da la idempotencia: sólo toca filas
    /// sin envelope, con o sin tenant.
    #[test]
    fn pending_filter_matches_only_rows_without_envelope() {
        let cfg = BackfillConfig::default();
        let sql = pending_filter(&cfg, Target::Patients);
        assert!(sql.contains("phi IS NONE OR phi = ''"));
        assert!(!sql.contains("tenant_id"));

        let scoped = BackfillConfig {
            tenant: Some("hosp-a".into()),
            ..BackfillConfig::default()
        };
        assert!(pending_filter(&scoped, Target::Patients).contains("tenant_id = $tenant"));
        assert!(!pending_filter(&scoped, Target::PushSubscription).contains("tenant_id"));
    }

    #[test]
    fn report_sums_per_table_metrics() {
        let report = BackfillReport {
            tables: vec![
                TableReport {
                    table: "patients".into(),
                    pending: 3,
                    sealed: 2,
                    errors: 1,
                    failed_ids: vec!["P-9".into()],
                },
                TableReport {
                    table: "measurements".into(),
                    pending: 1,
                    sealed: 1,
                    errors: 0,
                    ..TableReport::default()
                },
            ],
            ..BackfillReport::default()
        };
        assert_eq!(report.sealed(), 3);
        assert_eq!(report.errors(), 1);
        assert_eq!(report.pending(), 4);
        assert!(!report.is_clean());
        assert!(report.summary().contains("errores=1"));
    }

    /// Una fila sin la columna de id no puede paginarse: el cursor no avanza y
    /// el lote se rechaza en vez de reintentarse para siempre.
    #[test]
    fn cursor_value_ignores_rows_without_id() {
        let row = serde_json::json!({ "patient_id": "P-1" });
        assert_eq!(cursor_value(Target::Patients, &row).as_deref(), Some("P-1"));
        assert!(cursor_value(Target::Patients, &serde_json::json!({})).is_none());
        assert!(cursor_value(Target::Patients, &serde_json::json!({ "patient_id": 7 })).is_none());
    }
}
