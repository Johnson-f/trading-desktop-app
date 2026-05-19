//! Historical OHLCV bars — the desktop's read path for chart data.
//! Backed by the gateway's `HistoricalQuery::bars` resolver, which
//! aggregates ClickHouse `bars_1m` / `bars_1d` into the requested
//! bucket size. Prices come back as integer cents.

use chrono::{DateTime, Utc};
use graphql_client::GraphQLQuery;

use crate::transport::execute_query;
use crate::{ApiClient, ApiError};

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "queries/bars.graphql",
    response_derives = "Debug, Clone",
    custom_scalars_module = "crate::scalars"
)]
pub struct BarsQuery;

/// One aggregated bar in the response — re-exported at the crate root
/// so call sites don't reach into the codegen module.
pub type HistoricalBar = bars_query::BarsQueryBars;
pub use bars_query::{BarUnit, BucketInput};

impl ApiClient {
    /// Fetch aggregated OHLCV bars for `symbol` in `[from, to)`.
    /// `limit` is clamped server-side to [1, 5000]; if the underlying
    /// range produces more than `limit` bars, the OLDEST are dropped.
    pub async fn historical_bars(
        &self,
        symbol: impl Into<String>,
        bucket: BucketInput,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        limit: Option<i32>,
    ) -> Result<Vec<HistoricalBar>, ApiError> {
        let data = execute_query::<BarsQuery>(
            self,
            bars_query::Variables {
                symbol: symbol.into(),
                bucket,
                from: from.to_rfc3339(),
                to: to.to_rfc3339(),
                limit: limit.map(|n| n as i64),
            },
        )
        .await?;
        Ok(data.bars)
    }
}
