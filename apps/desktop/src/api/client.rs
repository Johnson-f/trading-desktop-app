//! Thin wrapper around `zaned_api_client::ApiClient` that pins the
//! server URL at compile time.
//!
//! The URL is resolved from the `ZANED_SERVER_URL` env var **at build
//! time** (via `option_env!`), with a localhost fallback for dev. Set
//! it before `cargo build` to point a release at staging or prod:
//!
//! ```text
//! ZANED_SERVER_URL=https://api.zaned.example cargo build --release
//! ```
//!
//! There is no runtime configuration — flipping environments means a
//! fresh build, which matches the Tauri-style deployment story.

use zaned_api_client::ApiClient;

/// Compile-time server base URL. Override with `ZANED_SERVER_URL` at
/// build time; defaults to the local dev gateway.
pub const SERVER_URL: &str = match option_env!("ZANED_SERVER_URL") {
    Some(url) => url,
    None => "http://localhost:8765",
};

/// Build a fresh anonymous client. Use [`make_authed`] when you have a
/// Clerk JWT — bearer tokens cannot be added in place once the client
/// is constructed (the inner field is private), so this is the entry
/// point for both authed and unauthed callers.
pub fn make() -> ApiClient {
    ApiClient::new(SERVER_URL)
}

/// Build a client that attaches `bearer` to every subsequent HTTP and
/// WebSocket request. Pass the freshly-rotated Clerk access token.
pub fn make_authed(bearer: impl Into<String>) -> ApiClient {
    ApiClient::new(SERVER_URL).with_bearer(bearer)
}
