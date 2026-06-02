use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use anyhow::Result;
use redis::aio::MultiplexedConnection;
use redis::AsyncCommands;
use tokio::sync::Mutex;

static GLOBAL_CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
static CACHE_AVAILABLE: AtomicBool = AtomicBool::new(false);

pub async fn init_global_cache(url: &str) -> bool {
    let cache = Cache::connect(url).await;
    let available = cache.conn.is_some();
    let _ = GLOBAL_CACHE.set(Mutex::new(cache));
    CACHE_AVAILABLE.store(available, Ordering::Relaxed);
    available
}

pub fn cache_available() -> bool {
    CACHE_AVAILABLE.load(Ordering::Relaxed)
}

pub async fn cache_get(key: &str) -> Option<String> {
    let lock = GLOBAL_CACHE.get()?;
    let mut cache = lock.lock().await;
    cache.get(key).await
}

pub async fn cache_set(key: &str, value: &str, ttl_secs: u64) {
    if let Some(lock) = GLOBAL_CACHE.get() {
        let mut cache = lock.lock().await;
        cache.set(key, value, ttl_secs).await;
    }
}

pub async fn cache_del(key: &str) {
    if let Some(lock) = GLOBAL_CACHE.get() {
        let mut cache = lock.lock().await;
        cache.del(key).await;
    }
}

struct Cache {
    conn: Option<MultiplexedConnection>,
}

impl Cache {
    async fn connect(url: &str) -> Self {
        match redis::Client::open(url) {
            Ok(client) => match client.get_multiplexed_async_connection().await {
                Ok(conn) => {
                    tracing::info!("Valkey/Redis connected at {}", url);
                    Cache { conn: Some(conn) }
                }
                Err(e) => {
                    tracing::warn!("Valkey unavailable ({}), running without cache", e);
                    Cache { conn: None }
                }
            },
            Err(e) => {
                tracing::warn!("Valkey config error ({}), running without cache", e);
                Cache { conn: None }
            }
        }
    }

    async fn get(&mut self, key: &str) -> Option<String> {
        let conn = self.conn.as_mut()?;
        conn.get::<_, String>(key).await.ok()
    }

    async fn set(&mut self, key: &str, value: &str, ttl_secs: u64) {
        if let Some(conn) = self.conn.as_mut() {
            let _: Result<(), _> = conn.set_ex(key, value, ttl_secs).await;
        }
    }

    async fn del(&mut self, key: &str) {
        if let Some(conn) = self.conn.as_mut() {
            let _: Result<(), _> = conn.del(key).await;
        }
    }
}
