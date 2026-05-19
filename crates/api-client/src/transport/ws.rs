//! Generic WebSocket subscription opener. One function for every
//! typed `GraphQLQuery` subscription — handles upgrade, bearer auth,
//! `graphql-transport-ws` subprotocol, actor spawn, and the per-frame
//! data/error mapping into [`ApiError`].

use futures::{Stream, StreamExt};
use graphql_client::GraphQLQuery;
use graphql_ws_client::graphql::StreamingOperation;
use serde::{Serialize, de::DeserializeOwned};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

use crate::{ApiClient, ApiError};

/// Open a typed subscription and return a `Stream` of decoded
/// `ResponseData` frames. The stream ends when the server closes or
/// the underlying socket errors.
pub async fn open_subscription<S>(
    client: &ApiClient,
    variables: S::Variables,
) -> Result<impl Stream<Item = Result<S::ResponseData, ApiError>>, ApiError>
where
    S: GraphQLQuery + Send + Sync + Unpin + 'static,
    S::Variables: Serialize + Send + Unpin,
    S::ResponseData: DeserializeOwned + Send + Unpin + 'static,
{
    let mut request = client
        .graphql_ws_url()
        .into_client_request()
        .map_err(|e| ApiError::WebSocket(e.to_string()))?;

    if let Some(token) = client.bearer() {
        let value = http::HeaderValue::from_str(&format!("Bearer {token}"))
            .map_err(|_| ApiError::InvalidBearer)?;
        request.headers_mut().insert("Authorization", value);
    }
    request.headers_mut().insert(
        "Sec-WebSocket-Protocol",
        http::HeaderValue::from_static("graphql-transport-ws"),
    );

    let (socket, _resp) = tokio_tungstenite::connect_async(request)
        .await
        .map_err(|e| ApiError::WebSocket(e.to_string()))?;

    let (gql_client, actor) = graphql_ws_client::Client::build(socket)
        .await
        .map_err(|e| ApiError::WebSocket(e.to_string()))?;
    tokio::spawn(actor.into_future());

    let op = StreamingOperation::<S>::new(variables);
    let stream = gql_client
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
