//! Typed GraphQL client for `zaned-server`. Exposes:
//!   - `ApiClient::version()` — sanity check (server build + health)
//!   - `ApiClient::subscribe_ticks(symbols)` — live OHLCV stream
//!
//! All methods carry the bearer token set via `with_bearer`. Without one,
//! Clerk-protected routes return 401.

pub mod queries;
pub mod subscriptions;

use graphql_client::{GraphQLQuery, Response as GqlResponse};
use thiserror::Error;

use crate::queries::{VersionQuery, version_query};

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("server returned errors: {0:?}")]
    Graphql(Vec<graphql_client::Error>),
    #[error("server returned no data")]
    NoData,
    #[error("subscription error: {0}")]
    Subscription(String),
    #[error("invalid bearer token (could not build header)")]
    InvalidBearer,
    #[error("websocket error: {0}")]
    WebSocket(String),
}

#[derive(Clone)]
pub struct ApiClient {
    base_url: String,
    bearer: Option<String>,
    http: reqwest::Client,
}

impl ApiClient {
    /// `base_url` is the HTTP root, e.g. `http://localhost:8765`. The
    /// WebSocket endpoint is derived from this (replace `http` → `ws` and
    /// append `/graphql/ws`).
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            bearer: None,
            http: reqwest::Client::new(),
        }
    }

    /// Attach a Clerk JWT to every subsequent request.
    pub fn with_bearer(mut self, bearer: impl Into<String>) -> Self {
        self.bearer = Some(bearer.into());
        self
    }

    pub fn graphql_url(&self) -> String {
        format!("{}/graphql", self.base_url)
    }

    /// Subscriptions endpoint. Replaces the `http`/`https` scheme with
    /// `ws`/`wss`.
    pub fn graphql_ws_url(&self) -> String {
        let s = self
            .base_url
            .replace("https://", "wss://")
            .replace("http://", "ws://");
        format!("{}/graphql/ws", s.trim_end_matches('/'))
    }

    pub fn bearer(&self) -> Option<&str> {
        self.bearer.as_deref()
    }

    /// Sanity check: returns `(version, health)`.
    pub async fn version(&self) -> Result<(String, String), ApiError> {
        let body = VersionQuery::build_query(version_query::Variables {});
        let mut req = self.http.post(self.graphql_url()).json(&body);
        if let Some(token) = &self.bearer {
            req = req.bearer_auth(token);
        }
        let resp: GqlResponse<version_query::ResponseData> =
            req.send().await?.error_for_status()?.json().await?;
        if let Some(errs) = resp.errors {
            return Err(ApiError::Graphql(errs));
        }
        let data = resp.data.ok_or(ApiError::NoData)?;
        Ok((data.version, data.health))
    }
}
