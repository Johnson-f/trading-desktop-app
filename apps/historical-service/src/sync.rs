use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Duration, TimeZone, Utc};
use futures::stream::{self, StreamExt};
use tokio::sync::mpsc;

use crate::bar::Bar;
use crate::state::JobLock;

/// FMP's `/stable/historical-chart/1min` returns ~1170 rows max per call
/// (≈ 3 trading days). We chunk per-day so each call returns one full day —
/// keeps the response size predictable and the per-symbol throughput easy to
/// reason about.
const FMP_CHUNK: Duration = Duration::days(1);
const YAHOO_WINDOW: Duration = Duration::days(29);
const MAX_ERRORS_PER_SYMBOL: u32 = 5;
const INSERT_CHANNEL_CAPACITY: usize = 64;
/// Bound on consecutive empty FMP responses before we stop processing a
/// symbol this pass. ~60 calendar days covers any reasonable cluster of
/// weekends + holidays + corporate halts; symbols genuinely dead for longer
/// just won't progress this pass (next pass picks up where we left off).
const MAX_EMPTY_CHUNKS: u32 = 60;

#[async_trait]
pub trait BarSource: Send + Sync {
    async fn fetch(
        &self,
        symbol: &str,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<Bar>>;
}

#[async_trait]
pub trait BarSink: Send + Sync {
    async fn high_water_marks(
        &self,
        symbols: &[String],
    ) -> Result<HashMap<String, DateTime<Utc>>>;
    async fn insert_bars(&self, bars: &[Bar]) -> Result<usize>;
}

pub async fn run_sync(
    lock: &dyn JobLock,
    sink: Arc<dyn BarSink>,
    fmp: Arc<dyn BarSource>,
    yahoo: Arc<dyn BarSource>,
    universe: &[String],
    earliest: DateTime<Utc>,
    symbol_earliest: &HashMap<String, DateTime<Utc>>,
    concurrency: usize,
) -> Result<bool> {
    if !lock.acquire().await? {
        tracing::info!("another instance holds the lock; skipping");
        return Ok(false);
    }

    let result = run_sync_inner(
        sink,
        fmp,
        yahoo,
        universe,
        earliest,
        symbol_earliest,
        concurrency,
    )
    .await;
    if result.is_err() {
        tracing::error!(?result, "sync errored; leaving lock for boot recovery");
    } else {
        lock.release().await.ok();
    }
    result
}

async fn run_sync_inner(
    sink: Arc<dyn BarSink>,
    fmp: Arc<dyn BarSource>,
    yahoo: Arc<dyn BarSource>,
    universe: &[String],
    earliest: DateTime<Utc>,
    symbol_earliest: &HashMap<String, DateTime<Utc>>,
    concurrency: usize,
) -> Result<bool> {
    let hwms = sink.high_water_marks(universe).await?;
    let now = Utc::now();

    let work: Vec<(String, DateTime<Utc>)> = universe
        .iter()
        .filter_map(|symbol| {
            // Per-symbol floor: probe phase tells us the earliest date FMP
            // actually has data for this symbol. Symbols with no data at all
            // are stored as DateTime::MAX_UTC — skip them entirely.
            let symbol_floor = symbol_earliest.get(symbol).copied().unwrap_or(earliest);
            if symbol_floor >= now {
                return None;
            }
            let hwm = hwms
                .get(symbol)
                .copied()
                .unwrap_or(symbol_floor.max(earliest));
            if (now - hwm) < Duration::seconds(60) {
                None
            } else {
                Some((symbol.clone(), hwm))
            }
        })
        .collect();

    if work.is_empty() {
        return Ok(false);
    }

    let (tx, mut rx) = mpsc::channel::<Vec<Bar>>(INSERT_CHANNEL_CAPACITY);

    let sink_inserter = sink.clone();
    let inserter = tokio::spawn(async move {
        // Coalesce many fetched chunks into one large insert. ClickHouse's
        // documented sweet spot is ~10k-100k rows per insert; smaller hammers
        // the parts manager, larger risks holding too much in RAM.
        const FLUSH_ROWS: usize = 100_000;
        const FLUSH_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);

        let mut buffer: Vec<Bar> = Vec::with_capacity(FLUSH_ROWS * 2);
        let mut total: usize = 0;
        let mut last_flush = tokio::time::Instant::now();

        loop {
            let timeout = tokio::time::sleep_until(last_flush + FLUSH_INTERVAL);
            let should_flush = tokio::select! {
                maybe_bars = rx.recv() => match maybe_bars {
                    Some(bars) => {
                        buffer.extend(bars);
                        buffer.len() >= FLUSH_ROWS
                    }
                    None => break,
                },
                _ = timeout => !buffer.is_empty(),
            };

            if should_flush {
                let attempted = buffer.len();
                match sink_inserter.insert_bars(&buffer).await {
                    Ok(n) => {
                        total += n;
                        tracing::info!(rows = n, "flushed batch");
                    }
                    Err(e) => {
                        tracing::error!(error = %e, attempted, "insert failed");
                    }
                }
                buffer.clear();
                last_flush = tokio::time::Instant::now();
            }
        }

        if !buffer.is_empty() {
            let attempted = buffer.len();
            match sink_inserter.insert_bars(&buffer).await {
                Ok(n) => total += n,
                Err(e) => tracing::error!(error = %e, attempted, "final insert failed"),
            }
        }
        total
    });

    stream::iter(work)
        .map(|(symbol, hwm)| {
            let fmp = fmp.clone();
            let yahoo = yahoo.clone();
            let tx = tx.clone();
            async move {
                catch_up_symbol(&symbol, hwm, fmp, yahoo, tx, now).await;
            }
        })
        .buffer_unordered(concurrency)
        .for_each(|_| async {})
        .await;

    drop(tx);
    let total_inserted = inserter.await.unwrap_or_default();
    tracing::info!(total_inserted, "sync pass complete");
    Ok(true)
}

/// Compute the start of the calendar day strictly after `ts`. Used to build
/// the next FMP request's `from` so we never re-fetch the day that produced
/// `ts` (FMP's date-only `from`/`to` is inclusive — without this jump we'd
/// always overlap by one day, doubling cost on healthy symbols and looping on
/// symbols whose next chunk happens to be a weekend).
fn day_after(ts: DateTime<Utc>) -> DateTime<Utc> {
    let next_date = ts
        .date_naive()
        .succ_opt()
        .expect("next day exists for any reasonable date");
    Utc.from_utc_datetime(
        &next_date
            .and_hms_opt(0, 0, 0)
            .expect("00:00:00 is always valid"),
    )
}

async fn catch_up_symbol(
    symbol: &str,
    mut hwm: DateTime<Utc>,
    fmp: Arc<dyn BarSource>,
    yahoo: Arc<dyn BarSource>,
    tx: mpsc::Sender<Vec<Bar>>,
    now: DateTime<Utc>,
) {
    let mut errors: u32 = 0;
    let mut empty_streak: u32 = 0;

    loop {
        let gap = now - hwm;
        if gap < Duration::seconds(60) {
            return;
        }

        // Two source paths with different cursoring semantics:
        //   - FMP: per-day chunked. We day-align `from` so consecutive calls
        //     never overlap (FMP's date-only `from`/`to` is inclusive, so
        //     asking from `hwm + 60s` would re-include hwm's whole day).
        //   - Yahoo: one-shot. The wrapper ignores `to` and serves "last N
        //     days since `from`" in a single call, so day-alignment isn't
        //     needed and would lose intraday precision.
        let use_fmp = gap > YAHOO_WINDOW;
        let (source, source_label, from, chunk_end) = if use_fmp {
            let nf = day_after(hwm);
            (&fmp, "fmp", nf, (nf + FMP_CHUNK).min(now))
        } else {
            let f = hwm + Duration::seconds(60);
            (&yahoo, "yahoo", f, now)
        };

        if from >= now {
            return;
        }

        let result = source.fetch(symbol, from, chunk_end).await;
        let bars = match result {
            Ok(b) => b,
            Err(e) => {
                errors += 1;
                tracing::warn!(symbol, source = source_label, error = %e, attempt = errors, "fetch failed");
                if errors >= MAX_ERRORS_PER_SYMBOL {
                    tracing::error!(symbol, "max errors reached, skipping symbol this pass");
                    return;
                }
                let backoff = std::time::Duration::from_secs(1u64 << errors.min(5));
                tokio::time::sleep(backoff).await;
                continue;
            }
        };

        if bars.is_empty() {
            // Yahoo: empty means "no new bars since hwm" — we're caught up.
            // FMP: empty means "no trading on this date" (weekend, holiday,
            // pre-IPO, halt). Skip past the chunk and try the next one,
            // bounded so genuinely-dead symbols don't spin forever.
            if !use_fmp {
                tracing::debug!(symbol, source = source_label, %from, "yahoo returned empty; caught up");
                return;
            }
            empty_streak += 1;
            tracing::debug!(
                symbol,
                source = source_label,
                %from,
                %chunk_end,
                empty_streak,
                "empty response — advancing past gap"
            );
            if empty_streak >= MAX_EMPTY_CHUNKS {
                tracing::debug!(
                    symbol,
                    empty_streak,
                    "max empty streak reached; stopping symbol this pass"
                );
                return;
            }
            // Advance hwm to the chunk_end so the next iteration's day-after
            // computation moves past this empty window. We're not lying about
            // having data here — hwm is just our orchestration cursor; the
            // sink isn't notified until we actually fetch bars.
            hwm = chunk_end;
            continue;
        }
        empty_streak = 0;

        let new_hwm = bars.last().expect("non-empty checked").ts;

        // Safety net: if the source somehow returned data older than what we
        // asked for, we'd otherwise loop forever. Send the batch (let the
        // sink dedupe it) and exit. With day-aligned `from` this shouldn't
        // trigger, but kept as defense in depth.
        if new_hwm <= hwm {
            tracing::warn!(
                symbol,
                source = source_label,
                %from,
                %chunk_end,
                %new_hwm,
                %hwm,
                "no progress: source returned stale data; stopping symbol"
            );
            let _ = tx.send(bars).await;
            return;
        }

        let count = bars.len();
        if tx.send(bars).await.is_err() {
            tracing::warn!(symbol, "insert channel closed; aborting symbol");
            return;
        }
        tracing::info!(
            symbol,
            source = source_label,
            count,
            new_hwm = %new_hwm,
            "page sent to inserter"
        );
        hwm = new_hwm;
        errors = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::InMemoryJobLock;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct EmptySource;
    #[async_trait]
    impl BarSource for EmptySource {
        async fn fetch(
            &self,
            _symbol: &str,
            _from: DateTime<Utc>,
            _to: DateTime<Utc>,
        ) -> Result<Vec<Bar>> {
            Ok(Vec::new())
        }
    }

    struct CountingSource {
        calls: Arc<AtomicUsize>,
    }
    #[async_trait]
    impl BarSource for CountingSource {
        async fn fetch(
            &self,
            _symbol: &str,
            _from: DateTime<Utc>,
            _to: DateTime<Utc>,
        ) -> Result<Vec<Bar>> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(Vec::new())
        }
    }

    struct CapturingSink {
        hwms: Mutex<HashMap<String, DateTime<Utc>>>,
        inserted: Mutex<Vec<Bar>>,
    }
    #[async_trait]
    impl BarSink for CapturingSink {
        async fn high_water_marks(
            &self,
            _symbols: &[String],
        ) -> Result<HashMap<String, DateTime<Utc>>> {
            Ok(self.hwms.lock().unwrap().clone())
        }
        async fn insert_bars(&self, bars: &[Bar]) -> Result<usize> {
            let mut state = self.inserted.lock().unwrap();
            state.extend_from_slice(bars);
            let mut h = self.hwms.lock().unwrap();
            for b in bars {
                let entry = h.entry(b.symbol.clone()).or_insert(b.ts);
                if b.ts > *entry {
                    *entry = b.ts;
                }
            }
            Ok(bars.len())
        }
    }

    fn make_sink() -> Arc<CapturingSink> {
        Arc::new(CapturingSink {
            hwms: Mutex::new(HashMap::new()),
            inserted: Mutex::new(Vec::new()),
        })
    }

    #[tokio::test]
    async fn empty_universe_does_no_work() {
        let lock = InMemoryJobLock::new();
        let sink: Arc<dyn BarSink> = make_sink();
        let fmp: Arc<dyn BarSource> = Arc::new(EmptySource);
        let yahoo: Arc<dyn BarSource> = Arc::new(EmptySource);
        let earliest = Utc::now() - Duration::days(365 * 21);
        let did_work = run_sync(&lock, sink.clone(), fmp, yahoo, &[], earliest, &HashMap::new(), 4)
            .await
            .unwrap();
        assert!(!did_work);
    }

    #[tokio::test]
    async fn fresh_symbol_routes_through_fmp() {
        let fmp_calls = Arc::new(AtomicUsize::new(0));
        let yahoo_calls = Arc::new(AtomicUsize::new(0));
        let fmp: Arc<dyn BarSource> = Arc::new(CountingSource { calls: fmp_calls.clone() });
        let yahoo: Arc<dyn BarSource> = Arc::new(CountingSource { calls: yahoo_calls.clone() });
        let lock = InMemoryJobLock::new();
        let sink: Arc<dyn BarSink> = make_sink();
        let earliest = Utc::now() - Duration::days(365 * 5);
        let universe = vec!["AAPL".to_string()];
        run_sync(&lock, sink, fmp, yahoo, &universe, earliest, &HashMap::new(), 4)
            .await
            .unwrap();
        // FMP gets called repeatedly because the orchestrator skips past
        // empty chunks (weekends, holidays, pre-IPO gaps) until either real
        // data appears or `MAX_EMPTY_CHUNKS` is hit. With a CountingSource
        // that always returns empty, that's exactly MAX_EMPTY_CHUNKS calls.
        assert_eq!(
            fmp_calls.load(Ordering::SeqCst),
            MAX_EMPTY_CHUNKS as usize,
            "FMP should be called MAX_EMPTY_CHUNKS times then bail"
        );
        assert_eq!(yahoo_calls.load(Ordering::SeqCst), 0, "Yahoo not called when gap is huge");
    }

    #[tokio::test]
    async fn recent_hwm_routes_through_yahoo_only() {
        let fmp_calls = Arc::new(AtomicUsize::new(0));
        let yahoo_calls = Arc::new(AtomicUsize::new(0));
        let fmp: Arc<dyn BarSource> = Arc::new(CountingSource { calls: fmp_calls.clone() });
        let yahoo: Arc<dyn BarSource> = Arc::new(CountingSource { calls: yahoo_calls.clone() });

        let lock = InMemoryJobLock::new();
        let mut hwms = HashMap::new();
        hwms.insert("AAPL".to_string(), Utc::now() - Duration::hours(1));
        let sink: Arc<dyn BarSink> = Arc::new(CapturingSink {
            hwms: Mutex::new(hwms),
            inserted: Mutex::new(Vec::new()),
        });
        let universe = vec!["AAPL".to_string()];
        let earliest = Utc::now() - Duration::days(365 * 21);
        run_sync(&lock, sink, fmp, yahoo, &universe, earliest, &HashMap::new(), 4)
            .await
            .unwrap();
        assert_eq!(fmp_calls.load(Ordering::SeqCst), 0, "FMP should NOT be called for recent HWM");
        assert_eq!(yahoo_calls.load(Ordering::SeqCst), 1, "Yahoo called once");
    }

    #[tokio::test]
    async fn lock_contention_skips_pass() {
        let lock = InMemoryJobLock::new();
        lock.acquire().await.unwrap();
        let sink: Arc<dyn BarSink> = make_sink();
        let fmp: Arc<dyn BarSource> = Arc::new(EmptySource);
        let yahoo: Arc<dyn BarSource> = Arc::new(EmptySource);
        let universe = vec!["AAPL".to_string()];
        let earliest = Utc::now() - Duration::days(365);
        let did_work = run_sync(&lock, sink, fmp, yahoo, &universe, earliest, &HashMap::new(), 4)
            .await
            .unwrap();
        assert!(!did_work, "should report no work when lock contended");
    }

    #[tokio::test]
    async fn concurrent_symbols_all_get_processed() {
        // Each of the 8 symbols hits the FMP path with empty responses, so
        // each makes MAX_EMPTY_CHUNKS calls before bailing — proving the
        // orchestrator does dispatch all symbols (no symbol is starved).
        let fmp_calls = Arc::new(AtomicUsize::new(0));
        let fmp: Arc<dyn BarSource> = Arc::new(CountingSource { calls: fmp_calls.clone() });
        let yahoo: Arc<dyn BarSource> = Arc::new(EmptySource);
        let lock = InMemoryJobLock::new();
        let sink: Arc<dyn BarSink> = make_sink();
        let earliest = Utc::now() - Duration::days(365 * 5);
        let universe: Vec<String> = (0..8).map(|i| format!("SYM{i}")).collect();
        run_sync(&lock, sink, fmp, yahoo, &universe, earliest, &HashMap::new(), 4)
            .await
            .unwrap();
        assert_eq!(
            fmp_calls.load(Ordering::SeqCst),
            8 * MAX_EMPTY_CHUNKS as usize,
            "every symbol should hit the empty-skip ceiling exactly once"
        );
    }
}
