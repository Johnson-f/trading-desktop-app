//! Clerk environment configuration loaded from env vars or a `.env` file
//! beside the running binary. Fails closed if either var is missing — the
//! app won't boot without auth configured.

use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct ClerkConfig {
    /// e.g. `https://well-hagfish-71.clerk.accounts.dev` — the issuer for
    /// your Clerk instance. Authorize endpoint is `{issuer}/oauth/authorize`,
    /// token endpoint `{issuer}/oauth/token`.
    pub issuer: String,
    /// The OAuth Application's Client ID from Clerk dashboard. Public
    /// clients (PKCE) don't have a client secret.
    pub client_id: String,
}

impl ClerkConfig {
    pub fn from_env() -> Result<Self> {
        let issuer = std::env::var("CLERK_ISSUER")
            .context("CLERK_ISSUER is required (e.g. https://<instance>.clerk.accounts.dev)")?;
        let client_id = std::env::var("CLERK_CLIENT_ID")
            .context("CLERK_CLIENT_ID is required (Clerk dashboard → OAuth Applications)")?;
        if issuer.trim().is_empty() || client_id.trim().is_empty() {
            anyhow::bail!("CLERK_ISSUER and CLERK_CLIENT_ID must both be non-empty");
        }
        Ok(Self { issuer, client_id })
    }

    pub fn authorize_url(&self) -> String {
        format!("{}/oauth/authorize", self.issuer.trim_end_matches('/'))
    }

    pub fn token_url(&self) -> String {
        format!("{}/oauth/token", self.issuer.trim_end_matches('/'))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_endpoints_with_trailing_slash() {
        let cfg = ClerkConfig {
            issuer: "https://x.clerk.accounts.dev/".to_string(),
            client_id: "pk_test".to_string(),
        };
        assert_eq!(cfg.authorize_url(), "https://x.clerk.accounts.dev/oauth/authorize");
        assert_eq!(cfg.token_url(), "https://x.clerk.accounts.dev/oauth/token");
    }

    #[test]
    fn builds_endpoints_without_trailing_slash() {
        let cfg = ClerkConfig {
            issuer: "https://x.clerk.accounts.dev".to_string(),
            client_id: "pk_test".to_string(),
        };
        assert_eq!(cfg.authorize_url(), "https://x.clerk.accounts.dev/oauth/authorize");
    }
}
