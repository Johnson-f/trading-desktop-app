//! Runtime configuration loaded from the process environment. The
//! gateway loads `.env` (via the gateway binary, not this crate) before
//! calling `Config::from_env`.

use std::collections::HashMap;

use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct Config {
    /// Iceberg REST catalog endpoint (read from the Cloudflare dashboard
    /// after `wrangler r2 bucket catalog enable`).
    pub catalog_uri: String,
    /// Warehouse identifier inside the catalog. Cloudflare formats this
    /// as `<account_hash>_<bucket_name>`.
    pub warehouse_name: String,
    /// R2 API token with Admin Read & Write permissions.
    pub token: String,
    /// Iceberg namespace under which the `bars_1d` / `bars_1m` tables live.
    /// Created on first run if absent.
    pub namespace: String,
    /// UTC hour of day to start each backfill cycle.
    pub schedule_hour_utc: u32,
    /// Bounded concurrency for Yahoo HTTP fetches.
    pub yahoo_workers: usize,
}

impl Config {
    /// Read configuration from the current process environment.
    pub fn from_env() -> Result<Self> {
        let map: HashMap<String, String> = std::env::vars().collect();
        Self::from_map(&map)
    }

    /// Parse configuration from an arbitrary key→value map. Pure function;
    /// unit-testable without mutating process-global state.
    pub fn from_map(map: &HashMap<String, String>) -> Result<Self> {
        let catalog_uri = map
            .get("WAREHOUSE_CATALOG_URI")
            .cloned()
            .context("WAREHOUSE_CATALOG_URI not set")?;
        let warehouse_name = map
            .get("WAREHOUSE_NAME")
            .cloned()
            .context("WAREHOUSE_NAME not set")?;
        let token = map
            .get("WAREHOUSE_TOKEN")
            .cloned()
            .context("WAREHOUSE_TOKEN not set")?;
        let namespace = map
            .get("WAREHOUSE_NAMESPACE")
            .cloned()
            .unwrap_or_else(|| "market_data".to_string());
        let schedule_hour_utc = map
            .get("SCHEDULE_HOUR_UTC")
            .and_then(|s| s.parse().ok())
            .unwrap_or(2);
        let yahoo_workers = map
            .get("YAHOO_WORKERS")
            .and_then(|s| s.parse().ok())
            .unwrap_or(8);
        Ok(Self {
            catalog_uri,
            warehouse_name,
            token,
            namespace,
            schedule_hour_utc,
            yahoo_workers,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map<const N: usize>(entries: [(&str, &str); N]) -> HashMap<String, String> {
        entries
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn from_map_loads_required_vars_and_defaults() {
        let cfg = Config::from_map(&map([
            ("WAREHOUSE_CATALOG_URI", "https://catalog.example/abc/wh"),
            ("WAREHOUSE_NAME", "abc_wh"),
            ("WAREHOUSE_TOKEN", "tok"),
        ]))
        .unwrap();
        assert_eq!(cfg.catalog_uri, "https://catalog.example/abc/wh");
        assert_eq!(cfg.warehouse_name, "abc_wh");
        assert_eq!(cfg.token, "tok");
        assert_eq!(cfg.namespace, "market_data");
        assert_eq!(cfg.schedule_hour_utc, 2);
        assert_eq!(cfg.yahoo_workers, 8);
    }

    #[test]
    fn from_map_honors_overrides() {
        let cfg = Config::from_map(&map([
            ("WAREHOUSE_CATALOG_URI", "u"),
            ("WAREHOUSE_NAME", "w"),
            ("WAREHOUSE_TOKEN", "t"),
            ("WAREHOUSE_NAMESPACE", "custom_ns"),
            ("SCHEDULE_HOUR_UTC", "14"),
            ("YAHOO_WORKERS", "16"),
        ]))
        .unwrap();
        assert_eq!(cfg.namespace, "custom_ns");
        assert_eq!(cfg.schedule_hour_utc, 14);
        assert_eq!(cfg.yahoo_workers, 16);
    }

    #[test]
    fn from_map_errors_when_token_missing() {
        let err = Config::from_map(&map([
            ("WAREHOUSE_CATALOG_URI", "u"),
            ("WAREHOUSE_NAME", "w"),
        ]))
        .unwrap_err();
        assert!(err.to_string().contains("WAREHOUSE_TOKEN"));
    }
}
