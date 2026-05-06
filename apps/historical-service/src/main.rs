//! historical-service: nightly Yahoo-backed OHLCV backfill.
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

mod bar;
mod clickhouse;
mod config;
mod daily_sync;
mod minute_sync;
mod scheduler;
mod universe;
mod yahoo;

use std::sync::Arc;

use anyhow::Result;
use tokio::signal::unix::{SignalKind, signal};

use crate::clickhouse::ClickhouseClient;
use crate::config::Config;
use crate::scheduler::wait_until_next_run;
use crate::universe::fetch_universe;
use crate::yahoo::YahooSource;

#[tokio::main]
async fn main() -> Result<()> {
    let crate_env = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".env");
    if dotenvy::from_path(&crate_env).is_err() {
        let _ = dotenvy::dotenv();
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "historical_service=info,markets=warn".into()),
        )
        .init();

    let cfg = Config::from_env()?;
    tracing::info!(
        clickhouse = %cfg.clickhouse_url,
        database = %cfg.clickhouse_database,
        schedule_hour_utc = cfg.schedule_hour_utc,
        yahoo_workers = cfg.yahoo_workers,
        "historical-service starting (yahoo-only)"
    );

    let ch = Arc::new(ClickhouseClient::new(
        &cfg.clickhouse_url,
        &cfg.clickhouse_user,
        &cfg.clickhouse_password,
        &cfg.clickhouse_database,
    )?);
    ch.ensure_table().await?;

    let yahoo = Arc::new(YahooSource::new());

    let sync_loop = async {
        loop {
            run_cycle(ch.clone(), yahoo.clone(), cfg.yahoo_workers).await;
            wait_until_next_run(cfg.schedule_hour_utc).await;
        }
    };

    let mut sigterm = signal(SignalKind::terminate())
        .map_err(|e| anyhow::anyhow!("install SIGTERM: {e}"))?;
    tokio::select! {
        _ = sync_loop => {}
        _ = tokio::signal::ctrl_c() => tracing::info!("SIGINT received; exiting"),
        _ = sigterm.recv() => tracing::info!("SIGTERM received; exiting"),
    }
    Ok(())
}

async fn run_cycle(
    ch: Arc<ClickhouseClient>,
    yahoo: Arc<YahooSource>,
    workers: usize,
) {
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
