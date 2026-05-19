//! GraphQL queries for historical OHLCV bars.

use std::sync::Arc;
use std::time::Instant;

use async_graphql::{Context, Object};
use chrono::{DateTime, Utc};

use super::types::{BucketInput, HistoricalBar};
use crate::service::historical_service::{BucketSpec, ClickhouseReader, validate_symbol};

#[derive(Default)]
pub struct HistoricalQuery;

#[Object]
impl HistoricalQuery {
    /// Aggregated OHLCV bars for `symbol` in `[from, to)`, bucketed
    /// per `bucket`. Sub-day buckets read `bars_1m`; day-and-above
    /// read `bars_1d`. If the underlying range produces more than
    /// `limit` bars, the OLDEST are dropped (the response always ends
    /// at or before `to`). Server clamps `limit` to [1, 5000].
    async fn bars(
        &self,
        ctx: &Context<'_>,
        symbol: String,
        bucket: BucketInput,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        limit: Option<i32>,
    ) -> async_graphql::Result<Vec<HistoricalBar>> {
        // Validation order: symbol → range → bucket → limit clamp.
        validate_symbol(&symbol)
            .map_err(|e| async_graphql::Error::new(format!("Invalid argument: {e}")))?;
        if from >= to {
            return Err(async_graphql::Error::new(
                "Invalid argument: from must be < to",
            ));
        }
        let spec = BucketSpec::new(bucket.unit.into(), bucket.count)
            .map_err(|e| async_graphql::Error::new(format!("Invalid argument: {e}")))?;
        let limit = limit.unwrap_or(5000).clamp(1, 5000) as u32;

        let reader = ctx.data::<Arc<ClickhouseReader>>()?;
        let started = Instant::now();
        let rows = reader
            .fetch_bars(&symbol, spec, from, to, limit)
            .await
            .map_err(|e| {
                tracing::warn!(error = ?e, symbol = %symbol, "bars query failed");
                async_graphql::Error::new("Internal error")
            })?;

        let elapsed_ms = started.elapsed().as_millis() as u64;
        tracing::info!(
            symbol = %symbol,
            unit = ?spec.unit,
            count = spec.count,
            from = %from,
            to = %to,
            bucket_count_returned = rows.len(),
            elapsed_ms,
            source_table = spec.source_table(),
            "bars resolved"
        );

        Ok(rows
            .into_iter()
            .map(|b| HistoricalBar {
                ts: b.ts,
                open: b.open,
                high: b.high,
                low: b.low,
                close: b.close,
                volume: b.volume as i64,
            })
            .collect())
    }
}
