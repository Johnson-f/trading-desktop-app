//! GraphQL query root. Currently exposes liveness/version checks; new
//! queries (user profile, watchlists, etc.) get added here as they land.

use async_graphql::Object;

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    /// Server build version — pulled from the Cargo package version at
    /// compile time so each release ships its own tag.
    async fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    /// Liveness probe via GraphQL. The unauthenticated `/health` REST
    /// route is preserved separately for load balancers / monitoring that
    /// don't speak GraphQL — this query exists so an authenticated client
    /// can confirm the schema responds without crafting a separate HTTP
    /// request.
    async fn health(&self) -> &'static str {
        "ok"
    }
}
