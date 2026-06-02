/// Helper: create a temporary SurrealKV database for testing
async fn test_db() -> (surrealdb::Surreal<surrealdb::engine::local::Db>, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let path = dir.path().join("test.db");
    let path_str = path.to_str().expect("invalid path");
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::SurrealKv>(path_str)
        .await
        .expect("failed to connect to SurrealKV");
    db.use_ns("dmart").use_db("icu").await.expect("failed to use namespace");
    (db, dir)
}

#[tokio::test]
async fn test_patient_crud() {
    let (db, _dir) = test_db().await;
    let patient = dmart_shared::models::Patient::new();
    let pid = patient.patient_id.clone();

    let created = dmart_server::db::create_patient(&db, patient)
        .await
        .expect("create failed");
    assert!(!created.patient_id.is_empty());

    let fetched = dmart_server::db::get_patient(&db, &pid)
        .await
        .expect("get failed")
        .expect("patient not found");
    assert_eq!(fetched.patient_id, pid);

    let deleted = dmart_server::db::delete_patient(&db, &pid).await;
    assert!(deleted.is_ok());

    let not_found = dmart_server::db::get_patient(&db, &pid)
        .await
        .expect("get failed");
    assert!(not_found.is_none());
}

#[tokio::test]
async fn test_patient_pagination() {
    let (db, _dir) = test_db().await;
    for _ in 0..5 {
        let p = dmart_shared::models::Patient::new();
        dmart_server::db::create_patient(&db, p)
            .await
            .expect("create failed");
    }

    let total = dmart_server::db::count_patients(&db)
        .await
        .expect("count failed");
    assert!(total >= 5);

    let page = dmart_server::db::list_patients(&db, 2, 0)
        .await
        .expect("list failed");
    assert_eq!(page.len(), 2);
}

#[tokio::test]
async fn test_auth_register() {
    let (db, _dir) = test_db().await;
    let auth = dmart_server::auth::AuthService::new(db);

    let reg = auth
        .register(dmart_server::auth::RegisterRequest {
            username: "testdoc".into(),
            password: "TestPass123!".into(),
            nombre: "Doctor Test".into(),
            rol: "medico".into(),
        })
        .await
        .expect("register failed");
    assert_eq!(reg.username, "testdoc");

    let login = auth
        .authenticate("testdoc", "TestPass123!")
        .await
        .expect("authenticate failed");
    assert!(!login.token.is_empty());

    let refresh = auth.refresh_token(&login.token).await;
    assert!(refresh.is_ok());
}
