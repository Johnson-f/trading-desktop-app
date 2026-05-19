//! Network / data-fetching layer for the desktop app.
//!
//! Each submodule owns one piece of the bridge between the egui UI loop
//! and the gateway (or an external CDN treated the same way):
//!
//!   - [`client`]        — compile-time `ZANED_SERVER_URL` + thin
//!                         constructors over `api_client::ApiClient`.
//!   - [`symbol_search`] — async fuzzy symbol search.
//!   - [`logo`]          — Parqet CDN logo URL helper + render shortcut.
//!
//! All three async loaders follow the same shape: `init(handle, auth)`
//! once at boot, then `*_async(...)` returns a `tokio::sync::oneshot`
//! receiver the UI polls each frame. New backend-facing modules should
//! land here so call sites have one consistent prefix
//! (`crate::api::*`) for "things that touch the network."

pub mod client;
pub mod logo;
pub mod symbol_search;
