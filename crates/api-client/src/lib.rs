//! Typed GraphQL client for `zaned-server`.
//!
//! Layout:
//!   - [`transport`] — generic HTTP / WebSocket helpers (the only
//!     place that touches `reqwest`, bearer headers, and
//!     `graphql_ws_client` directly).
//!   - [`operations`] — one file per query/mutation domain. Each file
//!     owns the `#[derive(GraphQLQuery)]` struct AND the public
//!     wrapper method on [`ApiClient`].
//!   - [`subscriptions`] — one file per subscription, same pattern.
//!
//! Adding a new query is three steps: drop the `.graphql` file in
//! `queries/`, add a `GraphQLQuery` derive in
//! `operations/<domain>.rs`, write a thin wrapper that delegates to
//! [`transport::execute_query`].

pub mod operations;
pub mod scalars;
pub mod subscriptions;
pub mod transport;

use thiserror::Error;

pub use operations::historical::{BarUnit, BucketInput, HistoricalBar};
pub use operations::symbols::Symbol;
pub use subscriptions::ticks::{TickEvent, TicksResponse};

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
    /// WebSocket endpoint is derived from this (replace `http` → `ws`
    /// and append `/graphql/ws`).
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

    /// Subscriptions endpoint. Replaces the `http` / `https` scheme
    /// with `ws` / `wss`.
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

    /// Shared `reqwest::Client` — used by [`transport::execute_query`].
    pub(crate) fn http(&self) -> &reqwest::Client {
        &self.http
    }
}
