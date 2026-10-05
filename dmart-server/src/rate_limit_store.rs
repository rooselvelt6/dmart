//! Rate Limit Store - Trait e implementaciones para rate limiting distribuido
//!
//! Permite compartir la cuota de rate limit entre múltiples instancias
//! usando Valkey/Redis como backend.

use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;

/// Trait para almacenar contadores de rate limit de forma distribuida
#[async_trait]
pub trait RateLimitStore: Send + Sync {
    /// Incrementa el contador para una clave y devuelve el nuevo valor
    /// Retorna (count, ttl_remaining_seconds)
    async fn increment(&self, key: &str, window_secs: u64) -> Result<(u32, u64), String>;

    /// Obtiene el valor actual sin incrementar
    async fn get(&self, key: &str) -> Result<Option<(u32, u64)>, String>;

    /// Resetea el contador para una clave
    async fn reset(&self, key: &str) -> Result<(), String>;
}

/// Implementación en memoria para tests y desarrollo
/// NO es thread-safe entre procesos - solo para tests unitarios
pub struct InMemoryFailingStore {
    data: Arc<tokio::sync::RwLock<std::collections::HashMap<String, (u32, std::time::Instant)>>>,
}

impl InMemoryFailingStore {
    pub fn new() -> Self {
        Self {
            data: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        }
    }
}

#[async_trait]
impl RateLimitStore for InMemoryFailingStore {
    async fn increment(&self, key: &str, window_secs: u64) -> Result<(u32, u64), String> {
        let mut data = self.data.write().await;
        let now = std::time::Instant::now();
        let entry = data.entry(key.to_string()).or_insert((0, now));

        // Limpiar si expiró
        if now.duration_since(entry.1) > Duration::from_secs(window_secs) {
            entry.0 = 0;
            entry.1 = now;
        }

        entry.0 += 1;
        let remaining = window_secs.saturating_sub(now.duration_since(entry.1).as_secs());
        Ok((entry.0, remaining))
    }

    async fn get(&self, key: &str) -> Result<Option<(u32, u64)>, String> {
        let data = self.data.read().await;
        if let Some((count, start)) = data.get(key) {
            let now = std::time::Instant::now();
            if now.duration_since(*start) > Duration::from_secs(60) {
                return Ok(None);
            }
            let remaining = 60u64.saturating_sub(now.duration_since(*start).as_secs());
            Ok(Some((*count, remaining)))
        } else {
            Ok(None)
        }
    }

    async fn reset(&self, key: &str) -> Result<(), String> {
        self.data.write().await.remove(key);
        Ok(())
    }
}

/// Implementación usando Valkey/Redis para producción
pub struct ValkeyRateLimitStore {
    client: redis::Client,
}

impl ValkeyRateLimitStore {
    pub fn new(url: &str) -> Result<Self, String> {
        let client =
            redis::Client::open(url).map_err(|e| format!("Valkey connection error: {}", e))?;
        Ok(Self { client })
    }

    async fn get_conn(&self) -> Result<redis::aio::MultiplexedConnection, String> {
        self.client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| format!("Valkey connection error: {}", e))
    }
}

#[async_trait]
impl RateLimitStore for ValkeyRateLimitStore {
    async fn increment(&self, key: &str, window_secs: u64) -> Result<(u32, u64), String> {
        let mut conn = self.get_conn().await?;
        let full_key = format!("ratelimit:{}", key);

        // Usar INCR con EX para TTL atómico
        let count: u32 = redis::cmd("INCR")
            .arg(&full_key)
            .query_async(&mut conn)
            .await
            .map_err(|e| format!("INCR error: {}", e))?;

        // Establecer TTL solo si es la primera vez (count == 1)
        if count == 1 {
            let _: () = redis::cmd("EXPIRE")
                .arg(&full_key)
                .arg(window_secs)
                .query_async(&mut conn)
                .await
                .map_err(|e| format!("EXPIRE error: {}", e))?;
        }

        // Obtener TTL restante
        let ttl: i64 = redis::cmd("TTL")
            .arg(&full_key)
            .query_async(&mut conn)
            .await
            .map_err(|e| format!("TTL error: {}", e))?;

        Ok((count, ttl.max(0) as u64))
    }

    async fn get(&self, key: &str) -> Result<Option<(u32, u64)>, String> {
        let mut conn = self.get_conn().await?;
        let full_key = format!("ratelimit:{}", key);

        let count: Option<u32> = redis::cmd("GET")
            .arg(&full_key)
            .query_async(&mut conn)
            .await
            .map_err(|e| format!("GET error: {}", e))?;

        if let Some(count) = count {
            let ttl: i64 = redis::cmd("TTL")
                .arg(&full_key)
                .query_async(&mut conn)
                .await
                .map_err(|e| format!("TTL error: {}", e))?;
            Ok(Some((count, ttl.max(0) as u64)))
        } else {
            Ok(None)
        }
    }

    async fn reset(&self, key: &str) -> Result<(), String> {
        let mut conn = self.get_conn().await?;
        let full_key = format!("ratelimit:{}", key);
        let _: () = redis::cmd("DEL")
            .arg(&full_key)
            .query_async(&mut conn)
            .await
            .map_err(|e| format!("DEL error: {}", e))?;
        Ok(())
    }
}

/// Factory para crear el store según configuración
pub async fn create_rate_limit_store() -> Arc<dyn RateLimitStore> {
    if let Ok(url) = std::env::var("DMART_VALKEY_URL") {
        if let Ok(store) = ValkeyRateLimitStore::new(&url) {
            tracing::info!("Rate limiting: usando Valkey en {}", url);
            return Arc::new(store);
        }
    }
    tracing::warn!(
        "Rate limiting: usando store en memoria (no distribuido). Configure DMART_VALKEY_URL para producción."
    );
    Arc::new(InMemoryFailingStore::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_inmemory_store_basic() {
        let store = InMemoryFailingStore::new();
        let (count, _) = store.increment("test_key", 60).await.unwrap();
        assert_eq!(count, 1);
        let (count, _) = store.increment("test_key", 60).await.unwrap();
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn test_inmemory_store_reset() {
        let store = InMemoryFailingStore::new();
        store.increment("test_key", 60).await.unwrap();
        store.reset("test_key").await.unwrap();
        let result = store.get("test_key").await.unwrap();
        assert!(result.is_none());
    }
}
