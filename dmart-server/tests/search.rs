use dmart_server::phi_store;
/// Búsqueda parcial de pacientes (SPEC-052 + trigramas ciegos).
///
/// Cubre el fallo que motivó el cambio: `nombre ~ $q` no era posible con
/// la PHI cifrada, así que "gust" no encontraba a nadie y el apellido no
/// estaba indexado. Estos tests fijan ese comportamiento contra la base real
/// (SurrealKV en disco), no contra mocks: el punto a probar es que la consulta
/// SurrealQL con `bi_tng CONTAINS` devuelve lo que debe.
use dmart_shared::models::{Patient, Sexo};
use surrealdb::Surreal;
use surrealdb::engine::local::Db;

async fn test_db() -> (Surreal<Db>, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("search.db");
    let db = Surreal::new::<surrealdb::engine::local::SurrealKv>(path.to_str().expect("path"))
        .await
        .expect("connect");
    db.use_ns("dmart").use_db("icu").await.expect("ns/db");
    dmart_server::migrations::run_migrations(&db)
        .await
        .expect("migrate");
    (db, dir)
}

/// Fragmentos de filtro que aplica `EstadoFilter` a los pacientes.
///
/// Se replican aquí en vez de llamar a `EstadoFilter::sql()`, que es privado:
/// el punto del test es el índice de trigramas, no re-verificar el filtro de
/// estado, que ya tiene cobertura propia.
const FILTRO_TODOS: &str = "";
const FILTRO_ACTIVOS: &str = " AND (fecha_egreso_uci IS NONE OR fecha_egreso_uci = '')";

/// Paciente con id único pero nombre/apellido controlados por el test.
fn paciente(id: &str, nombre: &str, apellido: &str, hc: &str) -> Patient {
    let mut p = Patient::new();
    p.patient_id = id.to_string();
    p.tenant_id = "hosp-a".to_string();
    p.nombre = nombre.to_string();
    p.apellido = apellido.to_string();
    p.historia_clinica = hc.to_string();
    p.cedula = format!("CC{id}");
    p.sexo = Sexo::Masculino;
    p
}

async fn crear(db: &Surreal<Db>, p: Patient) {
    dmart_server::db::create_patient(db, p)
        .await
        .expect("create");
}

/// Crea un paciente **con cama asignada**, que es lo que exige `egresar_paciente`
/// para hacer su trabajo: sin `cama_id` el egreso sale por `return Ok(())`.
async fn crear_con_cama(db: &Surreal<Db>, p: Patient) {
    let cama = dmart_shared::models::Cama {
        cama_id: format!("C-{}", p.patient_id),
        numero: 1,
        estado: dmart_shared::models::EstadoCama::Libre,
        ..Default::default()
    };
    let cama_id = cama.cama_id.clone();
    dmart_server::db::create_cama(db, cama)
        .await
        .expect("create cama");
    let mut p = p;
    p.cama_id = Some(cama_id);
    dmart_server::db::create_patient_with_assignments(db, p, &[])
        .await
        .expect("create con cama");
}

/// Nombres ordenados, para poder afirmar el ranking sin depender de ids.
fn nombres(rows: Vec<Patient>) -> Vec<String> {
    rows.into_iter().map(|p| p.nombre).collect()
}

#[tokio::test]
async fn busca_por_apellido_que_antes_no_estaba_indexado() {
    let (db, _dir) = test_db().await;
    crear(&db, paciente("P-1", "Gustavo", "Ortiz", "HC-1")).await;
    crear(&db, paciente("P-2", "María", "Fernández", "HC-2")).await;

    let r = phi_store::search_patients_fuzzy(&db, "hosp-a", "ortiz", FILTRO_TODOS, 50, 0)
        .await
        .expect("search");

    assert_eq!(nombres(r), vec!["Gustavo"], "el apellido debe ser buscable");
}

#[tokio::test]
async fn busca_por_nombre_completo_con_espacio() {
    let (db, _dir) = test_db().await;
    crear(&db, paciente("P-1", "Gustavo", "Ortiz", "HC-1")).await;
    crear(&db, paciente("P-2", "Yeimy", "Ortiz", "HC-2")).await;

    // El caso que fallaba en producción: "Gustavo Ortiz" devolvía 0.
    let r = phi_store::search_patients_fuzzy(&db, "hosp-a", "Gustavo Ortiz", FILTRO_TODOS, 50, 0)
        .await
        .expect("search");

    assert_eq!(
        nombres(r),
        vec!["Gustavo"],
        "el nombre completo debe filtrar por el paciente correcto"
    );
}

#[tokio::test]
async fn prefijos_cortos_de_tres_letras_encuentran() {
    let (db, _dir) = test_db().await;
    crear(&db, paciente("P-1", "Gustavo", "Ortiz", "HC-1")).await;
    crear(&db, paciente("P-2", "Rosa", "Lindqvist", "HC-2")).await;

    for (q, esperado) in [("gus", "Gustavo"), ("ort", "Gustavo"), ("ros", "Rosa")] {
        let r = phi_store::search_patients_fuzzy(&db, "hosp-a", q, FILTRO_TODOS, 50, 0)
            .await
            .expect("search");
        assert_eq!(nombres(r), vec![esperado], "q={q}");
    }
}

#[tokio::test]
async fn tolera_erratas_de_teclado() {
    let (db, _dir) = test_db().await;
    crear(&db, paciente("P-1", "Gustavo", "Ortiz", "HC-1")).await;

    // Erratas a partir de la 3.ª letra: el trigrama inicial sigue presente, así
    // que el filtro deja pasar al candidato y el ranking lo recupera.
    // "gtsavo" (errata ya en el trigrama filtro) NO entra: es un límite
    // conocido, ver `search::trigram_filtro`.
    for q in ["gustxavo", "gusatvo", "gustavp"] {
        let r = phi_store::search_patients_fuzzy(&db, "hosp-a", q, FILTRO_TODOS, 50, 0)
            .await
            .expect("search");
        assert_eq!(nombres(r), vec!["Gustavo"], "q={q} debería tolerar errata");
    }
}

#[tokio::test]
async fn ignora_tildes_y_mayusculas() {
    let (db, _dir) = test_db().await;
    crear(&db, paciente("P-1", "José", "Muñoz", "HC-1")).await;

    for q in ["jose", "JOSE", "munoz", "MuNoZ", "José", "Muñoz"] {
        let r = phi_store::search_patients_fuzzy(&db, "hosp-a", q, FILTRO_TODOS, 50, 0)
            .await
            .expect("search");
        assert_eq!(nombres(r), vec!["José"], "q={q}");
    }
}

#[tokio::test]
async fn historia_clinica_se_busca_exacta_aunque_no_haya_trigrama() {
    let (db, _dir) = test_db().await;
    crear(&db, paciente("P-1", "Gustavo", "Ortiz", "HC-100516")).await;
    crear(&db, paciente("P-2", "Rosa", "Lindqvist", "HC-100S16")).await;

    // Menos de 3 caracteres útiles en el prefijo: cae al índice exacto de
    // `bi_hc`, que debe resolver el código sin truncarse.
    let r = phi_store::search_patients_fuzzy(&db, "hosp-a", "HC-100516", FILTRO_TODOS, 50, 0)
        .await
        .expect("search");

    assert_eq!(
        nombres(r),
        vec!["Gustavo"],
        "un código no debe casar con otro parecido"
    );
}

#[tokio::test]
async fn el_ranking_ordena_por_similitud() {
    let (db, _dir) = test_db().await;
    // Todos contienen "mar", pero sólo uno empieza por ahí.
    crear(&db, paciente("P-1", "Marcelo", "Ríos", "HC-1")).await;
    crear(&db, paciente("P-2", "Carla", "Márquez", "HC-2")).await;
    crear(&db, paciente("P-3", "Wilmar", "Soto", "HC-3")).await;

    let r = phi_store::search_patients_fuzzy(&db, "hosp-a", "mar", FILTRO_TODOS, 50, 0)
        .await
        .expect("search");

    // Orden por puntuación:
    //  1. "Marcelo": "mar" es prefijo del nombre de pila.
    //  2. "Carla": "mar" es prefijo del apellido "Márquez" y la similitud es
    //     casi exacta, así que gana a "Wilmar".
    //  3. "Wilmar": sólo aparece dentro del nombre, sin ser prefijo.
    let n = nombres(r);
    assert_eq!(
        n,
        vec!["Marcelo", "Carla", "Wilmar"],
        "orden por puntuación"
    );
}

#[tokio::test]
async fn pagina_sobre_el_conjunto_ordenado() {
    let (db, _dir) = test_db().await;
    crear(&db, paciente("P-1", "Marcelo", "Ríos", "HC-1")).await;
    crear(&db, paciente("P-2", "Carla", "Márquez", "HC-2")).await;
    crear(&db, paciente("P-3", "Wilmar", "Soto", "HC-3")).await;

    let sql = FILTRO_TODOS;
    let p0 = phi_store::search_patients_fuzzy(&db, "hosp-a", "mar", sql, 2, 0)
        .await
        .expect("p0");
    let p1 = phi_store::search_patients_fuzzy(&db, "hosp-a", "mar", sql, 2, 2)
        .await
        .expect("p1");
    let total = phi_store::count_patients_fuzzy(&db, "hosp-a", "mar", sql)
        .await
        .expect("count");

    assert_eq!(nombres(p0), vec!["Marcelo", "Carla"]);
    assert_eq!(nombres(p1), vec!["Wilmar"]);
    assert_eq!(total, 3, "el recuento debe cubrir las páginas");
}

#[tokio::test]
async fn trigrama_generico_no_devuelve_la_tabla_entera() {
    let (db, _dir) = test_db().await;
    // Todos comparten trigramas con "son" pero ninguno se parece a "zaf".
    for i in 0..5 {
        crear(
            &db,
            paciente(&format!("P-{i}"), "Sonya", "Brunner", &format!("HC-{i}")),
        )
        .await;
    }

    let r = phi_store::search_patients_fuzzy(&db, "hosp-a", "zaf", FILTRO_TODOS, 50, 0)
        .await
        .expect("search");

    assert!(
        r.is_empty(),
        "una consulta sin candidatos reales no debe listarlos: {:?}",
        nombres(r)
    );
}

#[tokio::test]
async fn el_filtro_de_estado_se_respeta() {
    let (db, _dir) = test_db().await;
    crear_con_cama(&db, paciente("P-1", "Gustavo", "Ortiz", "HC-1")).await;
    let egresado = dmart_server::db::get_patient(&db, "P-1")
        .await
        .expect("get")
        .expect("existe");
    assert!(
        egresado.cama_id.is_some(),
        "el paciente debe tener cama para que el egreso sea real"
    );
    dmart_server::db::egresar_paciente(&db, &egresado, "Mejorado")
        .await
        .expect("egreso");

    let r = phi_store::search_patients_fuzzy(&db, "hosp-a", "gust", FILTRO_ACTIVOS, 50, 0)
        .await
        .expect("search");
    assert!(r.is_empty(), "egresado no debe aparecer en activos");
}

#[tokio::test]
async fn no_mezcla_tenants() {
    let (db, _dir) = test_db().await;
    let mut a = paciente("P-1", "Gustavo", "Ortiz", "HC-1");
    a.tenant_id = "hosp-a".into();
    let mut b = paciente("P-2", "Gustavo", "Ortiz", "HC-2");
    b.tenant_id = "hosp-b".into();
    crear(&db, a).await;
    crear(&db, b).await;

    let r = phi_store::search_patients_fuzzy(&db, "hosp-a", "gustavo", FILTRO_TODOS, 50, 0)
        .await
        .expect("search");

    assert_eq!(r.len(), 1, "aislamiento PHI: un tenant no ve al otro");
    assert_eq!(r[0].patient_id, "P-1");
}

#[tokio::test]
async fn la_phi_no_queda_escrita_en_claro_en_la_busqueda() {
    let (db, _dir) = test_db().await;
    crear(&db, paciente("P-1", "Gustavo", "Ortiz", "HC-100516")).await;

    // El índice se lee en crudo: sólo puede haber HMAC, nunca el nombre.
    let raw: Vec<serde_json::Value> = db
        .query("SELECT bi_tng FROM patients")
        .await
        .expect("query")
        .take(0)
        .expect("take");
    let json = serde_json::to_string(&raw).expect("json");

    for secreto in ["Gustavo", "Ortiz", "HC-100516", "gus", "ust"] {
        assert!(
            !json.contains(secreto),
            "trigrama en claro filtró {secreto}"
        );
    }
}
