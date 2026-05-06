//! GraphQL queries and mutations, one file per logical domain. Each
//! file owns the `#[derive(GraphQLQuery)]` struct AND the public
//! wrapper method on `ApiClient` that calls it via
//! [`crate::transport::execute_query`].

pub mod symbols;
pub mod version;
