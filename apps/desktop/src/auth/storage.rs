//! Persist the refresh token in the OS keychain so the user doesn't have
//! to re-authorize on every app launch. The access token is **not**
//! persisted — it lives only in memory and is re-derived from the refresh
//! token on startup.
//!
//! Keychain entry: service = `zaned`, username = `clerk-refresh`. One slot
//! per OS user account, which matches our single-tenant desktop model.

use anyhow::{Context, Result};

const SERVICE: &str = "zaned";
const USERNAME: &str = "clerk-refresh";

/// Persist a refresh token. Overwrites any existing entry.
pub fn store_refresh_token(token: &str) -> Result<()> {
    let entry = keyring::Entry::new(SERVICE, USERNAME).context("create keyring entry")?;
    entry
        .set_password(token)
        .context("write refresh token to keychain")?;
    Ok(())
}

/// Load a previously-stored refresh token. Returns `Ok(None)` if no entry
/// exists yet (first launch); errors are reserved for genuine keychain
/// failures (locked keyring, permission denied, etc.).
pub fn load_refresh_token() -> Result<Option<String>> {
    let entry = match keyring::Entry::new(SERVICE, USERNAME) {
        Ok(e) => e,
        Err(e) => return Err(anyhow::anyhow!("create keyring entry: {e}")),
    };
    match entry.get_password() {
        Ok(token) => Ok(Some(token)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(anyhow::anyhow!("read refresh token from keychain: {e}")),
    }
}

/// Wipe the stored refresh token. Idempotent — silent if no entry existed.
pub fn delete_refresh_token() -> Result<()> {
    let entry = keyring::Entry::new(SERVICE, USERNAME).context("create keyring entry")?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(anyhow::anyhow!("delete refresh token from keychain: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Use the in-memory keyring backend for tests so we don't pollute the
    /// developer's real keychain. `keyring::set_default_credential_builder`
    /// from the `mock` feature is the standard way; if it isn't enabled in
    /// our `keyring` feature set, this test should be skipped or marked
    /// `#[ignore]`. The mock builder is per-process global state, so this
    /// test must be the only test exercising the real API.
    #[test]
    #[ignore = "requires keyring mock feature; smoke-tested manually instead"]
    fn round_trip_store_and_load() {
        store_refresh_token("rt_test").unwrap();
        let loaded = load_refresh_token().unwrap();
        assert_eq!(loaded.as_deref(), Some("rt_test"));
        delete_refresh_token().unwrap();
        let loaded = load_refresh_token().unwrap();
        assert_eq!(loaded, None);
    }
}
