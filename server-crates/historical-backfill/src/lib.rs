//! historical-backfill: Yahoo-backed OHLCV backfill, embedded as a
//! library inside the gateway's tokio runtime.
//!
//! On each cycle (default 02:00 UTC):
//!   1. Fetch the universe from Yahoo's screener (~3,500 symbols).
//!   2. Run the daily sync pass: incremental writes to `bars_1d`.
//!   3. Run the minute sync pass: refresh of the rolling 5-day window
//!      of 1-min bars in `bars_1m`.
//!
//! On startup, runs an immediate cycle once before entering the
//! schedule loop — so a fresh deploy populates ClickHouse without
//! waiting until the next 02:00 UTC.
//!
//! # Embedding
//!
//! The gateway calls [`run`] inside a `tokio::spawn` at boot. The task
//! runs forever; cancellation happens implicitly when the runtime
//! drops on shutdown. A panic inside this task does not bring down the
//! gateway — `JoinHandle` surfaces it, and the caller decides whether
//! to log-and-forget or restart.

mod bar;
mod clickhouse;
mod config;
mod daily_sync;
mod minute_sync;
mod scheduler;
mod universe;
mod yahoo;

pub use config::Config;

use std::sync::Arc;

use anyhow::Result;

use crate::clickhouse::ClickhouseClient;
use crate::scheduler::wait_until_next_run;
use crate::universe::fetch_universe;
use crate::yahoo::YahooSource;

/// Run the backfill scheduler forever. Caller is expected to
/// `tokio::spawn` this; cancellation occurs when the runtime shuts
/// down. Returns `Err` only if initial setup (ClickHouse client build,
/// table creation) fails — once the loop starts, transient cycle
/// errors are logged and the next cycle proceeds.
pub async fn run(cfg: Config) -> Result<()> {
    tracing::info!(
        clickhouse = %cfg.clickhouse_url,
        database = %cfg.clickhouse_database,
        schedule_hour_utc = cfg.schedule_hour_utc,
        yahoo_workers = cfg.yahoo_workers,
        "historical-backfill starting"
    );

    let ch = Arc::new(ClickhouseClient::new(
        &cfg.clickhouse_url,
        &cfg.clickhouse_user,
        &cfg.clickhouse_password,
        &cfg.clickhouse_database,
    )?);
    ch.ensure_table().await?;

    let yahoo = Arc::new(YahooSource::new());

    loop {
        run_cycle(ch.clone(), yahoo.clone(), cfg.yahoo_workers).await;
        wait_until_next_run(cfg.schedule_hour_utc).await;
    }
}

async fn run_cycle(ch: Arc<ClickhouseClient>, yahoo: Arc<YahooSource>, workers: usize) {
    let universe = match fetch_universe().await {
        Ok(u) => u,
        Err(e) => {
            tracing::error!(error = ?e, "fetch_universe failed; skipping cycle");
            return;
        }
    };
    tracing::info!(symbol_count = universe.len(), "cycle: universe fetched");

    if let Err(e) = daily_sync::run_daily_sync(&universe, ch.clone(), yahoo.clone(), workers).await
    {
        tracing::error!(error = ?e, "daily sync failed");
    }

    if let Err(e) = minute_sync::run_minute_sync(&universe, ch, yahoo, workers).await {
        tracing::error!(error = ?e, "minute sync failed");
    }
}
