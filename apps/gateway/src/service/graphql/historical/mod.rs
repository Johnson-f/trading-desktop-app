//! GraphQL surface for historical bar aggregation. Backed by the
//! ClickHouse reader at `crate::service::historical_service`.

pub mod query;
pub mod types;

pub use query::HistoricalQuery;
