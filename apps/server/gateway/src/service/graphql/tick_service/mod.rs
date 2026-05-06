//! Tick-service GraphQL surface: types and subscription resolvers for
//! real-time market data streaming.

pub mod subscription;
pub mod types;

pub use subscription::{SubscriptionContext, TicksSubscription};
