use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use arrow_array::{
    ArrayRef, Int32Array, Int64Array, RecordBatch, StringArray, TimestampMicrosecondArray,
};
use std::collections::HashMap;

use arrow_schema::{DataType, Field, Schema, TimeUnit};

/// Canonical OHLCV row. Produced by every data source (Yahoo, FMP) and
/// consumed by the Iceberg writer. Prices are carried as `i32` representing
/// cents (e.g. $123.45 → 12345); the Iceberg schema uses `PrimitiveType::Int`
/// for OHLC and `PrimitiveType::Long` for volume and version. Convert
/// dollars→cents at parse time via `Bar::cents_from_dollars`; consumers
/// divide by 100.0 to render.

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bar {
    pub symbol: String,
    pub ts: DateTime<Utc>,
    pub open: i32,
    pub high: i32,
    pub low: i32,
    pub close: i32,
    pub volume: i64,
    pub version: i64,
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
            if floored_i % 2 == 0 {
                floored_i
            } else {
                floored_i + 1
            }
        } else {
            scaled.round() as i32
        }
    }

    /// Construct a Bar with `version` set to the current Unix timestamp.
    /// Prices are passed in cents already. Use this in tests and any inline
    /// construction; the parsing helpers in `yahoo.rs` do this themselves.
    pub fn now_versioned(
        symbol: impl Into<String>,
        ts: DateTime<Utc>,
        open: i32,
        high: i32,
        low: i32,
        close: i32,
        volume: i64,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            ts,
            open,
            high,
            low,
            close,
            volume,
            version: Utc::now().timestamp(),
        }
    }
}

/// Build the canonical Arrow schema for a bar table. Matches the Iceberg schema
/// declared in `warehouse::schema`. Each field carries `PARQUET:field_id`
/// metadata so the Parquet writer can match fields by ID (the default
/// `FieldMatchMode::Id`) without needing the `Name`-based fallback.
pub fn bar_arrow_schema() -> Arc<Schema> {
    use parquet::arrow::PARQUET_FIELD_ID_META_KEY;

    fn field_with_id(name: &str, dtype: DataType, field_id: i32) -> Field {
        Field::new(name, dtype, false).with_metadata(HashMap::from([(
            PARQUET_FIELD_ID_META_KEY.to_string(),
            field_id.to_string(),
        )]))
    }

    Arc::new(Schema::new(vec![
        field_with_id("symbol", DataType::Utf8, 1),
        field_with_id(
            "ts",
            // Use "+00:00" (not "UTC") to match the timezone string that
            // iceberg-rust emits for Timestamptz fields when converting
            // the Iceberg schema to Arrow for the Parquet writer.
            DataType::Timestamp(TimeUnit::Microsecond, Some("+00:00".into())),
            2,
        ),
        field_with_id("open", DataType::Int32, 3),
        field_with_id("high", DataType::Int32, 4),
        field_with_id("low", DataType::Int32, 5),
        field_with_id("close", DataType::Int32, 6),
        field_with_id("volume", DataType::Int64, 7),
        field_with_id("version", DataType::Int64, 8),
    ]))
}

/// Convert a slice of `Bar`s into a single Arrow `RecordBatch` matching
/// `bar_arrow_schema`. Empty input returns an empty (zero-row) batch with the
/// correct columns — iceberg's writer accepts that without surprise.
pub fn bars_to_record_batch(bars: &[Bar]) -> anyhow::Result<RecordBatch> {
    let symbol: ArrayRef = Arc::new(StringArray::from_iter_values(
        bars.iter().map(|b| b.symbol.as_str()),
    ));
    let ts: ArrayRef = Arc::new(
        TimestampMicrosecondArray::from_iter_values(bars.iter().map(|b| b.ts.timestamp_micros()))
            // Use "+00:00" to match iceberg-rust's UTC_TIME_ZONE constant, which
            // is what the Iceberg→Arrow schema conversion emits for Timestamptz.
            .with_timezone("+00:00"),
    );
    let open: ArrayRef = Arc::new(Int32Array::from_iter_values(bars.iter().map(|b| b.open)));
    let high: ArrayRef = Arc::new(Int32Array::from_iter_values(bars.iter().map(|b| b.high)));
    let low: ArrayRef = Arc::new(Int32Array::from_iter_values(bars.iter().map(|b| b.low)));
    let close: ArrayRef = Arc::new(Int32Array::from_iter_values(bars.iter().map(|b| b.close)));
    let volume: ArrayRef = Arc::new(Int64Array::from_iter_values(bars.iter().map(|b| b.volume)));
    let version: ArrayRef = Arc::new(Int64Array::from_iter_values(bars.iter().map(|b| b.version)));

    RecordBatch::try_new(
        bar_arrow_schema(),
        vec![symbol, ts, open, high, low, close, volume, version],
    )
    .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_versioned_sets_recent_version() {
        let before = Utc::now().timestamp();
        let bar = Bar::now_versioned(
            "AAPL",
            DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
            10000, // $100.00
            10100, // $101.00
            9950,  // $99.50
            10050, // $100.50
            12345,
        );
        let after = Utc::now().timestamp();
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

    #[test]
    fn bars_to_record_batch_roundtrips_fields() {
        use arrow_array::{Int32Array, Int64Array, StringArray, TimestampMicrosecondArray};

        let bars = vec![
            Bar::now_versioned(
                "AAPL",
                chrono::DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
                10000,
                10100,
                9950,
                10050,
                12345,
            ),
            Bar::now_versioned(
                "MSFT",
                chrono::DateTime::from_timestamp(1_700_000_060, 0).unwrap(),
                20000,
                20200,
                19800,
                20100,
                67890,
            ),
        ];

        let batch = bars_to_record_batch(&bars).expect("conversion succeeds");

        assert_eq!(batch.num_rows(), 2);
        assert_eq!(batch.num_columns(), 8);

        let symbol = batch
            .column_by_name("symbol")
            .unwrap()
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        assert_eq!(symbol.value(0), "AAPL");
        assert_eq!(symbol.value(1), "MSFT");

        let ts = batch
            .column_by_name("ts")
            .unwrap()
            .as_any()
            .downcast_ref::<TimestampMicrosecondArray>()
            .unwrap();
        assert_eq!(ts.value(0), 1_700_000_000_000_000);

        let open = batch
            .column_by_name("open")
            .unwrap()
            .as_any()
            .downcast_ref::<Int32Array>()
            .unwrap();
        assert_eq!(open.value(0), 10000);

        let volume = batch
            .column_by_name("volume")
            .unwrap()
            .as_any()
            .downcast_ref::<Int64Array>()
            .unwrap();
        assert_eq!(volume.value(0), 12345i64);
    }

    #[test]
    fn bars_to_record_batch_empty_input_returns_empty_batch() {
        let batch = bars_to_record_batch(&[]).expect("conversion succeeds");
        assert_eq!(batch.num_rows(), 0);
        assert_eq!(batch.num_columns(), 8);
    }
}
