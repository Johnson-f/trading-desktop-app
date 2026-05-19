mod common;

use std::sync::Arc;

use historical_backfill::test_support::Warehouse;
use iceberg::io::LocalFsStorageFactory;

#[tokio::test]
async fn ensure_tables_creates_both_tables_idempotently() {
    let fix = common::start().await.expect("start REST catalog");
    let wh = Warehouse::connect_with_factory(&fix.config, Arc::new(LocalFsStorageFactory))
        .await
        .expect("connect");
    wh.ensure_tables().await.expect("ensure_tables 1st time");
    // Idempotent — second call must not error.
    wh.ensure_tables().await.expect("ensure_tables 2nd time");
}
