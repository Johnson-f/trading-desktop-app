//! Idempotent creation of the Iceberg namespace and bars tables. Called
//! on every gateway boot via `Warehouse::ensure_tables`.

use std::collections::HashMap;

use anyhow::{Context, Result};
use iceberg::{Catalog, NamespaceIdent, TableCreation, TableIdent};

use crate::warehouse::Table;
use crate::warehouse::schema::{iceberg_schema, partition_spec, sort_order};

pub async fn ensure_namespace(catalog: &dyn Catalog, namespace: &str) -> Result<()> {
    let ident = NamespaceIdent::new(namespace.to_string());
    match catalog.namespace_exists(&ident).await {
        Ok(true) => Ok(()),
        Ok(false) => catalog
            .create_namespace(&ident, HashMap::new())
            .await
            .map(|_| ())
            .context("create namespace"),
        Err(e) => Err(e).context("namespace_exists check"),
    }
}

pub async fn ensure_table(catalog: &dyn Catalog, namespace: &str, table: Table) -> Result<()> {
    let ident = TableIdent::new(
        NamespaceIdent::new(namespace.to_string()),
        table.name().to_string(),
    );
    if catalog
        .table_exists(&ident)
        .await
        .context("table_exists check")?
    {
        return Ok(());
    }

    let schema = iceberg_schema(table)?;
    let pspec = partition_spec(&schema, table)?.into_unbound();
    let sorder = sort_order(&schema)?;

    let creation = TableCreation::builder()
        .name(table.name().to_string())
        .schema(schema)
        .partition_spec(pspec)
        .sort_order(sorder)
        .properties([
            ("format-version".to_string(), "2".to_string()),
            (
                "write.parquet.compression-codec".to_string(),
                "zstd".to_string(),
            ),
            (
                "write.parquet.compression-level".to_string(),
                "3".to_string(),
            ),
            (
                "write.parquet.row-group-size-bytes".to_string(),
                "268435456".to_string(),
            ),
        ])
        .build();

    catalog
        .create_table(&NamespaceIdent::new(namespace.to_string()), creation)
        .await
        .map(|_| ())
        .context("create_table")
}
