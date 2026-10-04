//! P0.2 — Backfill del cifrado de PHI en reposo.
//!
//! Para cada tabla cubierta por el backfill (`patients`, `measurements`,
//! `push_subscription`) comprueba el gate del ítem P0.2 del plan:
//! (a) una fila legacy sembrada **en claro** queda cifrada tras el backfill,
//! (b) la segunda ejecución es un no-op (la idempotencia la da el filtro SQL,
//! no `seal_*`), y (c) la fila sigue siendo localizable y legible.
//!
//! También cubre lo que reventaba el binario anterior: una fila legacy
//! ilegible no puede provocar un bucle infinito (B1) y sin `DMART_MASTER_KEY`
//! el backfill no escribe nada (B2).

use std::sync::Once;

use dmart_server::db;
use dmart_server::phi_backfill::{self, BackfillConfig, Target};
use dmart_server::phi_store;
use dmart_shared::models::{ApacheIIData, GcsData, Measurement, Patient};
use serde_json::Value;
use surrealdb::Surreal;
use surrealdb::engine::local::{Db, SurrealKv};

/// Clave maestra fuerte y fija para todo el binario de test.
///
/// `phi_store::cipher()` es un `OnceLock` de proceso: si la primera
/// inicialización cayera en la clave efímera, el envelope sembrado y el
/// verificado usarían subclaves distintas y el test no significaría nada. Se
/// fija **una vez** y siempre al mismo valor, así que el orden de los tests en
/// paralelo no altera el resultado.
const MASTER_KEY: &str = "9e107d9d372bb6826bd81d3542a419d6c3e2d1b0a4f7c8e5a6b3d9f1c0e2a4b6d";

/// Marcador para el test que se re-ejecuta en un subproceso sin clave maestra.
const CHILD_MARKER: &str = "DMART_PHI_BACKFILL_CHILD";
const CHILD_OK: &str = "backfill-rechazado-sin-clave-maestra";

static MASTER_KEY_ONCE: Once = Once::new();

fn init_master_key() {
    MASTER_KEY_ONCE.call_once(|| {
        // SAFETY: el binario de test es el único que fija esta variable y
        // siempre con el mismo valor, así que no hay carrera observable con
        // `phi_store::cipher()`.
        unsafe { std::env::set_var("DMART_MASTER_KEY", MASTER_KEY) };
    });
}

async fn test_db() -> (Surreal<Db>, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Surreal::new::<SurrealKv>(dir.path().join("phi_backfill.db"))
        .await
        .expect("db");
    db.use_ns("dmart").use_db("icu").await.expect("ns");
    dmart_server::migrations::run_migrations(&db)
        .await
        .expect("migrations");
    (db, dir)
}

fn cfg(targets: Vec<Target>) -> BackfillConfig {
    BackfillConfig {
        targets,
        dry_run: false,
        batch_size: 2,
        tenant: None,
        max_errors: 100,
    }
}

/// Inserta la fila como la dejaba el esquema anterior: PHI en columnas en claro.
async fn seed_clear(db: &Surreal<Db>, table: &str, id: &str, row: Value) {
    db.query("CREATE $rec CONTENT $row")
        .bind(("rec", db::sdb_id_pub(table, id)))
        .bind(("row", row))
        .await
        .expect("legacy insert");
}

/// `true` si la fila trae un envelope `phi` no vacío.
fn phi_poblado(row: &Value) -> bool {
    phi_value(row).is_some_and(|s| !s.is_empty())
}

fn phi_value(row: &Value) -> Option<&str> {
    row.get("phi").and_then(Value::as_str)
}

/// Lectura cruda de la tabla, sin pasar por la capa de descifrado.
async fn raw_rows(db: &Surreal<Db>, table: &str) -> Vec<Value> {
    db.query(format!("SELECT * OMIT id FROM {table}"))
        .await
        .expect("query")
        .take(0)
        .expect("take")
}

/// Paciente legacy con `patient_id` == id del record, como los que dejó el
/// esquema anterior (`save_patient` creaba el record con el id público).
fn legacy_patient(id: &str, tenant: &str, hc: &str, nombre: &str) -> Patient {
    let mut p = Patient::new();
    p.patient_id = id.into();
    p.tenant_id = tenant.into();
    p.historia_clinica = hc.into();
    p.cedula = "1098765432".into();
    p.nombre = nombre.into();
    p.apellido = "Prueba".into();
    p.direccion = "Calle 45 #12-30".into();
    p
}

const NOTAS: &str = "Paciente sedado; nota clínica identificable";

/// Medición legacy con `measurement_id` == id del record, como los que dejó el
/// esquema anterior (`create_measurement` creaba el record con ese id).
fn legacy_measurement(measurement_id: &str, tenant: &str, patient_id: &str) -> Measurement {
    let mut m = Measurement::new_for_tenant(patient_id, tenant, vital_data(), gcs_data());
    m.measurement_id = measurement_id.into();
    m.notas = NOTAS.into();
    m
}

fn vital_data() -> ApacheIIData {
    ApacheIIData {
        temperatura: 38.5,
        presion_arterial_media: 71.0,
        presion_sistolica: 96.0,
        frecuencia_cardiaca: 118.0,
        frecuencia_respiratoria: 27.0,
        fio2: 0.45,
        pao2: Some(70.0),
        spo2: 91.0,
        creatinina: 2.4,
        leucocitos: 19.2,
        gcs_ojos: 3,
        gcs_verbal: 4,
        gcs_motor: 5,
        gcs_total: 12,
        edad: 68,
        ..ApacheIIData::default()
    }
}

fn gcs_data() -> GcsData {
    GcsData {
        apertura_ocular: 3,
        respuesta_verbal: 4,
        respuesta_motora: 5,
    }
}

// ─── patients ────────────────────────────────────────────────────────────────

/// (a) fila legacy en claro → cifrada, (b) 2ª ejecución no-op,
/// (c) búsqueda exacta por índice ciego sigue operativa.
#[tokio::test]
async fn patients_legacy_row_is_sealed_and_search_still_works() {
    init_master_key();
    let (db, _dir) = test_db().await;

    seed_clear(
        &db,
        "patients",
        "P-1",
        serde_json::to_value(legacy_patient(
            "P-1",
            "hosp-a",
            "MRN-001",
            "María Fernández",
        ))
        .expect("value"),
    )
    .await;

    // Antes del backfill la PHI está en claro y no hay envelope.
    let antes = raw_rows(&db, "patients").await;
    assert_eq!(antes.len(), 1);
    assert!(
        serde_json::to_string(&antes)
            .expect("json")
            .contains("MRN-001"),
        "la fila debe sembrarse en claro"
    );

    let report = phi_backfill::run(&db, &cfg(vec![Target::Patients]))
        .await
        .expect("run");
    assert_eq!(report.sealed(), 1, "{}", report.summary());
    assert_eq!(report.errors(), 0, "{}", report.summary());

    // (a) La columna `phi` está poblada y la PHI ya no está en claro.
    let despues = raw_rows(&db, "patients").await;
    assert_eq!(despues.len(), 1);
    let crudo = serde_json::to_string(&despues[0]).expect("json");
    assert!(phi_poblado(&despues[0]), "columna phi poblada");
    for secreto in ["María", "Fernández", "MRN-001", "1098765432", "Calle 45"] {
        assert!(
            !crudo.contains(secreto),
            "PHI en claro tras el backfill: {secreto}"
        );
    }

    // El envelope abre con el contexto vigente y conserva todos los campos.
    let abierto = phi_store::open_patient(despues[0].clone()).expect("open");
    assert_eq!(abierto.historia_clinica, "MRN-001");
    assert_eq!(abierto.nombre, "María Fernández");
    assert_eq!(abierto.direccion, "Calle 45 #12-30");
    assert_eq!(abierto.cedula, "1098765432");

    // (c) Búsqueda exacta por índice ciego, insensible al formato del MRN.
    // Ahora ya no depende del fallback a columnas legacy: el fallback exige
    // `phi IS NONE`, así que si el índice ciego no casara, no encontraría nada.
    for variante in ["MRN-001", "mrn 001", "MRN.001"] {
        let found = phi_store::find_patients_by_identifier(&db, "hosp-a", variante, 10)
            .await
            .expect("find");
        assert_eq!(
            found.len(),
            1,
            "variante {variante} debe encontrar al paciente"
        );
        assert_eq!(found[0].patient_id, "P-1");
    }
    assert!(
        phi_store::find_patients_by_identifier(&db, "hosp-a", "MRN-999", 10)
            .await
            .expect("find")
            .is_empty(),
        "un MRN distinto no debe aparecer"
    );
    assert!(
        phi_store::find_patients_by_identifier(&db, "hosp-b", "MRN-001", 10)
            .await
            .expect("find")
            .is_empty(),
        "el índice ciego no debe cruzar tenants"
    );

    // (b) El segundo run es un no-op: 0 pendientes, 0 escrituras.
    let segunda = phi_backfill::run(&db, &cfg(vec![Target::Patients]))
        .await
        .expect("second run");
    assert_eq!(segunda.pending(), 0, "{}", segunda.summary());
    assert_eq!(segunda.sealed(), 0);
    assert!(segunda.is_clean());
    assert_eq!(
        raw_rows(&db, "patients").await.len(),
        1,
        "no debe duplicar filas"
    );
}

/// `--dry-run` cuenta pero no escribe.
#[tokio::test]
async fn dry_run_reports_without_writing() {
    init_master_key();
    let (db, _dir) = test_db().await;
    seed_clear(
        &db,
        "patients",
        "P-1",
        serde_json::to_value(legacy_patient(
            "P-1",
            "hosp-a",
            "MRN-001",
            "María Fernández",
        ))
        .expect("value"),
    )
    .await;

    let report = phi_backfill::run(
        &db,
        &BackfillConfig {
            dry_run: true,
            ..cfg(vec![Target::Patients])
        },
    )
    .await
    .expect("run");

    assert_eq!(report.pending(), 1);
    assert_eq!(report.sealed(), 0);
    assert!(report.dry_run);
    let crudo = serde_json::to_string(&raw_rows(&db, "patients").await[0]).expect("json");
    assert!(crudo.contains("MRN-001"), "el dry-run no debe cifrar nada");
}

// ─── measurements ────────────────────────────────────────────────────────────

/// (a) los signos vitales y las notas legacy en claro pasan al envelope sin
/// perder nada, (b) 2ª ejecución no-op, (c) la fila se rehidrata íntegra.
#[tokio::test]
async fn measurements_legacy_row_is_sealed_without_losing_vitals() {
    init_master_key();
    let (db, _dir) = test_db().await;
    seed_clear(
        &db,
        "measurements",
        "M-1",
        serde_json::to_value(legacy_measurement("M-1", "hosp-a", "P-1")).expect("value"),
    )
    .await;

    let antes = raw_rows(&db, "measurements").await;
    assert!(
        serde_json::to_string(&antes)
            .expect("json")
            .contains("38.5"),
        "los signos vitales deben sembrarse en claro"
    );

    let report = phi_backfill::run(&db, &cfg(vec![Target::Measurements]))
        .await
        .expect("run");
    assert_eq!(report.sealed(), 1, "{}", report.summary());
    assert_eq!(report.errors(), 0, "{}", report.summary());

    // (a) `notas` y los signos vitales ya no están en claro.
    let despues = raw_rows(&db, "measurements").await;
    let crudo = serde_json::to_string(&despues[0]).expect("json");
    assert!(
        !crudo.contains("38.5"),
        "signos vitales en claro tras el backfill"
    );
    assert!(!crudo.contains("sedado"), "notas en claro tras el backfill");
    assert!(phi_poblado(&despues[0]), "columna phi poblada");
    // Scores y fingerprint siguen en claro: son filtros, no PHI.
    assert!(despues[0].get("fingerprint").is_some());
    assert!(despues[0].get("apache_score").is_some());

    // (b)+(c) La medición se rehidrata con todos los valores y el 2º run no hace nada.
    let abierta = phi_store::open_measurement(despues[0].clone()).expect("open");
    assert_eq!(
        abierta.notas, NOTAS,
        "las notas deben viajar en el envelope"
    );
    assert_eq!(abierta.apache_data.temperatura, 38.5);
    assert_eq!(abierta.apache_data.frecuencia_cardiaca, 118.0);
    assert_eq!(abierta.apache_data.pao2, Some(70.0));
    assert_eq!(abierta.apache_data.leucocitos, 19.2);
    assert_eq!(abierta.gcs_data.total(), 12);
    assert_eq!(abierta.patient_id, "P-1");
    assert_eq!(abierta.tenant_id, "hosp-a");

    let segunda = phi_backfill::run(&db, &cfg(vec![Target::Measurements]))
        .await
        .expect("second run");
    assert_eq!(segunda.pending(), 0, "{}", segunda.summary());
    assert_eq!(raw_rows(&db, "measurements").await.len(), 1);
}

/// El camino de escritura normal tampoco deja `notas` en claro.
#[tokio::test]
async fn new_measurements_are_written_with_notas_encrypted() {
    init_master_key();
    let (db, _dir) = test_db().await;
    db::create_measurement(&db, legacy_measurement("M-1", "hosp-a", "P-1"))
        .await
        .expect("create");

    let crudo = serde_json::to_string(&raw_rows(&db, "measurements").await[0]).expect("json");
    assert!(
        !crudo.contains("sedado"),
        "create_measurement no debe dejar notas en claro"
    );
}

// ─── push_subscription ───────────────────────────────────────────────────────

/// (a) endpoint/user_agent legacy pasan al envelope, (b) 2ª ejecución no-op,
/// (c) la suscripción sigue siendo entregable: el endpoint permanece en claro
/// porque sin él no hay POST posible, y `open_push_sub` reconstruye el resto.
#[tokio::test]
async fn push_subscription_legacy_row_is_sealed_and_still_deliverable() {
    init_master_key();
    let (db, _dir) = test_db().await;
    let endpoint = "https://fcm.googleapis.com/fcm/send/abc123";
    let user_agent = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) Safari/604.1";
    db.query(
        "CREATE push_subscription SET user_id = $uid, endpoint = $ep, p256dh = 'pk', \
         auth = 'au', user_agent = $ua, created_at = '2026-01-01T00:00:00Z'",
    )
    .bind(("uid", "u-1".to_string()))
    .bind(("ep", endpoint.to_string()))
    .bind(("ua", user_agent.to_string()))
    .await
    .expect("seed");

    let antes = raw_rows(&db, "push_subscription").await;
    assert_eq!(antes.len(), 1, "la fila debe sembrarse");
    assert!(antes[0].get("phi").is_none(), "legacy: sin envelope");

    let report = phi_backfill::run(&db, &cfg(vec![Target::PushSubscription]))
        .await
        .expect("run");
    assert_eq!(report.sealed(), 1, "{}", report.summary());
    assert_eq!(report.errors(), 0, "{}", report.summary());

    // (a) `phi` poblada y `user_agent` fuera de claro.
    let despues = raw_rows(&db, "push_subscription").await;
    assert_eq!(despues.len(), 1, "no debe duplicar la suscripción");
    let crudo = serde_json::to_string(&despues[0]).expect("json");
    assert!(
        !crudo.contains("iPhone"),
        "user_agent en claro tras el backfill"
    );
    assert!(crudo.contains(endpoint), "el endpoint debe seguir en claro");
    assert!(phi_poblado(&despues[0]), "columna phi poblada");

    // (c) La fila sigue siendo legible y entregable.
    let sub = phi_store::open_push_sub(despues[0].clone()).expect("open");
    assert_eq!(sub.endpoint, endpoint);
    assert_eq!(sub.user_id, "u-1");
    assert_eq!(sub.user_agent.as_deref(), Some(user_agent));
    assert_eq!(sub.p256dh, "pk");

    let segunda = phi_backfill::run(&db, &cfg(vec![Target::PushSubscription]))
        .await
        .expect("second run");
    assert_eq!(segunda.pending(), 0, "{}", segunda.summary());
    assert_eq!(raw_rows(&db, "push_subscription").await.len(), 1);
}

// ─── camas ───────────────────────────────────────────────────────────────────

/// Cama legacy con `paciente_nombre` en claro y `cama_id` == id del record.
fn legacy_cama(cama_id: &str, tenant: &str, paciente_id: &str, nombre: &str) -> Value {
    serde_json::json!({
        "cama_id": cama_id,
        "tenant_id": tenant,
        "numero": 7,
        "tipo": "General",
        "estado": "Ocupada",
        "paciente_id": paciente_id,
        "paciente_nombre": nombre,
        "created_at": "2026-01-01T00:00:00+00:00",
    })
}

/// (a) el nombre del paciente sale del claro, (b) 2ª ejecución no-op,
/// (c) la cama sigue siendo legible con el AAD `(tenant_id, cama_id)`.
#[tokio::test]
async fn camas_legacy_row_is_sealed_and_still_readable() {
    init_master_key();
    let (db, _dir) = test_db().await;
    let nombre = "María Fernández";

    seed_clear(
        &db,
        "camas",
        "C-1",
        legacy_cama("C-1", "hosp-a", "P-1", nombre),
    )
    .await;

    let antes = raw_rows(&db, "camas").await;
    assert_eq!(antes.len(), 1);
    assert!(
        serde_json::to_string(&antes[0])
            .expect("json")
            .contains(nombre),
        "la cama debe sembrarse en claro"
    );

    let report = phi_backfill::run(&db, &cfg(vec![Target::Camas]))
        .await
        .expect("run");
    assert_eq!(report.sealed(), 1, "{}", report.summary());
    assert_eq!(report.errors(), 0, "{}", report.summary());

    // (a) `phi` poblada, nombre fuera del claro, y el resto de la fila intacto.
    let despues = raw_rows(&db, "camas").await;
    assert_eq!(despues.len(), 1, "no debe duplicar camas");
    let crudo = serde_json::to_string(&despues[0]).expect("json");
    assert!(phi_poblado(&despues[0]), "columna phi poblada");
    assert!(!crudo.contains(nombre), "nombre del paciente en claro");
    assert_eq!(
        despues[0]["paciente_id"], "P-1",
        "paciente_id sigue en claro"
    );
    assert_eq!(despues[0]["numero"], 7);
    assert_eq!(despues[0]["estado"], "Ocupada");

    // (c) La cama se reabre con el AAD correcto y devuelve el nombre.
    let cama = phi_store::open_cama(despues[0].clone()).expect("open");
    assert_eq!(cama.paciente_nombre.as_deref(), Some(nombre));
    assert_eq!(cama.cama_id, "C-1");
    assert_eq!(cama.paciente_id.as_deref(), Some("P-1"));

    // El envelope está atado a su fila: con otro tenant no abre.
    let mut otro = despues[0].clone();
    if let Some(obj) = otro.as_object_mut() {
        obj.insert("tenant_id".into(), Value::String("hosp-b".into()));
    }
    assert!(
        phi_store::open_cama(otro).is_err(),
        "el AAD debe impedir abrir la cama desde otro tenant"
    );

    // (b) El segundo run es un no-op.
    let segunda = phi_backfill::run(&db, &cfg(vec![Target::Camas]))
        .await
        .expect("second run");
    assert_eq!(segunda.pending(), 0, "{}", segunda.summary());
    assert_eq!(segunda.sealed(), 0);
    assert!(segunda.is_clean());
    assert_eq!(raw_rows(&db, "camas").await.len(), 1);
}

/// El camino de escritura normal tampoco deja `paciente_nombre` en claro: el
/// alta de un paciente con cama asignada sella ambas filas en una transacción.
#[tokio::test]
async fn new_camas_are_written_with_patient_name_encrypted() {
    init_master_key();
    let (db, _dir) = test_db().await;
    db::init_camas(&db, 1, dmart_shared::models::TipoCama::General)
        .await
        .expect("init_camas");

    let cama_id = db::get_cama_libre(&db)
        .await
        .expect("get_cama_libre")
        .expect("cama libre")
        .cama_id;

    let mut paciente = legacy_patient("P-NUEVO", "hosp-a", "HC-999", "María");
    paciente.apellido = "Fernández".into();
    paciente.cama_id = Some(cama_id);
    paciente.cama_numero = Some(1);
    db::create_patient_with_assignments(&db, paciente, &[])
        .await
        .expect("alta con cama");

    let crudo = serde_json::to_string(&raw_rows(&db, "camas").await[0]).expect("json");
    assert!(
        !crudo.contains("María"),
        "el alta no debe dejar el nombre del paciente en claro"
    );
    assert!(crudo.contains("\"phi\""), "debe escribir el envelope");
}

// ─── B1: avance garantizado ──────────────────────────────────────────────────

/// Una fila legacy ilegible no puede dejar el backfill girando: se cuenta como
/// error, se deja para el siguiente run y el proceso termina.
#[tokio::test]
async fn unreadable_legacy_row_does_not_loop_and_is_reported() {
    init_master_key();
    let (db, _dir) = test_db().await;

    // Sin las claves de identidad que `open_legacy_patient` exige, la fila es
    // ilegible: sellarla produciría un paciente vacío con datos perdidos.
    seed_clear(
        &db,
        "patients",
        "P-BROTO",
        serde_json::json!({ "patient_id": "P-BROTO", "tenant_id": "hosp-a" }),
    )
    .await;
    for (id, n) in [("P-A", 1), ("P-C", 2)] {
        let p = legacy_patient(id, "hosp-a", &format!("MRN-{n:03}"), "Paciente Sano");
        seed_clear(&db, "patients", id, serde_json::to_value(p).expect("value")).await;
    }

    let report = phi_backfill::run(
        &db,
        &BackfillConfig {
            max_errors: 10,
            ..cfg(vec![Target::Patients])
        },
    )
    .await
    .expect("run");

    assert_eq!(report.sealed(), 2, "las filas sanas sí se sellan");
    assert_eq!(report.errors(), 1, "{}", report.summary());
    assert!(
        report.tables[0].failed_ids.contains(&"P-BROTO".to_string()),
        "el id ilegible debe aparecer en el reporte: {:?}",
        report.tables[0].failed_ids
    );
    assert!(!report.is_clean(), "con errores el binario debe salir != 0");

    // La fila rota sigue sin cifrar (no se ha tocado) y las sanas sí.
    let rows = raw_rows(&db, "patients").await;
    let roto = rows
        .iter()
        .find(|r| r["patient_id"] == "P-BROTO")
        .expect("fila rota");
    assert!(roto.get("phi").is_none());
    let sano = rows
        .iter()
        .find(|r| r["patient_id"] == "P-A")
        .expect("fila sana");
    assert!(phi_poblado(sano), "la fila sana debe quedar cifrada");

    // Un segundo run reintenta sólo la que queda, y vuelve a fallar sin girar.
    let segunda = phi_backfill::run(&db, &cfg(vec![Target::Patients]))
        .await
        .expect("second run");
    assert_eq!(segunda.pending(), 1);
    assert_eq!(segunda.errors(), 1);
}

/// `--max-errors` corta la ejecución antes de recorrerlo todo.
#[tokio::test]
async fn max_errors_stops_the_run_and_reports_aborted() {
    init_master_key();
    let (db, _dir) = test_db().await;
    for id in ["P-1", "P-2", "P-3", "P-4"] {
        seed_clear(
            &db,
            "patients",
            id,
            serde_json::json!({ "patient_id": id, "tenant_id": "hosp-a" }),
        )
        .await;
    }

    let report = phi_backfill::run(
        &db,
        &BackfillConfig {
            max_errors: 2,
            batch_size: 2,
            ..cfg(vec![Target::Patients])
        },
    )
    .await
    .expect("run");

    assert!(report.aborted, "debe marcarse abortado");
    assert_eq!(report.sealed(), 0);
    assert_eq!(report.errors(), 2);
    assert_eq!(report.pending(), 4, "quedan filas para la siguiente pasada");
}

// ─── B2: nunca clave efímera ─────────────────────────────────────────────────

/// Sin `DMART_MASTER_KEY` el backfill aborta sin escribir nada. Es el bug que
/// dejaba la PHI de `patients` ilegible para el servidor.
///
/// El cifrador del proceso es un `OnceLock` global, así que la comprobación se
/// hace en un subproceso launched desde aquí (mismo binario de test, sin la
/// variable en el entorno).
#[tokio::test]
async fn refuses_to_run_without_master_key() {
    // El marker se comprueba ANTES de `init_master_key()`: si no, el hijo
    // volvería a fijar la clave y no probaría nada.
    if std::env::var(CHILD_MARKER).is_ok() {
        child_sin_clave_maestra().await;
        return;
    }
    init_master_key();

    let exe = std::env::current_exe().expect("ruta del test exe");
    let output = std::process::Command::new(exe)
        .args(["--exact", "refuses_to_run_without_master_key"])
        .args(["--nocapture", "--test-threads=1"])
        .env(CHILD_MARKER, "1")
        .env_remove("DMART_MASTER_KEY")
        .output()
        .expect("ejecutar el test hijo");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success() && stdout.contains(CHILD_OK),
        "sin DMART_MASTER_KEY el backfill debe negarse a escribir.\nstatus={}\nstdout={}\nstderr={}",
        output.status,
        stdout,
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Comprobación real, sólo en el subproceso sin clave maestra.
async fn child_sin_clave_maestra() {
    assert!(
        std::env::var("DMART_MASTER_KEY").is_err(),
        "el padre debe quitar DMART_MASTER_KEY"
    );
    assert!(phi_backfill::require_master_key().is_err());

    let (db, _dir) = test_db().await;
    seed_clear(
        &db,
        "patients",
        "P-1",
        serde_json::to_value(legacy_patient(
            "P-1",
            "hosp-a",
            "MRN-001",
            "María Fernández",
        ))
        .expect("value"),
    )
    .await;

    let result = phi_backfill::run(&db, &cfg(vec![Target::Patients])).await;
    assert!(result.is_err(), "run() debe fallar sin clave maestra");

    let crudo = serde_json::to_string(&raw_rows(&db, "patients").await[0]).expect("json");
    assert!(
        crudo.contains("MRN-001") && !crudo.contains("\"phi\""),
        "no debe haberse cifrado nada"
    );
    println!("{CHILD_OK}");
}

// ─── --table y --tenant ───────────────────────────────────────────────────────

/// `--table` acota el trabajo: `measurements` no toca `patients`.
#[tokio::test]
async fn table_selection_scopes_the_work() {
    init_master_key();
    let (db, _dir) = test_db().await;
    seed_clear(
        &db,
        "patients",
        "P-1",
        serde_json::to_value(legacy_patient("P-1", "hosp-a", "MRN-001", "María")).expect("value"),
    )
    .await;
    seed_clear(
        &db,
        "measurements",
        "M-1",
        serde_json::to_value(legacy_measurement("M-1", "hosp-a", "P-1")).expect("value"),
    )
    .await;

    let report = phi_backfill::run(&db, &cfg(vec![Target::Measurements]))
        .await
        .expect("run");
    assert_eq!(report.sealed(), 1);
    assert!(
        raw_rows(&db, "patients").await[0].get("phi").is_none(),
        "patients debe quedar intacta"
    );

    // `all` (sin `--table`) recoge lo que falte.
    let report = phi_backfill::run(&db, &cfg(phi_backfill::selection(None)))
        .await
        .expect("run");
    assert_eq!(report.sealed(), 1, "{}", report.summary());
    assert!(phi_poblado(&raw_rows(&db, "patients").await[0]));
}

/// `--tenant` acota a un hospital.
#[tokio::test]
async fn tenant_filter_scopes_the_work() {
    init_master_key();
    let (db, _dir) = test_db().await;
    for (id, tenant) in [("P-A", "hosp-a"), ("P-B", "hosp-b")] {
        let p = legacy_patient(id, tenant, &format!("MRN-{tenant}"), "Paciente");
        seed_clear(&db, "patients", id, serde_json::to_value(p).expect("value")).await;
    }

    let report = phi_backfill::run(
        &db,
        &BackfillConfig {
            tenant: Some("hosp-a".into()),
            ..cfg(vec![Target::Patients])
        },
    )
    .await
    .expect("run");
    assert_eq!(report.sealed(), 1, "{}", report.summary());

    let rows = raw_rows(&db, "patients").await;
    let a = rows.iter().find(|r| r["patient_id"] == "P-A").expect("P-A");
    let b = rows.iter().find(|r| r["patient_id"] == "P-B").expect("P-B");
    assert!(phi_poblado(a), "hosp-a debe quedar cifrado");
    assert!(b.get("phi").is_none(), "hosp-b no debe tocarse");
}
