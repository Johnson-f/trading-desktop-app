//! Live `ticks(symbols)` subscription — server-pushed OHLCV updates.

use futures::Stream;
use graphql_client::GraphQLQuery;

use crate::transport::open_subscription;
use crate::{ApiClient, ApiError};

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "queries/ticks_subscription.graphql",
    response_derives = "Debug, Clone"
)]
pub struct TicksSubscription;

/// Top-level subscription response — `{ ticks: TickEvent }`.
pub type TicksResponse = ticks_subscription::ResponseData;
/// One per-symbol tick event yielded by the stream.
pub type TickEvent = ticks_subscription::TicksSubscriptionTicks;

impl ApiClient {
    /// Open a WebSocket subscription for the given symbols. Returns a
    /// stream that yields one [`TicksResponse`] per server frame; ends
    /// when the server closes or the underlying socket errors.
    pub async fn subscribe_ticks(
        &self,
        symbols: Vec<String>,
    ) -> Result<impl Stream<Item = Result<TicksResponse, ApiError>>, ApiError> {
        open_subscription::<TicksSubscription>(self, ticks_subscription::Variables { symbols }).await
    }
}
