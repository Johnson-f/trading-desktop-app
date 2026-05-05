//! Top-level GraphQL subscription root. Composes per-subsystem subscription
//! structs (currently just `TicksSubscription`) via `async-graphql`'s
//! `MergedSubscription` derive so each subsystem owns its resolvers.

use async_graphql::MergedSubscription;

use super::tick_service::TicksSubscription;

#[derive(MergedSubscription, Default)]
pub struct SubscriptionRoot(TicksSubscription);
