//! Minute sync pass: rolling 5-day window of 1-minute candles per
//! symbol. Stateless — no HWM. Append-only writes; readers dedupe via
//! `version` (the `ReplacingMergeTree` semantic preserved in V1).
//!
//! Architecture: fetch all symbols, group results by calendar day,
//! commit one Iceberg snapshot per day in the window. Five snapshots
//! per cycle is well within the catalog's natural commit cadence.

use std::collections::BTreeMap;
use std::sync::Arc;

use anyhow::Result;
use chrono::NaiveDate;
use futures::stream::{self, StreamExt};

use crate::bar::Bar;
use crate::warehouse::{Table, Warehouse};
use crate::yahoo::YahooSource;

const TABLE: Table = Table::Minute;

pub async fn run_minute_sync(
    universe: &[String],
    wh: Warehouse,
    yahoo: Arc<YahooSource>,
    workers: usize,
) -> Result<()> {
    tracing::info!(
        symbol_count = universe.len(),
        workers,
        "minute sync: starting"
    );

    let per_symbol: Vec<(String, Result<Vec<Bar>>)> = stream::iter(universe.iter().cloned())
        .map(|symbol| {
            let yahoo = yahoo.clone();
            async move {
                let result = yahoo.fetch_minute_window(&symbol).await;
                (symbol, result)
            }
        })
        .buffer_unordered(workers)
        .collect()
        .await;

    let mut by_day: BTreeMap<NaiveDate, Vec<Bar>> = BTreeMap::new();
    let mut symbols_synced: u64 = 0;
    let mut symbols_failed: u64 = 0;

    for (symbol, res) in per_symbol {
        match res {
            Ok(bars) => {
                symbols_synced += 1;
                for bar in bars {
                    by_day.entry(bar.ts.date_naive()).or_default().push(bar);
                }
            }
            Err(e) => {
                symbols_failed += 1;
                tracing::warn!(symbol, error = %e, "minute sync: symbol failed");
            }
        }
    }

    let mut total_bars: u64 = 0;
    let mut days_synced: u64 = 0;
    let mut days_failed: u64 = 0;
    for (day, bars) in by_day {
        match wh.append_bars(TABLE, &bars).await {
            Ok(n) => {
                tracing::debug!(day = %day, bars = n, "minute sync: committed day");
                total_bars += n as u64;
                days_synced += 1;
            }
            Err(e) => {
                days_failed += 1;
                tracing::warn!(day = %day, error = %e, "minute sync: day failed");
            }
        }
    }

    tracing::info!(
        total_bars,
        symbols_synced,
        symbols_failed,
        days_synced,
        days_failed,
        "minute sync: complete"
    );
    Ok(())
}
