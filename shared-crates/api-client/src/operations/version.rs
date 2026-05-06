//! `query { version, health }` — server reachability + build version
//! sanity check.

use graphql_client::GraphQLQuery;

use crate::transport::execute_query;
use crate::{ApiClient, ApiError};

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "queries/version.graphql",
    response_derives = "Debug, Clone"
)]
pub struct VersionQuery;

impl ApiClient {
    /// Sanity check: returns `(version, health)`.
    pub async fn version(&self) -> Result<(String, String), ApiError> {
        let data = execute_query::<VersionQuery>(self, version_query::Variables {}).await?;
        Ok((data.version, data.health))
    }
}
