//! Spin up an `apache/iceberg-rest-fixture` container backed by an
//! in-memory warehouse. Returns a `Config` pointed at it.
//!
//! NOTE: Multiple tests share the host path `/tmp/iceberg_test_wh` (the
//! LocalFs storage factory writes there). Run integration tests with
//! `--test-threads=1` to avoid parallel collisions:
//!
//!     cargo test -p historical-backfill --test warehouse_append -- --test-threads=1
//!     cargo test -p historical-backfill --test warehouse_tables -- --test-threads=1
//!
//! Each test invocation wipes `/tmp/iceberg_test_wh` at the start of `start()`,
//! so a single-threaded run is always clean. Lib unit tests are unaffected.

use std::time::Duration;

use anyhow::Result;
use testcontainers::core::{IntoContainerPort, WaitFor};
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage, ImageExt};

use historical_backfill::Config;

pub struct CatalogFixture {
    _container: ContainerAsync<GenericImage>,
    pub config: Config,
}

pub async fn start() -> Result<CatalogFixture> {
    // Wipe any stale data from prior test runs. The host-side LocalFs writer
    // uses this same path; without cleanup, parquet files accumulate across
    // `cargo test` invocations and confuse subsequent test catalogs.
    let warehouse_path = std::path::Path::new("/tmp/iceberg_test_wh");
    if warehouse_path.exists() {
        std::fs::remove_dir_all(warehouse_path)
            .map_err(|e| anyhow::anyhow!("wipe stale warehouse {warehouse_path:?}: {e}"))?;
    }

    let image = GenericImage::new("apache/iceberg-rest-fixture", "latest")
        .with_exposed_port(8181u16.tcp())
        .with_wait_for(WaitFor::message_on_stderr("Started oejs.Server"));

    let container = image
        .with_env_var("CATALOG_WAREHOUSE", "/tmp/iceberg_test_wh")
        .start()
        .await?;

    let port = container.get_host_port_ipv4(8181u16.tcp()).await?;
    let uri = format!("http://127.0.0.1:{port}");

    let config = Config {
        catalog_uri: uri,
        // The warehouse param is sent to the catalog's /v1/config endpoint
        // as ?warehouse=... — for the local-filesystem fixture this is not
        // used by the client to locate files, so any non-empty string works.
        warehouse_name: "/tmp/iceberg_test_wh".to_string(),
        token: "test".to_string(),
        namespace: "test_ns".to_string(),
        schedule_hour_utc: 2,
        yahoo_workers: 1,
    };

    // Give the REST server a moment past the WaitFor signal.
    tokio::time::sleep(Duration::from_millis(500)).await;
    Ok(CatalogFixture {
        _container: container,
        config,
    })
}
