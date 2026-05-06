//! Live `ticks(symbols)` subscription helper. Returns a `Stream` of
//! `TickEvent`s decoded from `graphql-transport-ws` frames.
//!
//! Auth: the Clerk JWT is sent as an `Authorization: Bearer ...` HTTP header
//! on the WebSocket upgrade request; the server's Clerk middleware validates
//! it on the upgrade itself.

use futures::{Stream, StreamExt};
use graphql_ws_client::graphql::StreamingOperation;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

use crate::queries::{TicksSubscription, ticks_subscription};
use crate::{ApiClient, ApiError};

/// Re-export the generated `TicksSubscription`-related types so callers
/// can match on `__typename` without importing the inner module path.
pub use ticks_subscription::ResponseData as TicksResponse;
pub use ticks_subscription::TicksSubscriptionTicks as TickEvent;

impl ApiClient {
    /// Open a WebSocket subscription for the given symbols. Returns a
    /// stream that yields one `TickEvent` per server frame; ends when the
    /// server closes or the underlying socket errors.
    pub async fn subscribe_ticks(
        &self,
        symbols: Vec<String>,
    ) -> Result<impl Stream<Item = Result<TicksResponse, ApiError>>, ApiError> {
        let ws_url = self.graphql_ws_url();
        let mut request = ws_url
            .into_client_request()
            .map_err(|e| ApiError::WebSocket(e.to_string()))?;

        if let Some(token) = &self.bearer {
            let value = http::HeaderValue::from_str(&format!("Bearer {token}"))
                .map_err(|_| ApiError::InvalidBearer)?;
            request.headers_mut().insert("Authorization", value);
        }

        // graphql-transport-ws subprotocol header
        request.headers_mut().insert(
            "Sec-WebSocket-Protocol",
            http::HeaderValue::from_static("graphql-transport-ws"),
        );

        let (socket, _resp) = tokio_tungstenite::connect_async(request)
            .await
            .map_err(|e| ApiError::WebSocket(e.to_string()))?;

        let (client, actor) = graphql_ws_client::Client::build(socket)
            .await
            .map_err(|e| ApiError::WebSocket(e.to_string()))?;
        tokio::spawn(actor.into_future());

        let op = StreamingOperation::<TicksSubscription>::new(ticks_subscription::Variables {
            symbols,
        });
        let stream = client
            .subscribe(op)
            .await
            .map_err(|e| ApiError::WebSocket(e.to_string()))?;

        Ok(stream.map(|item| match item {
            Ok(resp) => match (resp.data, resp.errors) {
                (_, Some(errs)) if !errs.is_empty() => Err(ApiError::Graphql(errs)),
                (Some(data), _) => Ok(data),
                (None, _) => Err(ApiError::NoData),
            },
            Err(e) => Err(ApiError::WebSocket(e.to_string())),
        }))
    }
}
