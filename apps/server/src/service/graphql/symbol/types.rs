//! GraphQL types for symbol search results.

use async_graphql::SimpleObject;

/// One match from the Typesense `tickers` collection.
#[derive(Debug, Clone, SimpleObject)]
pub struct Symbol {
    /// Typesense document id (currently equal to `symbol` for upsert
    /// stability — but treat as opaque on the client).
    pub id: String,
    pub symbol: String,
    pub long_name: Option<String>,
    pub exchange: String,
    pub quote_type: String,
}
