//! GraphQL mutation root. Currently empty save for a no-op placeholder
//! (GraphQL does not allow an Object with zero fields). Real mutations
//! join here as they're built.

use async_graphql::Object;

pub struct MutationRoot;

#[Object]
impl MutationRoot {
    /// No-op. Returns `true` always. Exists only so the schema is well-formed.
    async fn ping(&self) -> bool {
        true
    }
}
