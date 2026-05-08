//! Async candle loader — bridges `zaned_api_client::historical_bars`
//! into the chart widget's per-frame poll loop.
//!
//! Mirrors the pattern in `drawings::persistence`: a once-initialized
//! tokio handle + auth handle live in static slots, and the public
//! `load_*_async` functions return a `oneshot::Receiver` the caller
//! polls each frame. The result is a `CandleData` ready to merge into
//! the chart, or a stringly-typed error for the chart to log.
//!
//! Two entry points:
//!   - [`load_async`]       — initial fetch, anchored at "now",
//!                            5 years of daily bars back.
//!   - [`load_older_async`] — backfill chunk anchored at a caller-
//!                            supplied `before` timestamp; loads the
//!                            5 years preceding it. Call when the user
//!                            scrolls past the leftmost loaded bar.

use std::sync::RwLock;

use chrono::{DateTime, Duration, Utc};
use tokio::sync::oneshot;
use zaned_api_client::{ApiClient, BarUnit, BucketInput, HistoricalBar};
use zaned_chart_core::{CandleData, CandleInstance};

use crate::api::client::SERVER_URL;
use crate::auth::{AuthState, AuthStateHandle};

static RUNTIME_HANDLE: RwLock<Option<tokio::runtime::Handle>> = RwLock::new(None);
static AUTH_STATE: RwLock<Option<AuthStateHandle>> = RwLock::new(None);

/// Lookback per fetch (initial AND backfill). One chunk = 5 years of
/// daily bars (~1,250 trading days), well under the server's 5,000-row
/// limit so a single round-trip is enough.
const DEFAULT_LOOKBACK_DAYS: i64 = 365 * 5;
const DEFAULT_LIMIT: i32 = 5000;

/// Wire up the loader. Must be called once at app boot before any
/// `load_*_async` call. Subsequent calls overwrite the slots, which is
/// fine — the handles are cheap to clone.
pub fn init(handle: tokio::runtime::Handle, auth: AuthStateHandle) {
    if let Ok(mut g) = RUNTIME_HANDLE.write() {
        *g = Some(handle);
    }
    if let Ok(mut g) = AUTH_STATE.write() {
        *g = Some(auth);
    }
}

/// Initial fetch: 5 years of daily bars ending at `now`. Returns a
/// `oneshot::Receiver` the caller polls each frame; resolves to
/// `Ok(CandleData)` on success or a `String` error (intended for
/// `tracing::warn!` — the chart treats failures as no-ops).
///
/// Sends `Err` synchronously when called before [`init`] or when the
/// global locks are poisoned, so callers always get a usable receiver.
pub fn load_async(symbol: String) -> oneshot::Receiver<Result<CandleData, String>> {
    let to = Utc::now();
    let from = to - Duration::days(DEFAULT_LOOKBACK_DAYS);
    load_range_async(symbol, from, to)
}

/// Backfill fetch: 5 years of daily bars ending at `before`. Call when
/// the user scrolls past the leftmost loaded bar; pass the timestamp of
/// that earliest bar as `before` and the next chunk of older history
/// comes back.
///
/// Empty `Ok(CandleData)` means the symbol has no older data — callers
/// should latch a "history exhausted" flag and stop requesting.
///
/// Same error/init semantics as [`load_async`].
pub fn load_older_async(
    symbol: String,
    before: DateTime<Utc>,
) -> oneshot::Receiver<Result<CandleData, String>> {
    let to = before;
    let from = to - Duration::days(DEFAULT_LOOKBACK_DAYS);
    load_range_async(symbol, from, to)
}

/// Shared async fetch for an explicit `[from, to)` range. Both public
/// entry points funnel through this so the auth + transport plumbing
/// only lives in one place.
fn load_range_async(
    symbol: String,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> oneshot::Receiver<Result<CandleData, String>> {
    let (tx, rx) = oneshot::channel();
    let (Ok(handle_guard), Ok(auth_guard)) = (RUNTIME_HANDLE.read(), AUTH_STATE.read()) else {
        let _ = tx.send(Err("candle_loader: lock poisoned".into()));
        return rx;
    };
    let (Some(handle), Some(auth)) = (handle_guard.clone(), auth_guard.clone()) else {
        let _ = tx.send(Err("candle_loader not initialized".into()));
        return rx;
    };
    drop(handle_guard);
    drop(auth_guard);

    handle.spawn(async move {
        let bearer = match auth.snapshot().await {
            AuthState::Authenticated { access_token, .. } => access_token,
            other => {
                let _ = tx.send(Err(format!("not authenticated: {other:?}")));
                return;
            }
        };

        let client = ApiClient::new(SERVER_URL).with_bearer(bearer);

        let payload = match client
            .historical_bars(
                symbol,
                BucketInput {
                    unit: BarUnit::DAY,
                    count: 1,
                },
                from,
                to,
                Some(DEFAULT_LIMIT),
            )
            .await
        {
            Ok(bars) => Ok(bars_to_candle_data(bars)),
            Err(e) => Err(e.to_string()),
        };
        let _ = tx.send(payload);
    });

    rx
}

/// Convert the wire format (i64 cents, RFC-3339 timestamp) into the
/// in-memory `CandleData` the chart consumes (f32 dollars,
/// `YYYY-MM-DD` date string). RFC-3339 strings always begin with the
/// 10-char date prefix, so `&ts[..10]` is safe when the server is
/// well-behaved; we fall back to the full string if parsing tripped up
/// somewhere upstream.
fn bars_to_candle_data(bars: Vec<HistoricalBar>) -> CandleData {
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
        .map(|b| b.ts.get(..10).unwrap_or(b.ts.as_str()).to_string())
        .collect();
    CandleData { instances, dates }
}
