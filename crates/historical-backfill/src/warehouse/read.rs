//! Version-aware read path. Scans the table with `(symbol, ts BETWEEN ?, ?)`
//! predicates and folds duplicate `(symbol, ts)` rows by keeping the row with
//! the largest `version`. This matches the ClickHouse `ReplacingMergeTree(version)`
//! "FINAL" semantic, applied client-side at scan time.
//!
//! Streaming: holds only one Arrow batch in memory at a time, plus a HashMap
//! keyed by `ts` micros (size proportional to the number of unique timestamps
//! returned, not the raw row count).

use std::collections::HashMap;

use anyhow::{Context, Result};
use arrow_array::{Int32Array, Int64Array, StringArray, TimestampMicrosecondArray};
use chrono::{DateTime, Utc};
use futures::TryStreamExt;
use iceberg::Catalog;
use iceberg::expr::Reference;
use iceberg::spec::Datum;
use iceberg::{NamespaceIdent, TableIdent};

use crate::bar::Bar;
use crate::warehouse::Table;

pub async fn read_bars(
    catalog: &dyn Catalog,
    namespace: &str,
    table: Table,
    symbol: &str,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<Vec<Bar>> {
    let ident = TableIdent::new(
        NamespaceIdent::new(namespace.to_string()),
        table.name().to_string(),
    );
    let tbl = catalog.load_table(&ident).await.context("load_table for read")?;

    let from_us = from.timestamp_micros();
    let to_us = to.timestamp_micros();

    let pred = Reference::new("symbol")
        .equal_to(Datum::string(symbol))
        .and(Reference::new("ts").greater_than_or_equal_to(Datum::timestamptz_micros(from_us)))
        .and(Reference::new("ts").less_than_or_equal_to(Datum::timestamptz_micros(to_us)));

    let scan = tbl
        .scan()
        .with_filter(pred)
        .select(["symbol", "ts", "open", "high", "low", "close", "volume", "version"])
        .build()
        .context("build TableScan")?;

    let mut stream = scan.to_arrow().await.context("scan to_arrow")?;

    let mut by_ts: HashMap<i64, Bar> = HashMap::new();

    while let Some(batch) = stream.try_next().await.context("next batch")? {
        let sym_col = batch
            .column_by_name("symbol")
            .context("symbol column")?
            .as_any()
            .downcast_ref::<StringArray>()
            .context("symbol as StringArray")?;
        let ts_col = batch
            .column_by_name("ts")
            .context("ts column")?
            .as_any()
            .downcast_ref::<TimestampMicrosecondArray>()
            .context("ts as TimestampMicrosecondArray")?;
        let open_col = batch.column_by_name("open").context("open")?
            .as_any().downcast_ref::<Int32Array>().context("open as Int32Array")?;
        let high_col = batch.column_by_name("high").context("high")?
            .as_any().downcast_ref::<Int32Array>().context("high as Int32Array")?;
        let low_col = batch.column_by_name("low").context("low")?
            .as_any().downcast_ref::<Int32Array>().context("low as Int32Array")?;
        let close_col = batch.column_by_name("close").context("close")?
            .as_any().downcast_ref::<Int32Array>().context("close as Int32Array")?;
        let volume_col = batch.column_by_name("volume").context("volume")?
            .as_any().downcast_ref::<Int64Array>().context("volume as Int64Array")?;
        let version_col = batch.column_by_name("version").context("version")?
            .as_any().downcast_ref::<Int64Array>().context("version as Int64Array")?;

        for row in 0..batch.num_rows() {
            let ts_us = ts_col.value(row);
            let version = version_col.value(row);

            let candidate = Bar {
                symbol: sym_col.value(row).to_string(),
                ts: DateTime::<Utc>::from_timestamp_micros(ts_us)
                    .context("invalid timestamp from scan")?,
                open: open_col.value(row),
                high: high_col.value(row),
                low: low_col.value(row),
                close: close_col.value(row),
                volume: volume_col.value(row),
                version,
            };

            by_ts.entry(ts_us)
                .and_modify(|cur| if version > cur.version { *cur = candidate.clone() })
                .or_insert(candidate);
        }
    }

    let mut bars: Vec<Bar> = by_ts.into_values().collect();
    bars.sort_by_key(|b| b.ts);
    Ok(bars)
}
