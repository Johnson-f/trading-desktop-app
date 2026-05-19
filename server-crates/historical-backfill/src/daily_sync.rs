//! Daily sync pass: full-history daily candles per symbol, incremental
//! against per-symbol HWMs from the start of the cycle.
//!
//! Architecture shift vs. ClickHouse era: previously we did one INSERT
//! per symbol (cheap in ClickHouse). With Iceberg every commit is a
//! snapshot; we batch all symbols' new bars into one snapshot per cycle.

use std::sync::Arc;

use anyhow::Result;
use chrono::{DateTime, Utc};
use futures::stream::{self, StreamExt};

use crate::bar::Bar;
use crate::warehouse::{Table, Warehouse};
use crate::yahoo::YahooSource;

const TABLE: Table = Table::Daily;

pub async fn run_daily_sync(
    universe: &[String],
    wh: Warehouse,
    yahoo: Arc<YahooSource>,
    workers: usize,
) -> Result<()> {
    tracing::info!(
        symbol_count = universe.len(),
        workers,
        "daily sync: starting"
    );

    let hwm = wh.high_water_marks(TABLE, universe).await?;

    let per_symbol: Vec<(String, Result<Vec<Bar>>)> = stream::iter(universe.iter().cloned())
        .map(|symbol| {
            let yahoo = yahoo.clone();
            let hwm_ts = hwm.get(&symbol).copied();
            async move {
                let result = fetch_one_symbol(&symbol, hwm_ts, &yahoo).await;
                (symbol, result)
            }
        })
        .buffer_unordered(workers)
        .collect()
        .await;

    let mut all_new_bars: Vec<Bar> = Vec::new();
    let mut symbols_synced: u64 = 0;
    let mut symbols_failed: u64 = 0;

    for (symbol, res) in per_symbol {
        match res {
            Ok(mut bars) => {
                symbols_synced += 1;
                if !bars.is_empty() {
                    tracing::debug!(symbol, bars = bars.len(), "daily sync: queued");
                    all_new_bars.append(&mut bars);
                }
            }
            Err(e) => {
                symbols_failed += 1;
                tracing::warn!(symbol, error = %e, "daily sync: symbol failed");
            }
        }
    }

    let total_bars = all_new_bars.len() as u64;
    let written = wh.append_bars(TABLE, &all_new_bars).await?;

    tracing::info!(
        total_bars,
        written,
        symbols_synced,
        symbols_failed,
        "daily sync: complete"
    );
    Ok(())
}

async fn fetch_one_symbol(
    symbol: &str,
    hwm: Option<DateTime<Utc>>,
    yahoo: &YahooSource,
) -> Result<Vec<Bar>> {
    let bars = yahoo.fetch_daily_max(symbol).await?;
    Ok(bars
        .into_iter()
        .filter(|b| match hwm {
            Some(h) => b.ts > h,
            None => true,
        })
        .collect())
}
