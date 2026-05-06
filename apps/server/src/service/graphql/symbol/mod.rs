//! Symbol search GraphQL surface. Backed by Typesense (read-only client
//! lives at `crate::service::typesense`); writes are owned by
//! `apps/symbol-service`.

pub mod query;
pub mod types;

pub use query::SymbolQuery;
