//! GraphQL API: queries, mutations, and subscriptions over HTTP at
//! `/graphql` and WebSocket at `/graphql/ws`. Tick streaming runs through
//! the `ticks(symbols)` subscription, replacing the bespoke `/ws` JSON
//! protocol once desktop clients have migrated.
//!
//! Each subsystem owns its types and resolvers under a feature-named
//! submodule (e.g. `tick_service`). The top-level `SubscriptionRoot`
//! composes them via `MergedSubscription`.

pub mod mutation;
pub mod query;
pub mod schema;
pub mod subscription;
pub mod tick_service;

use async_graphql::http::GraphiQLSource;
use async_graphql_axum::{GraphQL, GraphQLSubscription};
use axum::Router;
use axum::response::{Html, IntoResponse};
use axum::routing::get;

pub use schema::{AppSchema, build_schema};

/// Build the axum sub-router exposing GraphQL.
pub fn router(schema: AppSchema) -> Router {
    Router::new()
        .route(
            "/graphql",
            get(graphiql_redirect).post_service(GraphQL::new(schema.clone())),
        )
        .route_service("/graphql/ws", GraphQLSubscription::new(schema))
        .route("/graphiql", get(graphiql))
}

async fn graphiql() -> impl IntoResponse {
    Html(
        GraphiQLSource::build()
            .endpoint("/graphql")
            .subscription_endpoint("/graphql/ws")
            .finish(),
    )
}

async fn graphiql_redirect() -> impl IntoResponse {
    axum::response::Redirect::temporary("/graphiql")
}
