//! GraphQL queries for symbol search.

use std::sync::Arc;

use async_graphql::{Context, Object};

use super::types::Symbol;
use crate::service::typesense::TypesenseClient;

#[derive(Default)]
pub struct SymbolQuery;

#[Object]
impl SymbolQuery {
    /// Fuzzy search across symbol ticker + long company name. Up to
    /// `limit` results (server clamps to [1, 50]; default 20). Empty `q`
    /// returns an empty list without hitting Typesense.
    async fn search_symbols(
        &self,
        ctx: &Context<'_>,
        q: String,
        limit: Option<i32>,
    ) -> async_graphql::Result<Vec<Symbol>> {
        let typesense = ctx.data::<Arc<TypesenseClient>>()?;
        let limit = limit.unwrap_or(20).clamp(1, 50) as u32;
        let docs = typesense
            .search(&q, limit)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
        Ok(docs
            .into_iter()
            .map(|d| Symbol {
                id: d.id,
                symbol: d.symbol,
                long_name: d.long_name,
                exchange: d.exchange,
                quote_type: d.quote_type,
            })
            .collect())
    }
}
