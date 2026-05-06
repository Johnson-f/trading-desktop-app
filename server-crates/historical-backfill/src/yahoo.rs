//! Yahoo-Finance-backed historical OHLCV source. Wraps the `markets`
//! crate's `Ticker::chart` to expose two fetch shapes used by the daily
//! and minute sync passes.

use anyhow::{Context, Result};
use chrono::{TimeZone, Utc};
use markets::{Interval, Ticker, TimeRange};

use crate::bar::Bar;

/// Stateless Yahoo client. Each call constructs a `Ticker`, hits Yahoo,
/// and converts the candles into our `Bar` shape (cents-as-i32, tagged
/// with the current unix-second `version` for ReplacingMergeTree).
pub struct YahooSource;

impl YahooSource {
    pub fn new() -> Self {
        Self
    }

    /// Fetch the full daily history for `symbol` (Yahoo's `range=max,
    /// interval=1d`). Empty candle lists are returned as `Ok(vec![])` —
    /// dead tickers and IPO-pending symbols look the same as live ones
    /// with no recent trades, so we don't try to distinguish.
    pub async fn fetch_daily_max(&self, symbol: &str) -> Result<Vec<Bar>> {
        let ticker = Ticker::new(symbol)
            .await
            .with_context(|| format!("yahoo Ticker::new {symbol}"))?;
        let chart = ticker
            .chart(Interval::OneDay, TimeRange::Max)
            .await
            .with_context(|| format!("yahoo daily chart {symbol}"))?;
        Ok(candles_to_bars(symbol, &chart.candles))
    }

    /// Fetch 1-minute bars covering Yahoo's deepest free window
    /// (`range=5d, interval=1m`) — five trading days. Yahoo's hard cap on
    /// 1-min data is 7 days; `5d` is the largest enum variant the markets
    /// crate exposes.
    pub async fn fetch_minute_window(&self, symbol: &str) -> Result<Vec<Bar>> {
        let ticker = Ticker::new(symbol)
            .await
            .with_context(|| format!("yahoo Ticker::new {symbol}"))?;
        let chart = ticker
            .chart(Interval::OneMinute, TimeRange::FiveDays)
            .await
            .with_context(|| format!("yahoo 1-min chart {symbol}"))?;
        Ok(candles_to_bars(symbol, &chart.candles))
    }
}

fn candles_to_bars(symbol: &str, candles: &[markets::Candle]) -> Vec<Bar> {
    let version = Utc::now().timestamp() as u32;
    candles
        .iter()
        .filter_map(|c| {
            let ts = Utc.timestamp_opt(c.timestamp, 0).single()?;
            Some(Bar {
                symbol: symbol.to_string(),
                ts,
                open: Bar::cents_from_dollars(c.open),
                high: Bar::cents_from_dollars(c.high),
                low: Bar::cents_from_dollars(c.low),
                close: Bar::cents_from_dollars(c.close),
                volume: (c.volume as i64).max(0) as u32,
                version,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // markets::Candle is #[non_exhaustive], so it cannot be constructed outside
    // the crate. Both tests are ignored; conversion correctness is covered by
    // manual smoke testing against live Yahoo data.

    #[test]
    #[ignore = "markets::Candle is #[non_exhaustive] — cannot construct test fixtures outside the crate"]
    fn candles_to_bars_skips_invalid_timestamps() {
        // Would test that a candle with timestamp=i64::MIN is filtered out.
        unimplemented!()
    }

    #[test]
    #[ignore = "markets::Candle is #[non_exhaustive] — cannot construct test fixtures outside the crate"]
    fn candles_to_bars_converts_prices_to_cents() {
        // Would test OHLCV dollar->cents conversion and symbol propagation.
        unimplemented!()
    }
}
