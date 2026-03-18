#![allow(dead_code)]

use anyhow::Result;
use redis::aio::MultiplexedConnection;
use redis::AsyncCommands;

pub struct Cache {
    conn: Option<MultiplexedConnection>,
}

impl Cache {
    pub async fn connect(url: &str) -> Self {
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

    pub async fn get(&mut self, key: &str) -> Option<String> {
        let conn = self.conn.as_mut()?;
        conn.get::<_, String>(key).await.ok()
    }

    pub async fn set(&mut self, key: &str, value: &str, ttl_secs: u64) {
        if let Some(conn) = self.conn.as_mut() {
            let _: Result<(), _> = conn.set_ex(key, value, ttl_secs).await;
        }
    }

    pub async fn del(&mut self, key: &str) {
        if let Some(conn) = self.conn.as_mut() {
            let _: Result<(), _> = conn.del(key).await;
        }
    }
}
