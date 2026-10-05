//! P1.3 — Rotación de claves de PHI sin downtime.
//!
//! Comprueba las cuatro propiedades de las que depende el procedimiento de
//! rotación (`docs/compliance/KEY_ROTATION.md`):
//!
//! 1. **Escritura con la nueva, lectura con la antigua**: al desplegar el
//!    keyring A+B (`k2026-03` activa) lo escrito con A sigue abriéndose, y lo
//!    nuevo se etiqueta con la clave activa. Éste es el estado en el que se
//!    está casi toda la vida.
//! 2. **Un `kid` sin clave es un error explícito**, no una fila corrupta: si se
//!    retira A antes de re-cifrar, el error dice qué clave falta. Nunca se
//!    prueban las demás claves a ciegas.
//! 3. **Re-cifrar es idempotente y no toca los índices ciegos**: tras
//!    `reencrypt`, la PHI queda bajo la clave activa, el MRN se sigue
//!    encontrando, y una segunda pasada no hace nada.
//! 4. **El envelope v1 (sin `kid`) sigue leyéndose** contra la clave
//!    `default`, que es la de `DMART_MASTER_KEY`: por eso desplegar P1.3 no
//!    necesita migración ni parar el servicio.
//!
//! El keyring del proceso se fija **una vez** y es siempre el mismo
//! (`default`=A, `k2026-03`=B, B activa), porque `phi_store::cipher()` es un
//! `OnceLock`: si dos tests lo inicializaran con claves distintas, el resultado
//! dependería del orden de ejecución.

use std::collections::HashMap;
use std::sync::Once;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use dmart_server::crypto::{
    CryptoError, LEGACY_KEY_ID, MasterKey, PhiCipher, PhiContext, RotatingKeyProvider,
    StaticKeyProvider, parse_keyring, phi_envelope_key_id, phi_envelope_payload,
};
use dmart_server::db;
use dmart_server::phi_backfill::{self, ReencryptConfig, Target};
use dmart_server::phi_store;
use dmart_shared::models::Patient;
use serde_json::Value;
use surrealdb::Surreal;
use surrealdb::engine::local::{Db, SurrealKv};

/// `kid` de la clave nueva: el que se usa en todo el binario.
const KID_NUEVO: &str = "k2026-03";
/// `kid` de la clave retirada (la de `DMART_MASTER_KEY`).
const KID_VIEJO: &str = LEGACY_KEY_ID;

/// Clave maestra A, la que habría escrito las filas antes de rotar.
///
/// 32 bytes fijos y distintos de B: en la práctica saldrían de
/// `openssl rand -base64 32` y de la bóveda de secretos.
fn raw_a() -> [u8; 32] {
    let mut k = [0u8; 32];
    for (i, b) in k.iter_mut().enumerate() {
        *b = 0xA0 ^ (i as u8);
    }
    k
}

/// Clave maestra B, la nueva.
fn raw_b() -> [u8; 32] {
    let mut k = [0u8; 32];
    for (i, b) in k.iter_mut().enumerate() {
        *b = 0x5C ^ (i as u8).wrapping_mul(3);
    }
    k
}

static KEYRING: Once = Once::new();

/// Fija el keyring del proceso antes de que nadie toque `phi_store::cipher()`.
fn init_keyring() {
    KEYRING.call_once(|| {
        // SAFETY: el binario de test es el único que fija estas variables y
        // siempre con los mismos valores, así que no hay carrera observable
        // con la inicialización del `OnceLock` de `phi_store`.
        unsafe {
            std::env::remove_var("DMART_MASTER_KEY");
            std::env::set_var(
                "DMART_MASTER_KEYS",
                format!(
                    "{KID_VIEJO}={},{KID_NUEVO}={}",
                    B64.encode(raw_a()),
                    B64.encode(raw_b())
                ),
            );
            std::env::set_var("DMART_ACTIVE_KEY_ID", KID_NUEVO);
        }
    });
}

/// Cifrador con una sola clave.
fn cipher_with(kid: &str, raw: [u8; 32]) -> PhiCipher {
    PhiCipher::with_provider(std::sync::Arc::new(StaticKeyProvider::new(
        kid,
        MasterKey::from_raw_bytes(raw),
    )))
    .expect("cifrador de una clave")
}

/// Cifrador con el keyring de rotación.
fn cipher_with_keyring(kids: &[(&str, [u8; 32])], active: &str) -> PhiCipher {
    let map: HashMap<String, Vec<u8>> = kids
        .iter()
        .map(|(kid, raw)| ((*kid).to_string(), raw.to_vec()))
        .collect();
    let provider = RotatingKeyProvider::from_map(map, active).expect("keyring válido");
    PhiCipher::with_provider(std::sync::Arc::new(provider)).expect("cifrador del keyring")
}

/// El keyring del proceso bajo prueba: A y B, con B activa.
fn cipher_proceso() -> PhiCipher {
    cipher_with_keyring(&[(KID_VIEJO, raw_a()), (KID_NUEVO, raw_b())], KID_NUEVO)
}

fn ctx(patient_id: &str) -> PhiContext {
    PhiContext::new("hosp-a", "patient", patient_id)
}

fn paciente(id: &str, mrn: &str) -> Patient {
    let mut p = Patient::new();
    p.patient_id = id.into();
    p.tenant_id = "hosp-a".into();
    p.historia_clinica = mrn.into();
    p.cedula = format!("109876543{}", id);
    p.nombre = "María".into();
    p.apellido = "Fernández".into();
    p.direccion = "Calle 45 #12-30".into();
    p
}

async fn test_db() -> (Surreal<Db>, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Surreal::new::<SurrealKv>(dir.path().join("key_rotation.db"))
        .await
        .expect("db");
    db.use_ns("dmart").use_db("icu").await.expect("ns");
    dmart_server::migrations::run_migrations(&db)
        .await
        .expect("migrations");
    (db, dir)
}

/// Sella la fila del paciente con una clave concreta.
///
/// Simula "esto se escribió antes de la rotación": mismas columnas en claro y
/// mismos índices ciegos (que no dependen de la clave de cifrado), pero el
/// envelope etiquetado con el `kid` indicado.
fn fila_paciente_con_clave(patient: &Patient, key: &[u8; 32], kid: &str) -> Value {
    let mut fila = phi_store::seal_patient(patient).expect("sellar");
    let cipher = cipher_with(kid, *key);
    fila.phi = cipher
        .seal(&ctx(&patient.patient_id), patient)
        .expect("sellar con la clave antigua");
    serde_json::to_value(fila).expect("valor")
}

async fn seed_patient(db: &Surreal<Db>, id: &str, mrn: &str, key: &[u8; 32], kid: &str) {
    db.query("CREATE $rec CONTENT $row")
        .bind(("rec", db::sdb_id_pub("patients", id)))
        .bind(("row", fila_paciente_con_clave(&paciente(id, mrn), key, kid)))
        .await
        .expect("sembrar");
}

async fn phi_de(db: &Surreal<Db>, id: &str) -> String {
    let mut res = db
        .query("SELECT phi FROM $id")
        .bind(("id", db::sdb_id_pub("patients", id)))
        .await
        .expect("query");
    let rows: Vec<Value> = res.take(0).expect("take");
    rows.first()
        .and_then(|r| r.get("phi"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

// ─── 1. El keyring del proceso ───────────────────────────────────────────────

/// El despliegue con `DMART_MASTER_KEYS` usa el keyring de rotación, y el
/// `kid` activo es el de `DMART_ACTIVE_KEY_ID`.
#[test]
fn process_keyring_comes_from_the_environment() {
    init_keyring();
    let provider = dmart_server::crypto::key_provider_from_env().expect("keyring del entorno");
    assert_eq!(provider.active_key_id(), KID_NUEVO);
    assert_eq!(provider.key_ids(), vec!["default", KID_NUEVO]);
    assert!(provider.key_by_id(KID_VIEJO).is_some());
    assert!(provider.key_by_id(KID_NUEVO).is_some());
    assert!(provider.key_by_id("k1999-01").is_none());
}

/// Un `kid` de más de una clave sin `DMART_ACTIVE_KEY_ID` es un error de
/// arranque, no una adivinanza: escribir PHI con la clave equivocada deja
/// filas que este mismo despliegue no puede leer.
#[test]
fn active_key_is_mandatory_with_more_than_one_key() {
    let map = HashMap::from([
        ("a".to_string(), raw_a().to_vec()),
        ("b".to_string(), raw_b().to_vec()),
    ]);
    let err = RotatingKeyProvider::from_map(map.clone(), "").expect_err("debe exigir la activa");
    assert!(
        matches!(&err, CryptoError::KeyConfiguration(m) if m.contains("DMART_ACTIVE_KEY_ID")),
        "mensaje poco accionable: {err}"
    );

    let err = RotatingKeyProvider::from_map(map, "c").expect_err("la activa debe existir");
    assert!(
        matches!(&err, CryptoError::KeyConfiguration(m) if m.contains("no está en")),
        "mensaje poco accionable: {err}"
    );

    // Con una sola clave se puede omitir.
    let solo = HashMap::from([("unica".to_string(), raw_a().to_vec())]);
    let p = RotatingKeyProvider::from_map(solo, "").expect("una clave no necesita activa");
    assert_eq!(p.active_key_id(), "unica");
}

/// El parseo del keyring es estricto: base64 de exactamente 32 bytes y `kid`
/// sin punto (el `kid` va dentro del envelope, separado por puntos).
#[test]
fn keyring_parsing_is_strict() {
    let a = B64.encode(raw_a());
    let b = B64.encode(raw_b());
    let parsed = parse_keyring(&format!("default={a},k2026-03={b}")).expect("keyring válido");
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed["default"], raw_a().to_vec());
    assert_eq!(parsed["k2026-03"], raw_b().to_vec());

    // El padding `=` del base64 sobrevive al reparto por el primer `=`, que es
    // justo lo que rompería una implementación que partiese por el último `=`.
    let con_padding = B64.encode(raw_a());
    assert!(
        con_padding.ends_with('='),
        "32 bytes exigen padding en base64"
    );
    assert_eq!(
        parse_keyring(&format!("k2026-03={con_padding}")).expect("con padding")["k2026-03"],
        raw_a().to_vec(),
        "el padding no debe comerse parte de la clave"
    );

    // Se admiten mayúsculas, dígitos, `-` y `_`; el resto no.
    assert!(parse_keyring(&format!("K2026_Q1={a}")).is_ok());
    assert!(parse_keyring(&format!("k2026 01={a}")).is_err());

    assert!(parse_keyring("").is_err(), "keyring vacío");
    assert!(parse_keyring("sin_igual").is_err(), "entrada sin `=`");
    assert!(
        parse_keyring("k.2026=AAAA").is_err(),
        "un kid con punto haría ambiguo el parseo del envelope"
    );
    assert!(
        parse_keyring(&format!("k2026={}", B64.encode([0u8; 31]))).is_err(),
        "una clave que no son 32 bytes debe rechazarse"
    );
    assert!(
        parse_keyring(&format!("k2026={}", B64.encode([0u8; 33]))).is_err(),
        "una clave de 33 bytes también: sobra entropía, no es un truncamiento"
    );
    assert!(parse_keyring(&format!("k2026={a}")).is_ok());
}

// ─── 2. Envelopes v2 con `kid` ───────────────────────────────────────────────

/// El estado real tras desplegar la rotación: lo escrito con A se abre con el
/// keyring A+B, y lo nuevo se etiqueta con la clave activa.
#[test]
fn rotation_reads_the_old_key_and_writes_with_the_new_one() {
    let viejo = cipher_with(KID_VIEJO, raw_a());
    let rotando = cipher_proceso();
    let p = paciente("P-1", "MRN-001");

    let con_clave_vieja = viejo.seal(&ctx("P-1"), &p).expect("sellar con A");
    assert_eq!(phi_envelope_key_id(&con_clave_vieja), Some(KID_VIEJO));

    // Lo escrito con A se lee sin tocar nada: eso es el "sin downtime".
    let leido = rotando.open::<Patient>(&ctx("P-1"), &con_clave_vieja);
    assert_eq!(
        leido.expect("A debe seguir legible").historia_clinica,
        "MRN-001"
    );

    // Y lo nuevo sale ya con la clave activa.
    let con_clave_nueva = rotando.seal(&ctx("P-1"), &p).expect("sellar con B");
    assert_eq!(phi_envelope_key_id(&con_clave_nueva), Some(KID_NUEVO));

    // Retirada de A: B no puede abrir lo que se escribió con A, y el error lo
    // dice. No se prueba ninguna otra clave a ciegas.
    let solo_nueva = cipher_with_keyring(&[(KID_NUEVO, raw_b())], KID_NUEVO);
    match solo_nueva.open::<Patient>(&ctx("P-1"), &con_clave_vieja) {
        Err(CryptoError::UnknownKeyId(kid)) => assert_eq!(kid, KID_VIEJO),
        other => panic!("se esperaba UnknownKeyId(default), llegó {other:?}"),
    }

    // El mismo envelope sí abre cuando la clave está en el keyring, y el
    // envelope nuevo también: el error era la clave, no la fila.
    let solo_nueva = cipher_with_keyring(&[(KID_VIEJO, raw_a()), (KID_NUEVO, raw_b())], KID_NUEVO);
    assert_eq!(
        solo_nueva
            .open::<Patient>(&ctx("P-1"), &con_clave_nueva)
            .expect("B abre lo suyo")
            .patient_id,
        "P-1"
    );
}

/// La clave que abre un envelope es la que **declara**, no la que esté activa:
/// si no, con la activa podría devolver el `phi` de otro paciente.
#[test]
fn envelope_is_bound_to_its_own_key() {
    let con_a = cipher_with(KID_VIEJO, raw_a())
        .seal(&ctx("P-1"), &paciente("P-1", "MRN-001"))
        .expect("sellar");
    let con_b = cipher_proceso()
        .seal(&ctx("P-1"), &paciente("P-1", "MRN-999"))
        .expect("sellar");

    // Ambos son registros del mismo paciente escritos con distinta clave: cada
    // uno abre con la suya, y el AAD es el que impide cruzarlos.
    let rotando = cipher_proceso();
    assert_eq!(
        rotando
            .open::<Patient>(&ctx("P-1"), &con_a)
            .expect("abre con A")
            .historia_clinica,
        "MRN-001"
    );
    assert_eq!(
        rotando
            .open::<Patient>(&ctx("P-1"), &con_b)
            .expect("abre con B")
            .historia_clinica,
        "MRN-999"
    );
}

/// El envelope v1 (base64 a secas, sin `kid`) se resuelve contra `default`: es
/// lo que hace que la instalación existente siga legible sin tocar una fila.
#[test]
fn legacy_envelope_without_kid_resolves_to_the_default_key() {
    let con_a = cipher_with(KID_VIEJO, raw_a())
        .seal(&ctx("P-1"), &paciente("P-1", "MRN-001"))
        .expect("sellar");
    // Así era el envelope antes de P1.3: el mismo base64, sin prefijo.
    let v1 = phi_envelope_payload(&con_a).to_string();
    assert!(!v1.starts_with("v2."));
    assert_eq!(phi_envelope_key_id(&v1), None);

    let rotando = cipher_proceso();
    assert_eq!(
        rotando
            .open::<Patient>(&ctx("P-1"), &v1)
            .expect("un envelope v1 debe seguir leyéndose")
            .historia_clinica,
        "MRN-001"
    );

    // Sin la clave `default` el error nombra `default`, que es lo que hay que
    // restaurar en el keyring.
    let sin_default = cipher_with_keyring(&[(KID_NUEVO, raw_b())], KID_NUEVO);
    match sin_default.open::<Patient>(&ctx("P-1"), &v1) {
        Err(CryptoError::UnknownKeyId(kid)) => assert_eq!(kid, KID_VIEJO),
        other => panic!("se esperaba UnknownKeyId(default), llegó {other:?}"),
    }
}

/// La clave de índices ciegos **no** rota con la de cifrado.
///
/// Si rotara, todas las búsquedas por MRN/cédula devolverían cero resultados
/// sin ningún error visible: es el fallo silencioso más caro de una rotación.
#[test]
fn blind_indexes_survive_crypto_rotation() {
    let antes = cipher_with(KID_VIEJO, raw_a());
    let rotando = cipher_proceso();
    let otro_activo = cipher_with_keyring(&[(KID_VIEJO, raw_a()), (KID_NUEVO, raw_b())], KID_VIEJO);

    for c in [&antes, &rotando, &otro_activo] {
        assert_eq!(
            c.index_key_id(),
            KID_VIEJO,
            "los índices ciegos deben seguir anclados a `default`"
        );
        assert_eq!(
            c.blind_index("hosp-a", "historia_clinica", "MRN-001"),
            antes.blind_index("hosp-a", "historia_clinica", "MRN-001"),
            "el índice no puede depender de la clave activa"
        );
    }

    // Sin `default` en el keyring se ancla al primer `kid` por orden alfabético,
    // que es estable mientras no se borren kids anteriores.
    let anclado = cipher_with_keyring(&[("k2026-03", raw_b()), ("k2026-04", raw_a())], "k2026-04");
    assert_eq!(anclado.index_key_id(), "k2026-03");
    assert_eq!(
        anclado.blind_index("hosp-a", "historia_clinica", "MRN-001"),
        cipher_with("k2026-03", raw_b()).blind_index("hosp-a", "historia_clinica", "MRN-001")
    );
}

// ─── 3. Re-cifrado en base de datos ──────────────────────────────────────────

/// El job de re-cifrado mueve la PHI a la clave activa, deja el MRN buscable y
/// es idempotente. Es el paso 3 del runbook.
#[tokio::test]
async fn reencrypt_moves_rows_to_the_active_key_and_is_idempotent() {
    init_keyring();
    let (db, _dir) = test_db().await;

    // Dos filas escritas con A (una de ellas con el esquema v1, sin `kid`) y
    // una ya escrita con B, que el job no debe tocar.
    seed_patient(&db, "P-VIEJO-1", "MRN-001", &raw_a(), KID_VIEJO).await;
    seed_patient(&db, "P-VIEJO-2", "MRN-002", &raw_a(), KID_VIEJO).await;
    seed_patient(&db, "P-NUEVO", "MRN-003", &raw_b(), KID_NUEVO).await;
    // La segunda, como la dejaría el esquema anterior: el mismo base64 sin el
    // prefijo `v2.default.`.
    let v1 = phi_envelope_payload(&phi_de(&db, "P-VIEJO-2").await).to_string();
    db.query("UPDATE $rec SET phi = $phi")
        .bind(("rec", db::sdb_id_pub("patients", "P-VIEJO-2")))
        .bind(("phi", v1))
        .await
        .expect("degradar a v1");

    assert_eq!(
        phi_envelope_key_id(&phi_de(&db, "P-VIEJO-1").await),
        Some(KID_VIEJO)
    );
    assert_eq!(
        phi_envelope_key_id(&phi_de(&db, "P-VIEJO-2").await),
        None,
        "esta fila debe sembrarse como v1"
    );

    let cfg = ReencryptConfig {
        targets: vec![Target::Patients],
        dry_run: false,
        batch_size: 10,
        tenant: Some("hosp-a".into()),
        max_errors: 100,
        expected_key_id: Some(KID_NUEVO.into()),
    };

    let report = phi_backfill::reencrypt(&db, &cfg).await.expect("re-cifrar");
    assert_eq!(report.pending(), 2, "{}", report.summary());
    assert_eq!(report.resealed(), 2, "{}", report.summary());
    assert_eq!(report.errors(), 0, "{}", report.summary());
    assert!(report.is_clean());
    assert!(!report.aborted);
    assert_eq!(report.active_key_id, KID_NUEVO);

    // Las tres filas quedan bajo la clave activa y siguen legibles por la API.
    let esperadas = [
        ("P-VIEJO-1", "MRN-001"),
        ("P-VIEJO-2", "MRN-002"),
        ("P-NUEVO", "MRN-003"),
    ];
    for (id, mrn) in esperadas {
        assert_eq!(
            phi_envelope_key_id(&phi_de(&db, id).await),
            Some(KID_NUEVO),
            "{id} debe quedar bajo la clave activa"
        );
        let p = db::get_patient(&db, id)
            .await
            .expect("leer")
            .unwrap_or_else(|| panic!("{id} debe seguir existiendo"));
        assert_eq!(p.historia_clinica, mrn);
    }

    // El MRN se sigue encontrando: los índices ciegos no cambiaron.
    for (mrn, id) in [("MRN-001", "P-VIEJO-1"), ("MRN-002", "P-VIEJO-2")] {
        let found = phi_store::find_patients_by_identifier(&db, "hosp-a", mrn, 10)
            .await
            .expect("buscar");
        assert_eq!(found.len(), 1, "{mrn} debe seguir encontrable");
        assert_eq!(found[0].patient_id, id);
    }

    // Segunda pasada: nada que hacer.
    let segunda = phi_backfill::reencrypt(&db, &cfg).await.expect("2ª pasada");
    assert_eq!(segunda.pending(), 0, "{}", segunda.summary());
    assert_eq!(segunda.resealed(), 0, "{}", segunda.summary());
    assert!(segunda.is_clean());
}

/// `--dry-run` cuenta lo que movería sin escribir, y `expected_key_id` es la
/// guardia contra ejecutar el job con el `DMART_ACTIVE_KEY_ID` equivocado.
#[tokio::test]
async fn reencrypt_dry_run_does_not_write_and_checks_the_expected_key() {
    init_keyring();
    let (db, _dir) = test_db().await;
    seed_patient(&db, "P-VIEJO-1", "MRN-001", &raw_a(), KID_VIEJO).await;

    let dry = ReencryptConfig {
        targets: vec![Target::Patients],
        dry_run: true,
        expected_key_id: Some(KID_NUEVO.into()),
        ..ReencryptConfig::default()
    };
    let report = phi_backfill::reencrypt(&db, &dry).await.expect("dry-run");
    assert_eq!(report.pending(), 1, "{}", report.summary());
    assert_eq!(report.resealed(), 0);
    assert_eq!(
        phi_envelope_key_id(&phi_de(&db, "P-VIEJO-1").await),
        Some(KID_VIEJO),
        "el dry-run no debe escribir"
    );

    // Si el operador cree que la activa es otra, el job aborta sin escribir en
    // lugar de "terminar" sin haber movido nada.
    let erroneo = ReencryptConfig {
        dry_run: false,
        expected_key_id: Some("k2026-99".into()),
        ..ReencryptConfig::default()
    };
    let err = phi_backfill::reencrypt(&db, &erroneo)
        .await
        .expect_err("debe abortar");
    assert!(
        format!("{err:#}").contains(KID_NUEVO),
        "el error debe decir cuál es la clave activa de verdad: {err:#}"
    );
    assert_eq!(
        phi_envelope_key_id(&phi_de(&db, "P-VIEJO-1").await),
        Some(KID_VIEJO),
        "no debe haberse escrito nada"
    );
}

/// Paso 4 del runbook: una vez re-cifrado, la clave vieja se puede retirar del
/// keyring y las filas siguen abriéndose con la nueva.
#[tokio::test]
async fn rows_resealed_with_the_new_key_survive_retiring_the_old_one() {
    init_keyring();
    let (db, _dir) = test_db().await;
    seed_patient(&db, "P-1", "MRN-001", &raw_a(), KID_VIEJO).await;

    phi_backfill::reencrypt(&db, &ReencryptConfig::default())
        .await
        .expect("re-cifrar");
    let sellado = phi_de(&db, "P-1").await;

    // Un despliegue que ya no tiene la clave A en el keyring: es exactamente lo
    // que se hace al borrarla de `DMART_MASTER_KEYS` tras la rotación.
    let solo_nueva = cipher_with_keyring(&[(KID_NUEVO, raw_b())], KID_NUEVO);
    let abierto = solo_nueva
        .open::<Patient>(&ctx("P-1"), &sellado)
        .expect("tras re-cifrar, A ya no hace falta");
    assert_eq!(abierto.historia_clinica, "MRN-001");
    assert_eq!(
        phi_envelope_key_id(&sellado),
        Some(KID_NUEVO),
        "el envelope debe declarar la clave activa"
    );
}

/// Sin `DMART_MASTER_KEYS` ni `DMART_MASTER_KEY` el job se niega a escribir,
/// igual que el backfill: una clave efímera dejaría la PHI ilegible.
#[tokio::test]
async fn reencrypt_refuses_without_a_persistent_key() {
    let marker = "DMART_KEY_ROTATION_CHILD";
    if std::env::var(marker).is_ok() {
        assert!(
            phi_backfill::require_master_key().is_err(),
            "el padre debe quitar las variables de clave"
        );
        return;
    }
    init_keyring();

    let exe = std::env::current_exe().expect("ruta del test exe");
    let output = std::process::Command::new(exe)
        .args(["--exact", "reencrypt_refuses_without_a_persistent_key"])
        .args(["--nocapture", "--test-threads=1"])
        .env(marker, "1")
        .env_remove("DMART_MASTER_KEY")
        .env_remove("DMART_MASTER_KEYS")
        .env_remove("DMART_ACTIVE_KEY_ID")
        .output()
        .expect("ejecutar el test hijo");
    assert!(
        output.status.success(),
        "sin clave persistente el job debe negarse a escribir.\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
