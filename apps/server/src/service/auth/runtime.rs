//! Build the Clerk JWT verification layer.
//!
//! The Clerk SDK fetches the public JWKS for your Clerk instance and caches
//! it in memory. Each incoming request with an `Authorization: Bearer <jwt>`
//! header is validated against those keys; failures short-circuit with a
//! 401 before reaching downstream handlers (including the `/ws` upgrade).

use anyhow::{Context, Result};
use clerk_rs::ClerkConfiguration;
use clerk_rs::clerk::Clerk;
use clerk_rs::validators::axum::ClerkLayer;
use clerk_rs::validators::jwks::MemoryCacheJwksProvider;

/// Build a Clerk auth `tower::Layer` configured against the given secret key.
/// Apply with `Router::layer(...)` to any subtree that should be protected.
pub fn build_layer(secret_key: &str) -> Result<ClerkLayer<MemoryCacheJwksProvider>> {
    if secret_key.trim().is_empty() {
        anyhow::bail!("CLERK_SECRET_KEY is empty");
    }
    let config = ClerkConfiguration::new(None, None, Some(secret_key.to_string()), None);
    let clerk = Clerk::new(config);
    let provider = MemoryCacheJwksProvider::new(clerk);
    // (provider, allowed_origins, validate_session)
    //   - allowed_origins = None  → no origin allow-list (server is not browser-facing)
    //   - validate_session = true → require an active Clerk session, not just a valid JWT
    Ok(ClerkLayer::new(provider, None, true))
}

/// Read `CLERK_SECRET_KEY` from the environment, returning a configured
/// layer. Fails closed if the env var is missing — production should never
/// boot without auth.
pub fn from_env() -> Result<ClerkLayer<MemoryCacheJwksProvider>> {
    let key = std::env::var("CLERK_SECRET_KEY")
        .context("CLERK_SECRET_KEY is required (get it from Clerk dashboard → API Keys)")?;
    build_layer(&key)
}
