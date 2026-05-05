//! Build the GraphQL `Schema` once at boot, attaching shared state
//! (Redis handle, command channel, redis URL) as data extensions. The
//! `Schema` is `Clone`able and shared across all axum requests.

use std::sync::Arc;

use async_graphql::Schema;
use tokio::sync::mpsc;

use super::mutation::MutationRoot;
use super::query::QueryRoot;
use super::subscription::SubscriptionRoot;
use super::tick_service::SubscriptionContext;
use crate::service::tick_service::coordinator::CoordCmd;
use crate::service::tick_service::redis_state::RedisState;

pub type AppSchema = Schema<QueryRoot, MutationRoot, SubscriptionRoot>;

pub fn build_schema(
    redis: Arc<RedisState>,
    redis_url: String,
    cmd_tx: mpsc::Sender<CoordCmd>,
) -> AppSchema {
    let sub_ctx = SubscriptionContext { redis, redis_url, cmd_tx };
    Schema::build(QueryRoot, MutationRoot, SubscriptionRoot::default())
        .data(sub_ctx)
        .finish()
}
