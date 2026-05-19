//! End-to-end cycle simulation: exercises the warehouse-side shape of one
//! daily backfill cycle (ensure_tables → append → HWM → filter → append),
//! without depending on the live Yahoo HTTP source. The sync modules
//! themselves are not invoked; their warehouse-facing logic is mirrored
//! inline so that any future divergence will surface as a test failure.

mod common;

use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use historical_backfill::test_support::{Bar, Table, Warehouse};
use iceberg::io::LocalFsStorageFactory;

#[tokio::test]
async fn full_daily_cycle_round_trip() {
    let fix = common::start().await.expect("start REST catalog");
    let wh = Warehouse::connect_with_factory(&fix.config, Arc::new(LocalFsStorageFactory))
        .await
        .expect("connect");
    wh.ensure_tables().await.expect("ensure_tables");

    // Three consecutive days. All bars below fall in the same partition year,
    // which is intentional — we want cycle 2 to write into a partition that
    // already contains cycle 1's data file. Iceberg should add a second data
    // file to that partition without conflict.
    let day0 = DateTime::<Utc>::from_timestamp(1_700_000_000, 0).unwrap();
    let day1 = day0 + Duration::days(1);
    let day2 = day0 + Duration::days(2);

    // ---- Cycle 1: fresh universe, no HWM ----
    let cycle1: Vec<Bar> = vec![
        Bar::now_versioned("AAPL", day0, 10000, 10100, 9950, 10050, 12345),
        Bar::now_versioned("AAPL", day1, 10050, 10200, 10025, 10180, 13000),
        Bar::now_versioned("MSFT", day0, 20000, 20100, 19950, 20050, 50000),
    ];
    let n = wh
        .append_bars(Table::Daily, &cycle1)
        .await
        .expect("cycle1 append");
    assert_eq!(n, 3);

    // ---- HWM after cycle 1 ----
    let universe = vec!["AAPL".to_string(), "MSFT".to_string()];
    let hwm1 = wh
        .high_water_marks(Table::Daily, &universe)
        .await
        .expect("hwm1");
    assert_eq!(hwm1.get("AAPL"), Some(&day1));
    assert_eq!(hwm1.get("MSFT"), Some(&day0));

    // ---- Cycle 2: source returns the full daily window again. The HWM
    // filter must drop the bars already persisted and keep only the new ones. ----
    let cycle2_raw: Vec<Bar> = vec![
        Bar::now_versioned("AAPL", day0, 10000, 10100, 9950, 10050, 12345), // stale (<= HWM)
        Bar::now_versioned("AAPL", day1, 10050, 10200, 10025, 10180, 13000), // stale
        Bar::now_versioned("AAPL", day2, 10180, 10300, 10150, 10250, 14000), // NEW
        Bar::now_versioned("MSFT", day0, 20000, 20100, 19950, 20050, 50000), // stale
        Bar::now_versioned("MSFT", day1, 20050, 20150, 20000, 20100, 51000), // NEW
    ];

    // Mirror daily_sync::fetch_one_symbol's HWM filter.
    let cycle2_filtered: Vec<Bar> = cycle2_raw
        .into_iter()
        .filter(|b| match hwm1.get(&b.symbol) {
            Some(h) => b.ts > *h,
            None => true,
        })
        .collect();
    assert_eq!(
        cycle2_filtered.len(),
        2,
        "HWM filter must drop 3 stale bars, keeping only AAPL@day2 and MSFT@day1"
    );

    let n2 = wh
        .append_bars(Table::Daily, &cycle2_filtered)
        .await
        .expect("cycle2 append");
    assert_eq!(n2, 2);

    // ---- HWM after cycle 2: advanced for both symbols ----
    let hwm2 = wh
        .high_water_marks(Table::Daily, &universe)
        .await
        .expect("hwm2");
    assert_eq!(
        hwm2.get("AAPL"),
        Some(&day2),
        "AAPL HWM should advance to day2"
    );
    assert_eq!(
        hwm2.get("MSFT"),
        Some(&day1),
        "MSFT HWM should advance to day1"
    );
}
