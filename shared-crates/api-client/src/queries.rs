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

/// `query searchSymbols(q, limit)` — fuzzy search the Typesense ticker
/// index for symbols matching `q` (case-insensitive prefix + 1-typo
/// tolerance over both ticker symbol and long company name).
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "queries/search_symbols.graphql",
    response_derives = "Debug, Clone"
)]
pub struct SearchSymbolsQuery;
