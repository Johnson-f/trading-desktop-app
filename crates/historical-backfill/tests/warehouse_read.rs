mod common;

use std::sync::Arc;

use chrono::{DateTime, Utc};
use historical_backfill::test_support::{Bar, Table, Warehouse};
use iceberg::io::LocalFsStorageFactory;

#[tokio::test]
async fn read_bars_dedups_by_version_keeping_newest() {
    let fix = common::start().await.expect("start REST catalog");
    let wh = Warehouse::connect_with_factory(&fix.config, Arc::new(LocalFsStorageFactory))
        .await
        .expect("connect");
    wh.ensure_tables().await.expect("ensure_tables");

    let ts = DateTime::<Utc>::from_timestamp(1_700_000_000, 0).unwrap();

    // First write: AAPL at ts with version=100, close=10050.
    let bar_v1 = Bar {
        symbol: "AAPL".into(),
        ts,
        open: 10000, high: 10100, low: 9950, close: 10050,
        volume: 12345,
        version: 100,
    };
    wh.append_bars(Table::Daily, &[bar_v1]).await.expect("append v1");

    // Second write: same (symbol, ts) with version=200, close=10080.
    let bar_v2 = Bar {
        symbol: "AAPL".into(),
        ts,
        open: 10000, high: 10100, low: 9950, close: 10080,
        volume: 12345,
        version: 200,
    };
    wh.append_bars(Table::Daily, &[bar_v2]).await.expect("append v2");

    // Read back. Even though two rows exist for (AAPL, ts), we must get one,
    // and it must be the version=200 row (close=10080).
    let from = ts - chrono::Duration::days(1);
    let to   = ts + chrono::Duration::days(1);
    let bars = wh.read_bars(Table::Daily, "AAPL", from, to).await.expect("read_bars");

    assert_eq!(bars.len(), 1, "expected exactly one bar after dedup, got {}", bars.len());
    assert_eq!(bars[0].close, 10080, "expected version=200 row to win");
    assert_eq!(bars[0].version, 200);
}

#[tokio::test]
async fn read_bars_returns_empty_when_no_data() {
    let fix = common::start().await.expect("start REST catalog");
    let wh = Warehouse::connect_with_factory(&fix.config, Arc::new(LocalFsStorageFactory))
        .await
        .expect("connect");
    wh.ensure_tables().await.expect("ensure_tables");

    let ts = DateTime::<Utc>::from_timestamp(1_700_000_000, 0).unwrap();
    let bars = wh
        .read_bars(Table::Daily, "AAPL", ts - chrono::Duration::days(1), ts + chrono::Duration::days(1))
        .await
        .expect("read_bars");
    assert!(bars.is_empty());
}

#[tokio::test]
async fn read_bars_filters_by_symbol_and_time_range() {
    let fix = common::start().await.expect("start REST catalog");
    let wh = Warehouse::connect_with_factory(&fix.config, Arc::new(LocalFsStorageFactory))
        .await
        .expect("connect");
    wh.ensure_tables().await.expect("ensure_tables");

    let t0 = DateTime::<Utc>::from_timestamp(1_700_000_000, 0).unwrap();
    let t1 = t0 + chrono::Duration::days(1);
    let t2 = t0 + chrono::Duration::days(2);

    wh.append_bars(Table::Daily, &[
        Bar::now_versioned("AAPL", t0, 10000, 10100, 9950, 10050, 12345),
        Bar::now_versioned("AAPL", t1, 10050, 10150, 10000, 10100, 13000),
        Bar::now_versioned("AAPL", t2, 10100, 10200, 10050, 10150, 14000),
        Bar::now_versioned("MSFT", t0, 20000, 20100, 19950, 20050, 50000),
    ])
    .await
    .expect("append");

    // Window narrower than the data: only t1 should match.
    let from = t1 - chrono::Duration::hours(1);
    let to   = t1 + chrono::Duration::hours(1);
    let bars = wh.read_bars(Table::Daily, "AAPL", from, to).await.expect("read_bars");
    assert_eq!(bars.len(), 1);
    assert_eq!(bars[0].ts, t1);
    assert_eq!(bars[0].symbol, "AAPL");
}
