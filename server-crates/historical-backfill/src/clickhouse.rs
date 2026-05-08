use std::collections::HashMap;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};

use crate::bar::Bar;

// Compression: ZSTD(3) is ~5% bigger than ZSTD(22) but compresses 30-80x faster
// — during heavy backfill the server is CPU-bound on compression, not disk-bound.
// min_bytes_for_wide_part=0 forces wide format immediately so we don't pay the
// compact->wide conversion cost on every merge during burst ingest.
// parts_to_throw_insert=600 raises the safety ceiling above the default 300 so
// we don't 503 during high-throughput backfill before merges catch up.
const CREATE_TABLE_DDL: &str = r#"
CREATE TABLE IF NOT EXISTS bars_1m
(
    symbol      LowCardinality(String),
    ts          DateTime                              CODEC(DoubleDelta, ZSTD(3)),
    open        Decimal32(2)                          CODEC(Delta(4), ZSTD(3)),
    high        Decimal32(2)                          CODEC(Delta(4), ZSTD(3)),
    low         Decimal32(2)                          CODEC(Delta(4), ZSTD(3)),
    close       Decimal32(2)                          CODEC(Delta(4), ZSTD(3)),
    volume      UInt32                                CODEC(T64, ZSTD(3)),
    version     UInt32 DEFAULT toUnixTimestamp(now()) CODEC(DoubleDelta, ZSTD(3))
)
ENGINE = ReplacingMergeTree(version)
PARTITION BY toYear(ts)
ORDER BY (symbol, ts)
SETTINGS
    index_granularity                           = 16384,
    min_bytes_for_wide_part                     = 0,
    parts_to_throw_insert                       = 600,
    enable_block_number_column                  = 0,
    enable_block_offset_column                  = 0
"#;

const CREATE_TABLE_1D_DDL: &str = r#"
CREATE TABLE IF NOT EXISTS bars_1d
(
    symbol      LowCardinality(String),
    ts          DateTime                              CODEC(DoubleDelta, ZSTD(3)),
    open        Decimal32(2)                          CODEC(Delta(4), ZSTD(3)),
    high        Decimal32(2)                          CODEC(Delta(4), ZSTD(3)),
    low         Decimal32(2)                          CODEC(Delta(4), ZSTD(3)),
    close       Decimal32(2)                          CODEC(Delta(4), ZSTD(3)),
    volume      UInt32                                CODEC(T64, ZSTD(3)),
    version     UInt32 DEFAULT toUnixTimestamp(now()) CODEC(DoubleDelta, ZSTD(3))
)
ENGINE = ReplacingMergeTree(version)
PARTITION BY toYear(ts)
ORDER BY (symbol, ts)
SETTINGS
    index_granularity                           = 16384,
    min_bytes_for_wide_part                     = 0,
    parts_to_throw_insert                       = 600,
    enable_block_number_column                  = 0,
    enable_block_offset_column                  = 0
"#;

const CREATE_SYMBOL_METADATA_DDL: &str = r#"
CREATE TABLE IF NOT EXISTS symbol_metadata
(
    symbol         LowCardinality(String),
    earliest_data  DateTime,
    discovered_at  DateTime DEFAULT now()
)
ENGINE = ReplacingMergeTree(discovered_at)
ORDER BY symbol
"#;

#[derive(Clone)]
pub struct ClickhouseClient {
    client: ::clickhouse::Client,
    url: String,
    user: String,
    password: String,
    database: String,
}

impl ClickhouseClient {
    pub fn new(url: &str, user: &str, password: &str, database: &str) -> Result<Self> {
        // No async_insert: we batch client-side via the native RowBinary inserter
        // (the daily/minute sync passes call insert_bars per symbol). Recommended
        // when batches are large enough on their own.
        let client = ::clickhouse::Client::default()
            .with_url(url)
            .with_user(user)
            .with_password(password)
            .with_database(database);
        Ok(Self {
            client,
            url: url.to_string(),
            user: user.to_string(),
            password: password.to_string(),
            database: database.to_string(),
        })
    }

    pub fn database(&self) -> &str {
        &self.database
    }

    pub async fn ensure_table(&self) -> Result<()> {
        // CREATE DATABASE must run on a connection that doesn't already have
        // the target database selected — otherwise the server tries to switch
        // to it before executing the statement and fails with UNKNOWN_DATABASE.
        let bootstrap = ::clickhouse::Client::default()
            .with_url(&self.url)
            .with_user(&self.user)
            .with_password(&self.password);
        let create_db = format!("CREATE DATABASE IF NOT EXISTS {}", self.database);
        bootstrap
            .query(&create_db)
            .execute()
            .await
            .context("CREATE DATABASE")?;

        self.client
            .query(CREATE_TABLE_DDL)
            .execute()
            .await
            .context("CREATE TABLE bars_1m")?;

        self.client
            .query(CREATE_TABLE_1D_DDL)
            .execute()
            .await
            .context("CREATE TABLE bars_1d")?;

        self.client
            .query(CREATE_SYMBOL_METADATA_DDL)
            .execute()
            .await
            .context("CREATE TABLE symbol_metadata")?;
        Ok(())
    }

    /// Load the cached earliest-data date per symbol from `symbol_metadata`.
    /// Symbols not yet probed are absent from the result.
    pub async fn load_symbol_metadata(&self) -> Result<HashMap<String, DateTime<Utc>>> {
        #[derive(::clickhouse::Row, serde::Deserialize)]
        struct Row {
            symbol: String,
            #[serde(with = "::clickhouse::serde::chrono::datetime")]
            earliest_data: DateTime<Utc>,
        }
        let rows = self
            .client
            .query("SELECT symbol, earliest_data FROM symbol_metadata FINAL")
            .fetch_all::<Row>()
            .await
            .context("load symbol_metadata")?;
        Ok(rows
            .into_iter()
            .map(|r| (r.symbol, r.earliest_data))
            .collect())
    }

    /// Persist newly-discovered earliest-data dates. Idempotent — uses the
    /// ReplacingMergeTree's `discovered_at` to dedupe re-probes.
    pub async fn upsert_symbol_metadata(&self, rows: &[(String, DateTime<Utc>)]) -> Result<()> {
        if rows.is_empty() {
            return Ok(());
        }
        #[derive(::clickhouse::Row, serde::Serialize)]
        struct Row<'a> {
            symbol: &'a str,
            #[serde(with = "::clickhouse::serde::chrono::datetime")]
            earliest_data: DateTime<Utc>,
            #[serde(with = "::clickhouse::serde::chrono::datetime")]
            discovered_at: DateTime<Utc>,
        }
        let now = Utc::now();
        let mut insert = self
            .client
            .insert::<Row>("symbol_metadata")
            .await
            .context("create symbol_metadata insert")?;
        for (symbol, earliest_data) in rows {
            insert
                .write(&Row {
                    symbol,
                    earliest_data: *earliest_data,
                    discovered_at: now,
                })
                .await
                .context("write symbol_metadata row")?;
        }
        insert.end().await.context("flush symbol_metadata")?;
        Ok(())
    }

    pub async fn high_water_marks(
        &self,
        table: &str,
        symbols: &[String],
    ) -> Result<HashMap<String, DateTime<Utc>>> {
        if symbols.is_empty() {
            return Ok(HashMap::new());
        }

        let in_list = symbols
            .iter()
            .map(|s| format!("'{}'", s.replace('\'', "''")))
            .collect::<Vec<_>>()
            .join(",");

        let sql = format!(
            "SELECT symbol, max(ts) AS hwm FROM {table} WHERE symbol IN ({in_list}) GROUP BY symbol"
        );

        #[derive(::clickhouse::Row, serde::Deserialize)]
        struct Row {
            symbol: String,
            #[serde(with = "::clickhouse::serde::chrono::datetime")]
            hwm: DateTime<Utc>,
        }

        let rows = self
            .client
            .query(&sql)
            .fetch_all::<Row>()
            .await
            .context("high_water_marks query")?;

        Ok(rows.into_iter().map(|r| (r.symbol, r.hwm)).collect())
    }

    pub async fn insert_bars(&self, table: &str, bars: &[Bar]) -> Result<usize> {
        if bars.is_empty() {
            return Ok(0);
        }
        let mut insert = self
            .client
            .insert::<Bar>(table)
            .await
            .context("create insert handle")?;
        for bar in bars {
            insert.write(bar).await.context("write bar")?;
        }
        insert.end().await.context("flush insert")?;
        Ok(bars.len())
    }
}
