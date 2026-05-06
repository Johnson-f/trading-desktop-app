//! ClickHouse read client for the `bars_1m` and `bars_1d` tables.
//! Single async method `fetch_bars` runs the parameterized aggregation
//! query and returns rows in ascending `ts` order.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use clickhouse::Row;
use serde::Deserialize;

use super::interval::BucketSpec;
use super::query::build_select_sql;

/// One aggregated bar. Decimal32(2) prices arrive as `i32` cents on the
/// wire (the storage schema's `Decimal32(2)` decodes to `i32` — the
/// matching shape from the writer side lives in
/// `apps/historical-service/src/bar.rs`).
#[derive(Debug, Clone, PartialEq, Row, Deserialize)]
pub struct Bar {
    #[serde(with = "::clickhouse::serde::chrono::datetime")]
    pub ts: DateTime<Utc>,
    pub open: i32,
    pub high: i32,
    pub low: i32,
    pub close: i32,
    pub volume: u32,
}

#[derive(Clone)]
pub struct ClickhouseReader {
    client: clickhouse::Client,
}

impl ClickhouseReader {
    pub fn new(url: &str, user: &str, password: &str, database: &str) -> Result<Self> {
        let client = clickhouse::Client::default()
            .with_url(url)
            .with_user(user)
            .with_password(password)
            .with_database(database);
        Ok(Self { client })
    }

    /// Fetch aggregated bars in `[from, to)` for `symbol`, bucketed per
    /// `bucket`, capped at `limit` rows. Truncation drops the OLDEST
    /// rows (response includes the most recent `limit` buckets); the
    /// returned Vec is reversed to ASC by ts before returning.
    pub async fn fetch_bars(
        &self,
        symbol: &str,
        bucket: BucketSpec,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        limit: u32,
    ) -> Result<Vec<Bar>> {
        let sql = build_select_sql(&bucket);
        let mut rows = self
            .client
            .query(&sql)
            .bind(symbol)
            .bind(from)
            .bind(to)
            .bind(limit)
            .fetch_all::<Bar>()
            .await
            .context("clickhouse fetch_bars query")?;
        // Query returns DESC; flip to ASC for charting.
        rows.reverse();
        Ok(rows)
    }
}
