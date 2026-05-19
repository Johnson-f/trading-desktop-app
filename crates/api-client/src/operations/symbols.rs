//! Symbol search — fuzzy lookup against the Typesense `tickers`
//! collection (case-insensitive prefix + 1-typo tolerance over both
//! ticker symbol and long company name).

use graphql_client::GraphQLQuery;

use crate::transport::execute_query;
use crate::{ApiClient, ApiError};

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "queries/search_symbols.graphql",
    response_derives = "Debug, Clone"
)]
pub struct SearchSymbolsQuery;

/// One symbol-search result. Re-exported at the crate root.
pub type Symbol = search_symbols_query::SearchSymbolsQuerySearchSymbols;

impl ApiClient {
    /// Fuzzy-search the symbol index. `limit` defaults to 20
    /// server-side, clamped to [1, 50].
    pub async fn search_symbols(
        &self,
        q: impl Into<String>,
        limit: Option<i32>,
    ) -> Result<Vec<Symbol>, ApiError> {
        let data = execute_query::<SearchSymbolsQuery>(
            self,
            search_symbols_query::Variables {
                q: q.into(),
                limit: limit.map(|n| n as i64),
            },
        )
        .await?;
        Ok(data.search_symbols)
    }
}
