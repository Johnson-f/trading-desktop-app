//! Discover the earliest date FMP has 1-min data for each symbol, so the main
//! sync loop never wastes calls on pre-IPO date ranges.
//!
//! For each symbol with no cached metadata:
//! 1. Probe `now`. If empty, the symbol is dead — skip entirely.
//! 2. Probe `floor` (BACKFILL_EARLIEST). If non-empty, earliest = floor.
//! 3. Otherwise binary-search between `floor` and `now` for the cutoff,
//!    stopping when the window narrows to ~30 days (close enough).
//!
//! Persists results to `symbol_metadata` so the cost is paid once across the
//! service's lifetime, not per restart.

use std::sync::Arc;

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use futures::stream::{self, StreamExt};

use crate::clickhouse::ClickhouseClient;
use crate::fmp::FmpClient;

/// Resolution at which we stop binary-searching. 30 days = one calendar month
/// — close enough to avoid wasted FMP calls in the main loop without spending
/// extra search steps for pinpoint precision.
const BISECTION_RESOLUTION: Duration = Duration::days(30);

/// Sentinel for "FMP has no data for this symbol." Stored in `symbol_metadata`
/// so we don't re-probe on next boot. Must be representable as ClickHouse
/// `DateTime` (UInt32 epoch seconds, max ≈ 2106-02-07) and far enough in the
/// future that the main loop's `symbol_floor >= now` check always skips it.
fn dead_sentinel() -> DateTime<Utc> {
    chrono::TimeZone::with_ymd_and_hms(&Utc, 2099, 1, 1, 0, 0, 0)
        .single()
        .expect("2099-01-01 is a valid date")
}

/// Outcome of probing one symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeOutcome {
    /// Symbol has data at this earliest date (or earlier).
    Earliest(DateTime<Utc>),
    /// FMP returned no data even for `now` — symbol is dead, skip in main loop.
    Dead,
}

/// Probe one symbol via FMP. Returns the earliest date FMP serves data for,
/// or `Dead` if FMP has no data at all for this symbol.
pub async fn probe_symbol(
    fmp: &FmpClient,
    symbol: &str,
    floor: DateTime<Utc>,
) -> Result<ProbeOutcome> {
    let now = Utc::now();

    // Step 1: does the symbol have any recent data? Use a 3-day window to
    // ride over weekends without false negatives.
    let recent = fmp
        .fetch_1min(symbol, now - Duration::days(3), now)
        .await?;
    if recent.is_empty() {
        return Ok(ProbeOutcome::Dead);
    }

    // Step 2: does FMP serve data at the configured floor? If yes, we're done.
    let at_floor = fmp
        .fetch_1min(symbol, floor, floor + Duration::days(3))
        .await?;
    if !at_floor.is_empty() {
        return Ok(ProbeOutcome::Earliest(floor));
    }

    // Step 3: binary-search between floor (no data) and now (has data) for
    // the cutoff. Loop invariant: lo has no data, hi has data.
    let mut lo = floor;
    let mut hi = now;
    while hi - lo > BISECTION_RESOLUTION {
        let mid = lo + (hi - lo) / 2;
        let probe = fmp.fetch_1min(symbol, mid, mid + Duration::days(3)).await?;
        if probe.is_empty() {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Ok(ProbeOutcome::Earliest(hi))
}

/// Probe every symbol in `universe` that isn't already in the metadata cache.
/// Persists discovered dates to ClickHouse. Returns the full earliest-date map
/// (cached + newly discovered, with `Dead` symbols absent).
pub async fn discover_universe_earliest(
    ch: &ClickhouseClient,
    fmp: Arc<FmpClient>,
    universe: &[String],
    floor: DateTime<Utc>,
    concurrency: usize,
) -> Result<std::collections::HashMap<String, DateTime<Utc>>> {
    let mut cache = ch.load_symbol_metadata().await?;
    let to_probe: Vec<String> = universe
        .iter()
        .filter(|s| !cache.contains_key(*s))
        .cloned()
        .collect();

    if to_probe.is_empty() {
        tracing::info!(
            cached = cache.len(),
            universe = universe.len(),
            "all symbols already probed; skipping discovery"
        );
        return Ok(cache);
    }

    let total_to_probe = to_probe.len();
    tracing::info!(
        to_probe = total_to_probe,
        cached = cache.len(),
        universe = universe.len(),
        concurrency,
        "starting probe phase"
    );

    // Persist results in checkpoints so progress survives restarts AND is
    // visible mid-run via `SELECT count() FROM symbol_metadata`. Without this
    // a 15-min probe loses everything on Ctrl-C.
    const CHECKPOINT_EVERY: usize = 100;

    let mut stream = stream::iter(to_probe.into_iter())
        .map(|symbol| {
            let fmp = fmp.clone();
            async move {
                match probe_symbol(&fmp, &symbol, floor).await {
                    Ok(outcome) => {
                        tracing::debug!(symbol, ?outcome, "probed");
                        (symbol, outcome)
                    }
                    Err(e) => {
                        tracing::warn!(symbol, error = %e, "probe failed; treating as Dead");
                        (symbol, ProbeOutcome::Dead)
                    }
                }
            }
        })
        .buffer_unordered(concurrency);

    let mut batch: Vec<(String, DateTime<Utc>)> = Vec::with_capacity(CHECKPOINT_EVERY);
    let mut alive_count = 0usize;
    let mut dead_count = 0usize;
    let mut done = 0usize;

    while let Some((symbol, outcome)) = stream.next().await {
        let earliest = match outcome {
            ProbeOutcome::Earliest(ts) => {
                alive_count += 1;
                ts
            }
            ProbeOutcome::Dead => {
                dead_count += 1;
                // Sentinel must be ClickHouse-representable; see dead_sentinel().
                dead_sentinel()
            }
        };
        cache.insert(symbol.clone(), earliest);
        batch.push((symbol, earliest));
        done += 1;

        if batch.len() >= CHECKPOINT_EVERY {
            ch.upsert_symbol_metadata(&batch).await?;
            tracing::info!(
                done,
                total = total_to_probe,
                alive = alive_count,
                dead = dead_count,
                pct = format!("{:.1}%", (done as f64 / total_to_probe as f64) * 100.0),
                "probe checkpoint"
            );
            batch.clear();
        }
    }

    if !batch.is_empty() {
        ch.upsert_symbol_metadata(&batch).await?;
    }

    tracing::info!(
        alive = alive_count,
        dead = dead_count,
        cached_total = cache.len(),
        "probe phase complete"
    );
    Ok(cache)
}
