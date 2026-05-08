//! Live tick-stream subscription bridge.
//!
//! Mirrors the loader pattern in [`crate::api::candle_loader`]: a
//! once-initialized tokio handle + auth handle live in static slots,
//! and `subscribe(symbol)` returns an `mpsc::UnboundedReceiver` the
//! chart drains each frame.
//!
//! The spawned task forwards every `TickEvent` from the GraphQL
//! WebSocket subscription to the receiver. When the chart drops the
//! receiver (e.g. on symbol change), the next send fails and the task
//! exits cleanly — taking the WebSocket connection down with it.

use std::sync::RwLock;

use futures::StreamExt;
use tokio::sync::mpsc;
use zaned_api_client::{ApiClient, TickEvent};

use crate::api::client::SERVER_URL;
use crate::auth::{AuthState, AuthStateHandle};

static RUNTIME_HANDLE: RwLock<Option<tokio::runtime::Handle>> = RwLock::new(None);
static AUTH_STATE: RwLock<Option<AuthStateHandle>> = RwLock::new(None);

/// Wire up the subscriber. Must be called once at app boot before any
/// `subscribe` call. Subsequent calls overwrite the slots.
pub fn init(handle: tokio::runtime::Handle, auth: AuthStateHandle) {
    if let Ok(mut g) = RUNTIME_HANDLE.write() {
        *g = Some(handle);
    }
    if let Ok(mut g) = AUTH_STATE.write() {
        *g = Some(auth);
    }
}

/// Open a tick subscription for `symbol`. Returns a receiver the
/// caller polls each frame; events arrive in arrival order.
///
/// Drop the receiver to cancel: the task notices the closed channel on
/// its next send and exits, closing the WebSocket. There is no
/// "unsubscribe" method — receiver lifetime IS the subscription
/// lifetime.
///
/// Returns an empty/closed receiver on auth or init failure so callers
/// don't need to thread `Result`. Errors are logged via `tracing`.
pub fn subscribe(symbol: String) -> mpsc::UnboundedReceiver<TickEvent> {
    let (tx, rx) = mpsc::unbounded_channel();

    let (Ok(handle_guard), Ok(auth_guard)) = (RUNTIME_HANDLE.read(), AUTH_STATE.read()) else {
        tracing::warn!("tick_stream: lock poisoned");
        return rx;
    };
    let (Some(handle), Some(auth)) = (handle_guard.clone(), auth_guard.clone()) else {
        tracing::warn!("tick_stream not initialized");
        return rx;
    };
    drop(handle_guard);
    drop(auth_guard);

    handle.spawn(async move {
        let bearer = match auth.snapshot().await {
            AuthState::Authenticated { access_token, .. } => access_token,
            other => {
                tracing::warn!(symbol = %symbol, ?other, "tick subscribe: not authenticated");
                return;
            }
        };

        let client = ApiClient::new(SERVER_URL).with_bearer(bearer);
        let stream = match client.subscribe_ticks(vec![symbol.clone()]).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(symbol = %symbol, error = %e, "tick subscribe failed");
                return;
            }
        };
        tokio::pin!(stream);

        tracing::info!(symbol = %symbol, "tick subscription opened");

        while let Some(result) = stream.next().await {
            match result {
                Ok(resp) => {
                    if tx.send(resp.ticks).is_err() {
                        // Receiver dropped — symbol changed or chart torn down.
                        break;
                    }
                }
                Err(e) => {
                    tracing::warn!(symbol = %symbol, error = %e, "tick stream error");
                    break;
                }
            }
        }

        tracing::info!(symbol = %symbol, "tick subscription closed");
    });

    rx
}
