//! Async symbol-search loader — bridges `api_client::search_symbols`
//! into the search bar's per-frame poll loop.
//!
//! Same shape as `candle_loader`: a once-initialized tokio handle + auth
//! handle live in static slots, and `search_async` returns a
//! `oneshot::Receiver` the caller polls each frame. The result is a
//! `Vec<Symbol>` ready to render as dropdown rows, or a stringly-typed
//! error for the search bar to log and clear.

use std::sync::RwLock;

use api_client::{ApiClient, Symbol};
use tokio::sync::oneshot;

use crate::api::client::SERVER_URL;
use crate::auth::{AuthState, AuthStateHandle};

static RUNTIME_HANDLE: RwLock<Option<tokio::runtime::Handle>> = RwLock::new(None);
static AUTH_STATE: RwLock<Option<AuthStateHandle>> = RwLock::new(None);

/// Server clamps the `limit` argument to [1, 50] — 12 is a comfortable
/// dropdown height on a 720px viewport without scrolling.
const DEFAULT_LIMIT: i32 = 12;

/// Wire up the loader. Must be called once at app boot before any
/// `search_async` call.
pub fn init(handle: tokio::runtime::Handle, auth: AuthStateHandle) {
    if let Ok(mut g) = RUNTIME_HANDLE.write() {
        *g = Some(handle);
    }
    if let Ok(mut g) = AUTH_STATE.write() {
        *g = Some(auth);
    }
}

/// Fire a fuzzy symbol-search for `query`. The returned receiver
/// resolves to `Ok(Vec<Symbol>)` on success or a `String` error
/// (intended for `tracing::warn!`).
///
/// Callers should drop the previous receiver when firing a new search;
/// the old future may still complete but its result will fall on a
/// dropped channel and be discarded.
pub fn search_async(query: String) -> oneshot::Receiver<Result<Vec<Symbol>, String>> {
    let (tx, rx) = oneshot::channel();
    let (Ok(handle_guard), Ok(auth_guard)) = (RUNTIME_HANDLE.read(), AUTH_STATE.read()) else {
        let _ = tx.send(Err("symbol_search: lock poisoned".into()));
        return rx;
    };
    let (Some(handle), Some(auth)) = (handle_guard.clone(), auth_guard.clone()) else {
        let _ = tx.send(Err("symbol_search not initialized".into()));
        return rx;
    };
    drop(handle_guard);
    drop(auth_guard);

    handle.spawn(async move {
        let bearer = match auth.snapshot().await {
            AuthState::Authenticated { access_token, .. } => access_token,
            other => {
                let _ = tx.send(Err(format!("not authenticated: {other:?}")));
                return;
            }
        };

        let client = ApiClient::new(SERVER_URL).with_bearer(bearer);
        let payload = match client.search_symbols(query, Some(DEFAULT_LIMIT)).await {
            Ok(symbols) => Ok(symbols),
            Err(e) => Err(e.to_string()),
        };
        let _ = tx.send(payload);
    });

    rx
}
