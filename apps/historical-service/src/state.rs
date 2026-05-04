use anyhow::{Context, Result};
use async_trait::async_trait;

const KEY_LOCK: &str = "historical_service:run_lock";
const LOCK_TTL_SECS: u64 = 60 * 60 * 6; // 6h: longer than any reasonable single iteration

/// Just a single-runner lock. Unlike symbol-service's JobState, there is no
/// per-symbol cursor — ClickHouse `MAX(ts)` is the source of truth for
/// progress, so the lock is the only thing Redis tracks for this service.
#[async_trait]
pub trait JobLock: Send + Sync {
    /// Acquire the run lock. Returns `true` if we got it. The lock auto-
    /// expires after LOCK_TTL_SECS as a safety net for crashed instances.
    async fn acquire(&self) -> Result<bool>;

    /// Release the lock unconditionally (clean shutdown path).
    async fn release(&self) -> Result<()>;

    /// Force-release any stale lock. Called only at boot when we know we're
    /// the only instance — corrects for previous crashed instances that
    /// couldn't release.
    async fn force_release(&self) -> Result<()>;
}

pub struct RedisJobLock {
    conn: redis::aio::ConnectionManager,
}

impl RedisJobLock {
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
impl JobLock for RedisJobLock {
    async fn acquire(&self) -> Result<bool> {
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

    async fn release(&self) -> Result<()> {
        let mut c = self.conn.clone();
        redis::AsyncCommands::del::<_, ()>(&mut c, KEY_LOCK)
            .await
            .context("redis DEL lock")?;
        Ok(())
    }

    async fn force_release(&self) -> Result<()> {
        // Same as release — idempotent. Naming distinguishes intent at the
        // call site.
        self.release().await
    }
}

/// Test impl — single-threaded `Mutex`-backed.
#[cfg(test)]
pub struct InMemoryJobLock {
    held: tokio::sync::Mutex<bool>,
}

#[cfg(test)]
impl InMemoryJobLock {
    pub fn new() -> Self {
        Self {
            held: tokio::sync::Mutex::new(false),
        }
    }
}

#[cfg(test)]
#[async_trait]
impl JobLock for InMemoryJobLock {
    async fn acquire(&self) -> Result<bool> {
        let mut g = self.held.lock().await;
        if *g {
            return Ok(false);
        }
        *g = true;
        Ok(true)
    }
    async fn release(&self) -> Result<()> {
        *self.held.lock().await = false;
        Ok(())
    }
    async fn force_release(&self) -> Result<()> {
        self.release().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn acquire_is_exclusive_until_release() {
        let l = InMemoryJobLock::new();
        assert!(l.acquire().await.unwrap());
        assert!(!l.acquire().await.unwrap());
        l.release().await.unwrap();
        assert!(l.acquire().await.unwrap());
    }

    #[tokio::test]
    async fn force_release_clears_held_state() {
        let l = InMemoryJobLock::new();
        l.acquire().await.unwrap();
        l.force_release().await.unwrap();
        assert!(l.acquire().await.unwrap());
    }
}
