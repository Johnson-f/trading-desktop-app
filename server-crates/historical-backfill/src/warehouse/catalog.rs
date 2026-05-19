//! Build an Iceberg REST catalog client pointed at R2 Data Catalog
//! (or any standard-compliant Iceberg REST server during testing).

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context, Result};
use iceberg::CatalogBuilder;
use iceberg::io::StorageFactory;
use iceberg_catalog_rest::{
    REST_CATALOG_PROP_URI, REST_CATALOG_PROP_WAREHOUSE, RestCatalog, RestCatalogBuilder,
};

use crate::config::Config;

/// Property key for the bearer token. The iceberg-rust REST client reads
/// `"token"` from the props map and uses it as an OAuth2 bearer credential
/// (see `RestCatalogConfig::token` in iceberg-catalog-rest 0.9.1).
/// No public constant is exported by the crate for this key, so we define
/// our own to avoid a bare string literal.
const REST_CATALOG_PROP_TOKEN: &str = "token";

/// Build the REST catalog. `extra_props` are merged into the user-level
/// properties and flow through to every `FileIO` the catalog creates
/// (e.g. `s3.endpoint`, `s3.region` for the R2 storage factory).
pub async fn build_rest_catalog(
    cfg: &Config,
    storage_factory: Arc<dyn StorageFactory>,
    extra_props: HashMap<String, String>,
) -> Result<RestCatalog> {
    let mut props = HashMap::from([
        (REST_CATALOG_PROP_URI.to_string(), cfg.catalog_uri.clone()),
        (
            REST_CATALOG_PROP_WAREHOUSE.to_string(),
            cfg.warehouse_name.clone(),
        ),
        (REST_CATALOG_PROP_TOKEN.to_string(), cfg.token.clone()),
    ]);
    props.extend(extra_props);
    RestCatalogBuilder::default()
        .with_storage_factory(storage_factory)
        .load("rest", props)
        .await
        .context("build Iceberg REST catalog")
}
