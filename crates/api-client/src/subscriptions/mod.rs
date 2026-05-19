//! GraphQL subscriptions, one file per stream. Each file owns the
//! `#[derive(GraphQLQuery)]` struct AND the public wrapper method on
//! `ApiClient` that opens it via [`crate::transport::open_subscription`].

pub mod ticks;
