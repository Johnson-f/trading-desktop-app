//! High water marks: per-symbol `max(ts)` across the table. Used by the
//! daily sync pass to skip bars already persisted.
//!
//! V1 implementation: full Arrow scan with a (symbol, ts) projection,
//! aggregate in memory. Works at our row counts (millions, not
//! billions). V2 will read manifest file stats directly to avoid the
//! data-file IO.

use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result};
use arrow_array::{StringArray, TimestampMicrosecondArray};
use chrono::{DateTime, Utc};
use futures::TryStreamExt;
use iceberg::{Catalog, NamespaceIdent, TableIdent};

use crate::warehouse::Table;

pub async fn high_water_marks(
    catalog: &dyn Catalog,
    namespace: &str,
    table: Table,
    symbols: &[String],
) -> Result<HashMap<String, DateTime<Utc>>> {
    if symbols.is_empty() {
        return Ok(HashMap::new());
    }

    let ident = TableIdent::new(
        NamespaceIdent::new(namespace.to_string()),
        table.name().to_string(),
    );
    let tbl = catalog
        .load_table(&ident)
        .await
        .context("load_table for hwm")?;

    let scan = tbl
        .scan()
        .select(["symbol", "ts"])
        .build()
        .context("build TableScan")?;

    let mut hwm: HashMap<String, DateTime<Utc>> = HashMap::new();
    let symbol_filter: HashSet<&str> = symbols.iter().map(String::as_str).collect();

    let mut stream = scan.to_arrow().await.context("scan to_arrow")?;

    while let Some(batch) = stream.try_next().await.context("next batch")? {
        let sym_col = batch
            .column_by_name("symbol")
            .context("symbol column missing")?
            .as_any()
            .downcast_ref::<StringArray>()
            .context("symbol column is not StringArray")?;
        let ts_col = batch
            .column_by_name("ts")
            .context("ts column missing")?
            .as_any()
            .downcast_ref::<TimestampMicrosecondArray>()
            .context("ts column is not TimestampMicrosecondArray")?;

        for row in 0..batch.num_rows() {
            let sym = sym_col.value(row);
            if !symbol_filter.contains(sym) {
                continue;
            }
            let micros = ts_col.value(row);
            let secs = micros / 1_000_000;
            let nanos = ((micros % 1_000_000) * 1_000) as u32;
            let ts = DateTime::<Utc>::from_timestamp(secs, nanos)
                .context("invalid timestamp from scan")?;
            hwm.entry(sym.to_string())
                .and_modify(|cur| {
                    if ts > *cur {
                        *cur = ts;
                    }
                })
                .or_insert(ts);
        }
    }

    Ok(hwm)
}
