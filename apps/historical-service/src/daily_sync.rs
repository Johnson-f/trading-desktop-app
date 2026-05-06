//! Daily sync pass: full-history daily candles per symbol, incremental
//! against the per-symbol HWM read from `bars_1d` at the start of each
//! cycle.
//!
//! Wall-clock behavior on a 3,500-symbol universe at 8 concurrent fetches:
//! ~1 hour for the first cycle (each symbol returns up to 5,000 daily
//! bars), under 5 minutes per cycle thereafter (only the latest 1–2 bars
//! per symbol pass the HWM filter).

use std::sync::Arc;

use anyhow::Result;
use chrono::{DateTime, Utc};
use futures::stream::{self, StreamExt};

use crate::bar::Bar;
use crate::clickhouse::ClickhouseClient;
use crate::yahoo::YahooSource;

const TABLE: &str = "bars_1d";

pub async fn run_daily_sync(
    universe: &[String],
    ch: Arc<ClickhouseClient>,
    yahoo: Arc<YahooSource>,
    workers: usize,
) -> Result<()> {
    tracing::info!(
        symbol_count = universe.len(),
        workers,
        "daily sync: starting"
    );

    let hwm = ch.high_water_marks(TABLE, universe).await?;

    let inserted: Vec<(String, Result<usize>)> = stream::iter(universe.iter().cloned())
        .map(|symbol| {
            let ch = ch.clone();
            let yahoo = yahoo.clone();
            let hwm_ts = hwm.get(&symbol).copied();
            async move {
                let result = sync_one_symbol(&symbol, hwm_ts, &yahoo, &ch).await;
                (symbol, result)
            }
        })
        .buffer_unordered(workers)
        .collect()
        .await;

    let mut total_bars: u64 = 0;
    let mut symbols_synced: u64 = 0;
    let mut symbols_failed: u64 = 0;

    for (symbol, res) in inserted {
        match res {
            Ok(n) => {
                total_bars += n as u64;
                symbols_synced += 1;
                if n > 0 {
                    tracing::debug!(symbol, bars = n, "daily sync: inserted");
                }
            }
            Err(e) => {
                symbols_failed += 1;
                tracing::warn!(symbol, error = %e, "daily sync: symbol failed");
            }
        }
    }

    tracing::info!(
        total_bars,
        symbols_synced,
        symbols_failed,
        "daily sync: complete"
    );
    Ok(())
}

async fn sync_one_symbol(
    symbol: &str,
    hwm: Option<DateTime<Utc>>,
    yahoo: &YahooSource,
    ch: &ClickhouseClient,
) -> Result<usize> {
    let bars = yahoo.fetch_daily_max(symbol).await?;
    let new_bars: Vec<Bar> = bars
        .into_iter()
        .filter(|b| match hwm {
            Some(h) => b.ts > h,
            None => true,
        })
        .collect();
    let n = new_bars.len();
    if n > 0 {
        ch.insert_bars(TABLE, &new_bars).await?;
    }
    Ok(n)
}
