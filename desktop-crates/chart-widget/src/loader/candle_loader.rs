//! Async candle loader — bridges `zaned_api_client::historical_bars`
//! into the chart widget's per-frame poll loop.
//!
//! Mirrors the pattern in `drawings::persistence`: a once-initialized
//! tokio handle + auth handle live in static slots, and the public
//! `load_*_async` functions return a `oneshot::Receiver` the caller
//! polls each frame. The result is a `CandleData` ready to merge into
//! the chart, or a stringly-typed error for the chart to log.
//!
//! Three entry points:
//!   - [`load_async`]        — initial fetch for the given base scale,
//!                             anchored at "now". Daily → 5y back;
//!                             Minute → 1 trading day back.
//!   - [`load_older_async`]  — backfill chunk for the same base scale,
//!                             anchored at a caller-supplied `before`
//!                             timestamp.
//!   - [`bars_to_candle_data`] — exposed for `set_data` callers that
//!                             have raw bars in hand.
//!
//! Date-string format depends on base scale: daily uses `YYYY-MM-DD`,
//! minute uses `YYYY-MM-DD HH:MM` so each bar has a unique, lex-
//! sortable key. Both formats start with `YYYY-MM-DD` so the existing
//! `parse_ymd` (which only reads the first 10 chars) still works for
//! drawings anchoring and date-axis labels.

use chrono::{DateTime, Duration, Utc};
use tokio::sync::oneshot;
use zaned_api_client::{ApiClient, BarUnit, BucketInput, HistoricalBar};
use zaned_chart_core::{BaseScale, CandleData, CandleInstance};

use crate::config;

/// Lookback per fetch on the daily base. One chunk = 5 years of daily
/// bars (~1,250 trading days), well under the server's 5,000-row limit.
const DAILY_LOOKBACK_DAYS: i64 = 365 * 5;

/// Lookback per fetch on the minute base. 7 days = ~5 trading days of
/// regular + extended hours (~960 minutes/day → ~6,700 rows uncapped),
/// which the server then caps at 5,000 returning the MOST RECENT
/// rows. The wide window means we always anchor at the latest minute
/// data the backfill scheduler has produced — even if its last
/// successful pass is 2-3 calendar days stale (weekend, holiday,
/// crashed run). Without this, a 24h window can sit entirely in a
/// gap past the backfill's high-water mark and return zero rows.
const MINUTE_LOOKBACK_HOURS: i64 = 24 * 7;

const DEFAULT_LIMIT: i32 = 5000;

/// Initial fetch ending at `now`, sized to the base scale. Returns a
/// `oneshot::Receiver` the caller polls each frame; resolves to
/// `Ok(CandleData)` on success or a `String` error (intended for
/// `tracing::warn!` — the chart treats failures as no-ops).
///
/// Sends `Err` synchronously when called before [`init`] or when the
/// global locks are poisoned, so callers always get a usable receiver.
pub fn load_async(
    symbol: String,
    scale: BaseScale,
) -> oneshot::Receiver<Result<CandleData, String>> {
    let to = Utc::now();
    let from = to - lookback_for(scale);
    load_range_async(symbol, scale, from, to)
}

/// Backfill fetch ending at `before`, sized to the base scale. Call
/// when the user scrolls past the leftmost loaded bar; pass the
/// timestamp of that earliest bar as `before`.
///
/// Empty `Ok(CandleData)` means there is no older history at this
/// scale — callers should latch a "history exhausted" flag and stop
/// requesting.
pub fn load_older_async(
    symbol: String,
    scale: BaseScale,
    before: DateTime<Utc>,
) -> oneshot::Receiver<Result<CandleData, String>> {
    let to = before;
    let from = to - lookback_for(scale);
    load_range_async(symbol, scale, from, to)
}

fn lookback_for(scale: BaseScale) -> Duration {
    match scale {
        BaseScale::Daily => Duration::days(DAILY_LOOKBACK_DAYS),
        BaseScale::Minute => Duration::hours(MINUTE_LOOKBACK_HOURS),
    }
}

fn bucket_for(scale: BaseScale) -> BucketInput {
    match scale {
        BaseScale::Daily => BucketInput {
            unit: BarUnit::DAY,
            count: 1,
        },
        BaseScale::Minute => BucketInput {
            unit: BarUnit::MINUTE,
            count: 1,
        },
    }
}

/// Shared async fetch for an explicit `[from, to)` range at the given
/// base scale. Both public entry points funnel through this so the
/// auth + transport plumbing only lives in one place.
fn load_range_async(
    symbol: String,
    scale: BaseScale,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> oneshot::Receiver<Result<CandleData, String>> {
    let (tx, rx) = oneshot::channel();
    let runtime = match config::runtime_handle() {
        Some(h) => h,
        None => {
            let _ = tx.send(Err(
                "chart-widget not initialized (call chart_widget::init first)".into(),
            ));
            return rx;
        }
    };
    let auth = match config::auth() {
        Some(a) => a,
        None => {
            let _ = tx.send(Err(
                "chart-widget not initialized (call chart_widget::init first)".into(),
            ));
            return rx;
        }
    };
    let server_url = match config::server_url() {
        Some(s) => s,
        None => {
            let _ = tx.send(Err(
                "chart-widget not initialized (call chart_widget::init first)".into(),
            ));
            return rx;
        }
    };

    runtime.spawn(async move {
        let bearer = match auth.bearer_boxed().await {
            Ok(token) => token,
            Err(msg) => {
                let _ = tx.send(Err(msg));
                return;
            }
        };

        let client = ApiClient::new(&server_url).with_bearer(bearer);

        let payload = match client
            .historical_bars(symbol, bucket_for(scale), from, to, Some(DEFAULT_LIMIT))
            .await
        {
            Ok(bars) => Ok(bars_to_candle_data(bars, scale)),
            Err(e) => Err(e.to_string()),
        };
        let _ = tx.send(payload);
    });

    rx
}

/// Convert the wire format (i64 cents, RFC-3339 timestamp) into the
/// in-memory `CandleData` the chart consumes.
///
/// Date-string format depends on `scale`:
///   - `Daily`  → `YYYY-MM-DD` (10 chars; one row per trading day).
///   - `Minute` → `YYYY-MM-DD HH:MM` (16 chars; unique per minute, and
///                still starts with `YYYY-MM-DD` so `parse_ymd` works
///                for drawings anchoring + date-axis labels).
///
/// Server timestamps are RFC-3339 like `2026-05-08T20:01:00Z`, so we
/// can slice by byte offset; the fallback is the full string if the
/// server ever sends something shorter.
pub fn bars_to_candle_data(bars: Vec<HistoricalBar>, scale: BaseScale) -> CandleData {
    let instances = bars
        .iter()
        .enumerate()
        .map(|(i, b)| CandleInstance {
            index: i as f32,
            open: (b.open as f64 / 100.0) as f32,
            high: (b.high as f64 / 100.0) as f32,
            low: (b.low as f64 / 100.0) as f32,
            close: (b.close as f64 / 100.0) as f32,
            volume: b.volume as f32,
        })
        .collect();
    let dates = bars
        .iter()
        .map(|b| ts_to_date_string(&b.ts, scale))
        .collect();
    CandleData { instances, dates }
}

/// Format an RFC-3339 timestamp into the `dates` string for `scale`.
fn ts_to_date_string(ts: &str, scale: BaseScale) -> String {
    match scale {
        BaseScale::Daily => ts.get(..10).unwrap_or(ts).to_string(),
        BaseScale::Minute => {
            // Convert RFC-3339 `2026-05-08T20:01:00Z` → `2026-05-08 20:01`.
            // The server may emit fractional seconds (rare for minute
            // bars but possible) — slicing first 16 chars after a `T`
            // swap is enough.
            let bytes = ts.as_bytes();
            if bytes.len() >= 16 && bytes[10] == b'T' {
                let mut out = String::with_capacity(16);
                out.push_str(&ts[..10]);
                out.push(' ');
                out.push_str(&ts[11..16]);
                out
            } else {
                ts.get(..10).unwrap_or(ts).to_string()
            }
        }
    }
}
