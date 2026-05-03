use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};

const KEY_CURRENT: &str = "symbol_service:current_run";
const KEY_LAST: &str = "symbol_service:last_completed_run";
const KEY_LOCK: &str = "symbol_service:run_lock";
const LOCK_TTL_SECS: u64 = 60 * 60 * 6; // 6h: longer than any real run

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunCursor {
    pub run_id: String,
    pub started_at: DateTime<Utc>,
    /// Yahoo exchange code currently being processed (e.g. "NMS").
    pub exchange: String,
    /// Next page offset to request inside `exchange`.
    pub offset: u32,
    pub total_upserted: u64,
}

#[async_trait]
pub trait JobState: Send + Sync {
    /// Return the in-flight run cursor, if any.
    async fn load_current(&self) -> Result<Option<RunCursor>>;

    /// Persist the cursor (overwrite). Called after each successful upsert.
    async fn save_current(&self, cursor: &RunCursor) -> Result<()>;

    /// Mark the run done: rename current → last_completed atomically and
    /// clear the lock. Idempotent — if there's no current, returns Ok(()).
    async fn complete_current(&self) -> Result<()>;

    /// Acquire the single-runner lock. Returns true if we got it.
    /// Lock auto-expires after LOCK_TTL_SECS as a safety net.
    async fn acquire_lock(&self) -> Result<bool>;

    /// Release the lock unconditionally (used at clean shutdown).
    async fn release_lock(&self) -> Result<()>;
}

pub struct RedisJobState {
    conn: redis::aio::ConnectionManager,
}

impl RedisJobState {
    pub async fn connect(url: &str) -> Result<Self> {
        let client = redis::Client::open(url).context("open redis client")?;
        let conn = client
            .get_connection_manager()
            .await
            .context("redis connection manager")?;
        Ok(Self { conn })
    }
}

#[async_trait]
impl JobState for RedisJobState {
    async fn load_current(&self) -> Result<Option<RunCursor>> {
        let mut c = self.conn.clone();
        let json: Option<String> = c.get(KEY_CURRENT).await.context("redis GET current")?;
        match json {
            Some(s) => Ok(Some(serde_json::from_str(&s).context("parse cursor")?)),
            None => Ok(None),
        }
    }

    async fn save_current(&self, cursor: &RunCursor) -> Result<()> {
        let mut c = self.conn.clone();
        let json = serde_json::to_string(cursor).context("serialize cursor")?;
        let _: () = c.set(KEY_CURRENT, json).await.context("redis SET current")?;
        Ok(())
    }

    async fn complete_current(&self) -> Result<()> {
        let mut c = self.conn.clone();
        // RENAME fails if source missing — treat that as already-complete.
        let _: redis::RedisResult<()> = c.rename(KEY_CURRENT, KEY_LAST).await;
        let _: () = c.del(KEY_LOCK).await.context("redis DEL lock")?;
        Ok(())
    }

    async fn acquire_lock(&self) -> Result<bool> {
        let mut c = self.conn.clone();
        let acquired: bool = redis::cmd("SET")
            .arg(KEY_LOCK)
            .arg(chrono::Utc::now().to_rfc3339())
            .arg("NX")
            .arg("EX")
            .arg(LOCK_TTL_SECS)
            .query_async(&mut c)
            .await
            .context("redis SET NX lock")?;
        Ok(acquired)
    }

    async fn release_lock(&self) -> Result<()> {
        let mut c = self.conn.clone();
        let _: () = c.del(KEY_LOCK).await.context("redis DEL lock")?;
        Ok(())
    }
}

/// Test impl — single-threaded `Mutex`-backed.
#[cfg(test)]
pub struct InMemoryJobState {
    inner: tokio::sync::Mutex<InMemoryInner>,
}

#[cfg(test)]
#[derive(Default)]
struct InMemoryInner {
    current: Option<RunCursor>,
    last: Option<RunCursor>,
    lock_held: bool,
}

#[cfg(test)]
impl InMemoryJobState {
    pub fn new() -> Self {
        Self {
            inner: tokio::sync::Mutex::new(InMemoryInner::default()),
        }
    }
}

#[cfg(test)]
#[async_trait]
impl JobState for InMemoryJobState {
    async fn load_current(&self) -> Result<Option<RunCursor>> {
        Ok(self.inner.lock().await.current.clone())
    }
    async fn save_current(&self, c: &RunCursor) -> Result<()> {
        self.inner.lock().await.current = Some(c.clone());
        Ok(())
    }
    async fn complete_current(&self) -> Result<()> {
        let mut g = self.inner.lock().await;
        g.last = g.current.take();
        g.lock_held = false;
        Ok(())
    }
    async fn acquire_lock(&self) -> Result<bool> {
        let mut g = self.inner.lock().await;
        if g.lock_held {
            return Ok(false);
        }
        g.lock_held = true;
        Ok(true)
    }
    async fn release_lock(&self) -> Result<()> {
        self.inner.lock().await.lock_held = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn lock_is_exclusive_until_released() {
        let s = InMemoryJobState::new();
        assert!(s.acquire_lock().await.unwrap());
        assert!(!s.acquire_lock().await.unwrap());
        s.release_lock().await.unwrap();
        assert!(s.acquire_lock().await.unwrap());
    }

    #[tokio::test]
    async fn complete_clears_current_and_lock() {
        let s = InMemoryJobState::new();
        s.acquire_lock().await.unwrap();
        s.save_current(&RunCursor {
            run_id: "r".into(),
            started_at: Utc::now(),
            exchange: "NMS".into(),
            offset: 250,
            total_upserted: 100,
        })
        .await
        .unwrap();
        s.complete_current().await.unwrap();
        assert!(s.load_current().await.unwrap().is_none());
        assert!(s.acquire_lock().await.unwrap()); // lock released
    }
}
