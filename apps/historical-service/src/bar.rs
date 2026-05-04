use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Canonical OHLCV row. Produced by every data source (FMP, Yahoo) and
/// consumed by the ClickHouse client.
///
/// Prices are carried as `i32` representing **cents** because the storage
/// schema declares them as `Decimal32(2)` and the `clickhouse` crate's
/// `RowBinaryWithNamesAndTypes` wire format requires Rust types to match
/// the server's binary encoding exactly — Decimal32 is encoded as i32.
/// Convert from the source's `f64` via `Bar::cents_from_dollars` at parse
/// time. The desktop app divides by 100.0 to render.
///
/// The `version` column on the table has `DEFAULT toUnixTimestamp(now())`,
/// but we set it client-side here so the whole batch shares one deterministic
/// version (latest write wins on `ReplacingMergeTree(version)` merges).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ::clickhouse::Row)]
pub struct Bar {
    pub symbol: String,
    #[serde(with = "::clickhouse::serde::chrono::datetime")]
    pub ts: DateTime<Utc>,
    pub open: i32,
    pub high: i32,
    pub low: i32,
    pub close: i32,
    pub volume: u32,
    pub version: u32,
}

impl Bar {
    /// Convert a price expressed in dollars (e.g. `123.45`) to the cents
    /// integer the storage layer expects (`12345`). Rounds half-to-even
    /// (banker's rounding): when the fractional part is exactly 0.5, rounds
    /// to the nearest even integer.
    pub fn cents_from_dollars(price: f64) -> i32 {
        let scaled = price * 100.0;
        let floored = scaled.floor();
        let frac = scaled - floored;
        if (frac - 0.5).abs() < f64::EPSILON {
            // Exactly halfway — round to even
            let floored_i = floored as i32;
            if floored_i % 2 == 0 { floored_i } else { floored_i + 1 }
        } else {
            scaled.round() as i32
        }
    }

    /// Construct a Bar with `version` set to the current Unix timestamp.
    /// Prices are passed in cents already. Use this in tests and any inline
    /// construction; the parsing helpers in `fmp.rs` and `yahoo.rs` do this
    /// themselves.
    pub fn now_versioned(
        symbol: impl Into<String>,
        ts: DateTime<Utc>,
        open: i32,
        high: i32,
        low: i32,
        close: i32,
        volume: u32,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            ts,
            open,
            high,
            low,
            close,
            volume,
            version: Utc::now().timestamp() as u32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_versioned_sets_recent_version() {
        let before = Utc::now().timestamp() as u32;
        let bar = Bar::now_versioned(
            "AAPL",
            DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
            10000,  // $100.00
            10100,  // $101.00
            9950,   // $99.50
            10050,  // $100.50
            12345,
        );
        let after = Utc::now().timestamp() as u32;
        assert!(bar.version >= before && bar.version <= after);
        assert_eq!(bar.symbol, "AAPL");
        assert_eq!(bar.volume, 12345);
        assert_eq!(bar.open, 10000);
    }

    #[test]
    fn cents_from_dollars_rounds_correctly() {
        assert_eq!(Bar::cents_from_dollars(123.45), 12345);
        assert_eq!(Bar::cents_from_dollars(0.01), 1);
        assert_eq!(Bar::cents_from_dollars(0.005), 0); // half-to-even at zero
        assert_eq!(Bar::cents_from_dollars(123.455), 12346); // banker's-style round
        assert_eq!(Bar::cents_from_dollars(0.0), 0);
    }
}
