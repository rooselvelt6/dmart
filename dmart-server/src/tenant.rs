//! SPEC-025: Multi-tenancy — catálogo de tenants y aislamiento de datos.
//! Modelos y Store de la tabla `tenant`; los handlers HTTP viven en `api::tenant`.
//! El middleware inyecta `tenant_id` en las Claims JWT; toda query de datos debe
//! filtrar por `tenant_id` (RLS pattern) — ver `db::with_tenant_filter`.

use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use surrealdb::Surreal;
use surrealdb::engine::local::Db;

use dmart_shared::models::default_tenant_id;

/// Hospital/unidad (tenant) con slug único e identificable en las Claims JWT.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tenant {
    #[serde(skip_serializing, skip_deserializing, default)]
    pub id: Option<String>,
    /// Slug único (ej: "hosp-a"). Es la key pública y el `tenant_id` de datos.
    pub slug: String,
    pub name: String,
    #[serde(default = "default_active")]
    pub active: bool,
    /// RFC3339
    pub created_at: String,
}

fn default_active() -> bool {
    true
}

impl Tenant {
    pub fn new(slug: String, name: String) -> Self {
        Self {
            id: None,
            slug,
            name,
            active: true,
            created_at: Utc::now().to_rfc3339(),
        }
    }
}

/// Crea un tenant. `slug` debe ser `[a-z0-9-]` (validado por la API).
pub async fn create_tenant(db: &Surreal<Db>, slug: &str, name: &str) -> Result<Tenant> {
    let tenant = Tenant::new(slug.to_string(), name.to_string());
    let created: Option<Tenant> = db
        .create(("tenant", slug.to_string()))
        .content(tenant)
        .await?;
    created.ok_or_else(|| anyhow::anyhow!("Failed to create tenant"))
}

pub async fn list_tenants(db: &Surreal<Db>) -> Result<Vec<Tenant>> {
    let tenants: Vec<Tenant> = db.select("tenant").await?;
    Ok(tenants)
}

pub async fn get_tenant(db: &Surreal<Db>, slug: &str) -> Result<Option<Tenant>> {
    let tenants: Vec<Tenant> = db
        .query("SELECT * FROM tenant WHERE slug = $slug LIMIT 1")
        .bind(("slug", slug.to_string()))
        .await?
        .take(0)?;
    Ok(tenants.into_iter().next())
}

pub async fn tenant_exists(db: &Surreal<Db>, slug: &str) -> Result<bool> {
    Ok(get_tenant(db, slug).await?.is_some())
}

/// Desactiva un tenant (todos sus usuarios quedan out; datos preservados).
pub async fn set_tenant_active(db: &Surreal<Db>, slug: &str, active: bool) -> Result<Option<Tenant>> {
    let updated: Option<Tenant> = db
        .update(("tenant", slug.to_string()))
        .merge(serde_json::json!({ "active": active }))
        .await?;
    Ok(updated)
}

/// Feature flag env: multi-tenant OFF por defecto (single-tenant legacy).
pub fn multi_tenant_enabled() -> bool {
    std::env::var("DMART_MULTI_TENANT")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Slug de tenant por defecto (single-tenant).
pub fn default_tenant() -> String {
    default_tenant_id()
}

#[cfg(test)]
mod tests {
    use super::*;
    use surrealdb::engine::local::SurrealKv;

    async fn test_db() -> (Surreal<Db>, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("tenant.db");
        let db = Surreal::new::<SurrealKv>(path.to_str().unwrap())
            .await
            .expect("connect");
        db.use_ns("dmart").use_db("icu").await.expect("ns");
        crate::migrations::run_migrations(&db).await.expect("migrate");
        (db, dir)
    }

    #[tokio::test]
    async fn tenant_crud_slug_unique() {
        let (db, _dir) = test_db().await;
        let created = create_tenant(&db, "hosp-a", "Hospital A")
            .await
            .expect("create");
        assert_eq!(created.slug, "hosp-a");
        assert!(created.active);

        let dup = create_tenant(&db, "hosp-a", "Otro")
            .await
            .expect_err("duplicate slug must fail");
        assert!(!dup.to_string().is_empty());

        let found = get_tenant(&db, "hosp-a").await.expect("get").expect("exists");
        assert_eq!(found.name, "Hospital A");

        let list = list_tenants(&db).await.expect("list");
        assert_eq!(list.len(), 1);

        let deactivated = set_tenant_active(&db, "hosp-a", false)
            .await
            .expect("update")
            .expect("exists");
        assert!(!deactivated.active);

        assert!(!tenant_exists(&db, "nope").await.expect("check"));
    }

    #[test]
    fn tenant_default_is_default() {
        assert_eq!(default_tenant(), "default");
        assert!(!multi_tenant_enabled());
    }
}