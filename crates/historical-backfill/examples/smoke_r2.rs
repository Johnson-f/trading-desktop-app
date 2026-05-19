//! Live smoke test against the real Cloudflare R2 Data Catalog.
//!
//! Walks the full write/read chain end-to-end:
//!   1. Load env vars (`WAREHOUSE_*`) from the process environment.
//!   2. Construct `Warehouse` with the production OpenDAL S3 factory.
//!   3. `ensure_tables()` — creates `bars_1d` / `bars_1m` if absent (idempotent).
//!   4. Append one synthetic bar to `bars_1d`.
//!   5. Read it back via `high_water_marks`.
//!   6. Print PASS/FAIL with a clear summary.
//!
//! Run with:
//!     set -a; source server-crates/historical-backfill/.env; set +a
//!     cargo run -p historical-backfill --example smoke_r2
//!
//! Or, if you prefer not to source manually:
//!     env $(grep -v '^#' server-crates/historical-backfill/.env | xargs) \
//!         cargo run -p historical-backfill --example smoke_r2
//!
//! Expected wall-clock: ~10–30 seconds for a fresh warehouse; <5s for a
//! warm one. If it hangs >60 seconds the catalog or S3 endpoint is
//! probably unreachable — Ctrl-C and check the printed config values.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use historical_backfill::Config;
use historical_backfill::test_support::{Bar, Table, Warehouse};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,iceberg=info".into()),
        )
        .init();
    println!("[1/6] loading config from environment...");
    let cfg = Config::from_env().context("Config::from_env — is your .env loaded?")?;
    println!(
        "      catalog_uri    = {}\n      warehouse_name = {}\n      namespace      = {}",
        cfg.catalog_uri, cfg.warehouse_name, cfg.namespace
    );

    println!("[2/6] connecting Warehouse (S3 factory via iceberg-storage-opendal)...");
    let wh = Warehouse::connect(&cfg)
        .await
        .context("Warehouse::connect — check WAREHOUSE_CATALOG_URI and WAREHOUSE_TOKEN")?;
    println!("      connected.");

    println!("[3/6] ensure_tables() — create bars_1d / bars_1m if missing...");
    wh.ensure_tables()
        .await
        .context("ensure_tables — namespace/table creation failed")?;
    println!("      ok.");

    let symbol = "ZANED_SMOKE";
    let ts = DateTime::<Utc>::from_timestamp(1_700_000_000, 0).unwrap();
    let bar = Bar::now_versioned(symbol, ts, 12345, 12399, 12300, 12380, 1_000_000);

    println!("[4/6] appending one synthetic bar (symbol={symbol}, ts={ts})...");
    let written = wh
        .append_bars(Table::Daily, &[bar])
        .await
        .context("append_bars — Parquet write or catalog commit failed")?;
    println!("      wrote {written} bar.");

    println!("[5/6] reading high_water_marks back...");
    let hwm = wh
        .high_water_marks(Table::Daily, &[symbol.to_string()])
        .await
        .context("high_water_marks — Arrow scan failed")?;

    let hwm_ts = hwm
        .get(symbol)
        .copied()
        .context("HWM missing for our just-written symbol — write was silently lost?")?;

    println!("      hwm[{symbol}] = {hwm_ts}");
    if hwm_ts < ts {
        anyhow::bail!("HWM ({hwm_ts}) is earlier than what we just wrote ({ts}) — data loss");
    }

    println!("[6/6] ✓ smoke test PASSED — write/read round-trip works against real R2.");
    println!(
        "      Note: ZANED_SMOKE will sit in bars_1d as a single test row. \
         Delete via PyIceberg if you want a clean table."
    );
    Ok(())
}
