//! Token shapes shared across the auth module.
//!
//! `TokenSet` is what we persist to the OS keychain and what the refresh
//! worker holds in memory. Access tokens are JWTs signed by Clerk; refresh
//! tokens are opaque strings.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// One coherent set of tokens from a Clerk token endpoint response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenSet {
    pub access_token: String,
    /// Some Clerk responses (refresh) may omit a new refresh token; we
    /// fall back to the previous one. Stored as `Option` so `from_response`
    /// can preserve continuity.
    pub refresh_token: Option<String>,
    /// Absolute expiry. We refresh proactively at 80% of remaining lifetime.
    pub expires_at: DateTime<Utc>,
}

/// Wire shape of Clerk's `/oauth/token` JSON response.
#[derive(Debug, Clone, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: Option<String>,
    /// Lifetime in seconds (RFC 6749).
    pub expires_in: i64,
}

impl TokenSet {
    /// Build a new `TokenSet` from a fresh token endpoint response. If the
    /// server returns no `refresh_token`, carry forward the previous one.
    pub fn from_response(resp: TokenResponse, prev_refresh: Option<String>) -> Self {
        let expires_at = Utc::now() + chrono::Duration::seconds(resp.expires_in);
        Self {
            access_token: resp.access_token,
            refresh_token: resp.refresh_token.or(prev_refresh),
            expires_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_response_carries_forward_prev_refresh_when_missing() {
        let resp = TokenResponse {
            access_token: "new_access".into(),
            refresh_token: None,
            expires_in: 7200,
        };
        let set = TokenSet::from_response(resp, Some("prev_refresh".into()));
        assert_eq!(set.access_token, "new_access");
        assert_eq!(set.refresh_token.as_deref(), Some("prev_refresh"));
    }

    #[test]
    fn from_response_uses_new_refresh_when_present() {
        let resp = TokenResponse {
            access_token: "new_access".into(),
            refresh_token: Some("new_refresh".into()),
            expires_in: 7200,
        };
        let set = TokenSet::from_response(resp, Some("prev_refresh".into()));
        assert_eq!(set.refresh_token.as_deref(), Some("new_refresh"));
    }
}
