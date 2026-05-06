//! Top-level GraphQL query root. Composes per-subsystem query structs
//! (currently `GenericQuery` for liveness/version, `SymbolQuery` for
//! symbol search, and `HistoricalQuery` for OHLCV bars) via
//! `async-graphql`'s `MergedObject` derive.

use async_graphql::{MergedObject, Object};

use super::historical::HistoricalQuery;
use super::symbol::SymbolQuery;

#[derive(MergedObject, Default)]
pub struct QueryRoot(GenericQuery, SymbolQuery, HistoricalQuery);

#[derive(Default)]
struct GenericQuery;

#[Object]
impl GenericQuery {
    /// Server build version — pulled from the Cargo package version at
    /// compile time so each release ships its own tag.
    async fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    /// Liveness probe via GraphQL.
    async fn health(&self) -> &'static str {
        "ok"
    }
}
