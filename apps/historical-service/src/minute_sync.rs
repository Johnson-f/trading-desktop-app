//! Minute sync pass: rolling 5-day window of 1-minute candles per
//! symbol. Stateless — no HWM, no Redis. ReplacingMergeTree(version) on
//! `bars_1m` dedupes against bars the previous cycle wrote, so re-running
//! is a no-op for unchanged data.
//!
//! The 251 symbols whose deep 2005→present 1-min history was pre-populated
//! by the legacy FMP run are unaffected — those bars sit at older `ts`
//! values outside the 5-day window and stay untouched.

use std::sync::Arc;

use anyhow::Result;
use futures::stream::{self, StreamExt};

use crate::clickhouse::ClickhouseClient;
use crate::yahoo::YahooSource;

const TABLE: &str = "bars_1m";

pub async fn run_minute_sync(
    universe: &[String],
    ch: Arc<ClickhouseClient>,
    yahoo: Arc<YahooSource>,
    workers: usize,
) -> Result<()> {
    tracing::info!(
        symbol_count = universe.len(),
        workers,
        "minute sync: starting"
    );

    let inserted: Vec<(String, Result<usize>)> = stream::iter(universe.iter().cloned())
        .map(|symbol| {
            let ch = ch.clone();
            let yahoo = yahoo.clone();
            async move {
                let result = sync_one_symbol(&symbol, &yahoo, &ch).await;
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
            }
            Err(e) => {
                symbols_failed += 1;
                tracing::warn!(symbol, error = %e, "minute sync: symbol failed");
            }
        }
    }

    tracing::info!(
        total_bars,
        symbols_synced,
        symbols_failed,
        "minute sync: complete"
    );
    Ok(())
}

async fn sync_one_symbol(
    symbol: &str,
    yahoo: &YahooSource,
    ch: &ClickhouseClient,
) -> Result<usize> {
    let bars = yahoo.fetch_minute_window(symbol).await?;
    let n = bars.len();
    if n > 0 {
        ch.insert_bars(TABLE, &bars).await?;
    }
    Ok(n)
}
