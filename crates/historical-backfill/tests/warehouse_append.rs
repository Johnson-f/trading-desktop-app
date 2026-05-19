mod common;

use std::sync::Arc;

use chrono::{DateTime, Utc};
use historical_backfill::test_support::{Bar, Table, Warehouse};
use iceberg::io::LocalFsStorageFactory;

#[tokio::test]
async fn append_three_bars_returns_three() {
    let fix = common::start().await.expect("start REST catalog");
    let wh = Warehouse::connect_with_factory(&fix.config, Arc::new(LocalFsStorageFactory))
        .await
        .expect("connect");
    wh.ensure_tables().await.expect("ensure_tables");

    let ts = DateTime::<Utc>::from_timestamp(1_700_000_000, 0).unwrap();
    let bars = vec![
        Bar::now_versioned("AAPL", ts, 10000, 10100, 9950, 10050, 12345),
        Bar::now_versioned(
            "AAPL",
            ts + chrono::Duration::seconds(86_400),
            10100,
            10200,
            10050,
            10180,
            13000,
        ),
        Bar::now_versioned("MSFT", ts, 20000, 20100, 19950, 20050, 50000),
    ];

    let n = wh.append_bars(Table::Daily, &bars).await.expect("append");
    assert_eq!(n, 3);
}

#[tokio::test]
async fn append_empty_slice_is_noop() {
    let fix = common::start().await.expect("start REST catalog");
    let wh = Warehouse::connect_with_factory(&fix.config, Arc::new(LocalFsStorageFactory))
        .await
        .expect("connect");
    wh.ensure_tables().await.expect("ensure_tables");

    let n = wh.append_bars(Table::Daily, &[]).await.expect("append");
    assert_eq!(n, 0);
}

#[tokio::test]
async fn append_bars_spanning_two_partition_years() {
    let fix = common::start().await.expect("start REST catalog");
    let wh = Warehouse::connect_with_factory(&fix.config, Arc::new(LocalFsStorageFactory))
        .await
        .expect("connect");
    wh.ensure_tables().await.expect("ensure_tables");

    // 2023 bar
    let t_2023 = DateTime::<Utc>::from_timestamp(1_672_531_200, 0).unwrap(); // 2023-01-01
    // 2024 bar
    let t_2024 = DateTime::<Utc>::from_timestamp(1_704_067_200, 0).unwrap(); // 2024-01-01
    let bars = vec![
        Bar::now_versioned("AAPL", t_2023, 10000, 10100, 9950, 10050, 12345),
        Bar::now_versioned("AAPL", t_2024, 10100, 10200, 10050, 10180, 13000),
    ];

    let n = wh.append_bars(Table::Daily, &bars).await.expect("append");
    assert_eq!(n, 2);
    // Survives the commit means FanoutWriter correctly emitted two
    // per-partition Parquet files. No further assertion needed at this stage —
    // Task 11 will add the HWM-based round-trip check.
}

#[tokio::test]
async fn high_water_marks_returns_max_ts_per_symbol() {
    let fix = common::start().await.expect("start REST catalog");
    let wh = Warehouse::connect_with_factory(&fix.config, Arc::new(LocalFsStorageFactory))
        .await
        .expect("connect");
    wh.ensure_tables().await.expect("ensure_tables");

    let t0 = DateTime::<Utc>::from_timestamp(1_700_000_000, 0).unwrap();
    let bars = vec![
        Bar::now_versioned("AAPL", t0, 10000, 10100, 9950, 10050, 12345),
        Bar::now_versioned(
            "AAPL",
            t0 + chrono::Duration::days(2),
            10100,
            10200,
            10050,
            10180,
            13000,
        ),
        Bar::now_versioned(
            "AAPL",
            t0 + chrono::Duration::days(1),
            10050,
            10150,
            9975,
            10090,
            12000,
        ),
        Bar::now_versioned("MSFT", t0, 20000, 20100, 19950, 20050, 50000),
    ];
    wh.append_bars(Table::Daily, &bars).await.expect("append");

    let hwm = wh
        .high_water_marks(Table::Daily, &["AAPL".to_string(), "MSFT".to_string()])
        .await
        .expect("hwm");

    assert_eq!(hwm.get("AAPL"), Some(&(t0 + chrono::Duration::days(2))));
    assert_eq!(hwm.get("MSFT"), Some(&t0));
}

#[tokio::test]
async fn high_water_marks_skips_symbols_not_in_table() {
    let fix = common::start().await.expect("start REST catalog");
    let wh = Warehouse::connect_with_factory(&fix.config, Arc::new(LocalFsStorageFactory))
        .await
        .expect("connect");
    wh.ensure_tables().await.expect("ensure_tables");

    let t0 = DateTime::<Utc>::from_timestamp(1_700_000_000, 0).unwrap();
    wh.append_bars(
        Table::Daily,
        &[Bar::now_versioned("AAPL", t0, 100, 101, 99, 100, 1000)],
    )
    .await
    .expect("append");

    let hwm = wh
        .high_water_marks(Table::Daily, &["AAPL".to_string(), "GOOG".to_string()])
        .await
        .expect("hwm");

    assert_eq!(hwm.get("AAPL"), Some(&t0));
    assert!(!hwm.contains_key("GOOG"));
}

#[tokio::test]
async fn high_water_marks_empty_input_short_circuits() {
    let fix = common::start().await.expect("start REST catalog");
    let wh = Warehouse::connect_with_factory(&fix.config, Arc::new(LocalFsStorageFactory))
        .await
        .expect("connect");
    wh.ensure_tables().await.expect("ensure_tables");

    let hwm = wh.high_water_marks(Table::Daily, &[]).await.expect("hwm");
    assert!(hwm.is_empty());
}
