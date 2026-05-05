//! Compile-time-checked GraphQL query/subscription structs. Each `.graphql`
//! file under `crates/api-client/queries/` paired with a Rust struct here
//! produces typed request/response shapes via the `graphql_client` derive.

use graphql_client::GraphQLQuery;

/// `query { version, health }` — smoke test that confirms server reachable
/// and returns its build version.
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "queries/version.graphql",
    response_derives = "Debug, Clone"
)]
pub struct VersionQuery;

/// `subscription { ticks(symbols) { ... } }` — live OHLCV stream.
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "queries/ticks_subscription.graphql",
    response_derives = "Debug, Clone"
)]
pub struct TicksSubscription;
