//! Iceberg-on-R2 warehouse client. Replaces the previous ClickHouse client.
//! Public surface is the [`Warehouse`] type. Submodules are private to the
//! crate.

mod append;
mod catalog;
mod hwm;
mod schema;
mod tables;

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use iceberg::Catalog;
use iceberg::io::{S3_ENDPOINT, S3_REGION, StorageFactory};
use iceberg_storage_opendal::OpenDalStorageFactory;

use crate::bar::Bar;
use crate::config::Config;

/// Bars-table identifier. Two values: `Daily` (`bars_1d`) and `Minute`
/// (`bars_1m`). Used by sync passes to pick which table to write into.
#[derive(Debug, Clone, Copy)]
pub enum Table {
    Daily,
    Minute,
}

impl Table {
    pub fn name(self) -> &'static str {
        match self {
            Table::Daily => "bars_1d",
            Table::Minute => "bars_1m",
        }
    }
}

/// Iceberg-backed warehouse. One per process. Cheap to clone (internal
/// catalog handle is `Arc`).
#[derive(Clone)]
pub struct Warehouse {
    catalog: Arc<dyn Catalog>,
    namespace: String,
}

impl Warehouse {
    /// Production constructor. Wires an OpenDAL S3-compatible storage factory
    /// for R2-backed Iceberg tables. The R2 Data Catalog vends per-table
    /// SigV4 credentials via the REST protocol; the factory receives them
    /// automatically via the `StorageConfig` props on each `loadTable` call.
    /// No static credentials are baked in — the vended-credentials pattern
    /// is used exclusively.
    pub async fn connect(cfg: &Config) -> Result<Self> {
        let endpoint = r2_s3_endpoint_from_catalog_uri(&cfg.catalog_uri)?;
        let factory = build_r2_s3_factory();
        let extra_props = HashMap::from([
            (S3_ENDPOINT.to_string(), endpoint),
            (S3_REGION.to_string(), "auto".to_string()),
        ]);
        let catalog = catalog::build_rest_catalog(cfg, factory, extra_props).await?;
        Ok(Self {
            catalog: Arc::new(catalog),
            namespace: cfg.namespace.clone(),
        })
    }

    /// Test-facing constructor that accepts an explicit storage factory.
    /// Integration tests pass `LocalFsStorageFactory` to talk to the
    /// `apache/iceberg-rest-fixture` container's local filesystem.
    pub async fn connect_with_factory(
        cfg: &Config,
        storage_factory: Arc<dyn StorageFactory>,
    ) -> Result<Self> {
        let catalog = catalog::build_rest_catalog(cfg, storage_factory, HashMap::new()).await?;
        Ok(Self {
            catalog: Arc::new(catalog),
            namespace: cfg.namespace.clone(),
        })
    }

    pub async fn ensure_tables(&self) -> Result<()> {
        tables::ensure_namespace(&*self.catalog, &self.namespace).await?;
        tables::ensure_table(&*self.catalog, &self.namespace, Table::Daily).await?;
        tables::ensure_table(&*self.catalog, &self.namespace, Table::Minute).await?;
        Ok(())
    }

    pub async fn high_water_marks(
        &self,
        table: Table,
        symbols: &[String],
    ) -> Result<HashMap<String, DateTime<Utc>>> {
        hwm::high_water_marks(&*self.catalog, &self.namespace, table, symbols).await
    }

    pub async fn append_bars(&self, table: Table, bars: &[Bar]) -> Result<usize> {
        if bars.is_empty() {
            return Ok(0);
        }
        append::append_bars(&*self.catalog, &self.namespace, table, bars).await
    }
}

/// Derive the R2 S3-compatible endpoint from a Cloudflare R2 catalog URI.
///
/// R2 catalog URIs have the shape:
///   `https://catalog.cloudflarestorage.com/<account_hash>/<bucket>`
///
/// The corresponding S3 endpoint is:
///   `https://<account_hash>.r2.cloudflarestorage.com`
fn r2_s3_endpoint_from_catalog_uri(catalog_uri: &str) -> Result<String> {
    let url = reqwest::Url::parse(catalog_uri).context("parse catalog URI as URL")?;
    let account_hash = url
        .path_segments()
        .context("catalog URI has no path segments")?
        .find(|s| !s.is_empty())
        .context(
            "catalog URI path is empty — expected \
             https://catalog.cloudflarestorage.com/<account_hash>/<bucket>",
        )?;
    Ok(format!("https://{account_hash}.r2.cloudflarestorage.com"))
}

/// Build the OpenDAL S3 storage factory used for R2 in production.
///
/// No static credentials are provided. The REST catalog injects vended
/// SigV4 credentials per table (via `StorageConfig` props) when it processes
/// the `loadTable` response from the R2 Data Catalog.
///
/// Endpoint and region are injected by the caller via the catalog's
/// `extra_props` so they propagate into every `FileIO` the catalog builds.
fn build_r2_s3_factory() -> Arc<dyn StorageFactory> {
    Arc::new(OpenDalStorageFactory::S3 {
        configured_scheme: "s3".to_string(),
        customized_credential_load: None,
    })
}

#[cfg(test)]
mod endpoint_tests {
    use super::r2_s3_endpoint_from_catalog_uri;

    #[test]
    fn derives_r2_s3_endpoint_from_catalog_uri() {
        let uri = "https://catalog.cloudflarestorage.com/abc123def456/zaned-warehouse";
        let endpoint = r2_s3_endpoint_from_catalog_uri(uri).unwrap();
        assert_eq!(endpoint, "https://abc123def456.r2.cloudflarestorage.com");
    }

    #[test]
    fn rejects_catalog_uri_without_account_hash() {
        let uri = "https://catalog.cloudflarestorage.com/";
        assert!(r2_s3_endpoint_from_catalog_uri(uri).is_err());
    }
}
