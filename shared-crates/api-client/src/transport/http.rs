//! Generic HTTP query executor. One function for every typed
//! `GraphQLQuery` — handles bearer-auth, content-type, status check,
//! and `GqlResponse` decode/error mapping in one place.

use graphql_client::{GraphQLQuery, Response as GqlResponse};
use serde::{Serialize, de::DeserializeOwned};

use crate::{ApiClient, ApiError};

/// Execute a typed GraphQL query against the configured `ApiClient`'s
/// HTTP endpoint. Returns the decoded `ResponseData` or maps any
/// transport / GraphQL / missing-data error into [`ApiError`].
pub async fn execute_query<Q>(client: &ApiClient, variables: Q::Variables) -> Result<Q::ResponseData, ApiError>
where
    Q: GraphQLQuery,
    Q::Variables: Serialize,
    Q::ResponseData: DeserializeOwned,
{
    let body = Q::build_query(variables);
    let mut req = client.http().post(client.graphql_url()).json(&body);
    if let Some(token) = client.bearer() {
        req = req.bearer_auth(token);
    }
    let resp: GqlResponse<Q::ResponseData> = req.send().await?.error_for_status()?.json().await?;
    if let Some(errs) = resp.errors {
        return Err(ApiError::Graphql(errs));
    }
    resp.data.ok_or(ApiError::NoData)
}
