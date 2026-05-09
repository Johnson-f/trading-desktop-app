//! Live tick-stream subscription bridge.
//!
//! Mirrors the loader pattern in [`crate::loader::candle_loader`]: a
//! once-initialized tokio handle + auth handle live in static slots,
//! and `subscribe(symbol)` returns an `mpsc::UnboundedReceiver` the
//! chart drains each frame.
//!
//! The spawned task forwards every `TickEvent` from the GraphQL
//! WebSocket subscription to the receiver. When the chart drops the
//! receiver (e.g. on symbol change), the next send fails and the task
//! exits cleanly — taking the WebSocket connection down with it.

use futures::StreamExt;
use tokio::sync::mpsc;
use zaned_api_client::{ApiClient, TickEvent};

use crate::config;

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

    let runtime = match config::runtime_handle() {
        Some(h) => h,
        None => {
            tracing::warn!("chart-widget not initialized (call chart_widget::init first)");
            return rx;
        }
    };
    let auth = match config::auth() {
        Some(a) => a,
        None => {
            tracing::warn!("chart-widget not initialized (call chart_widget::init first)");
            return rx;
        }
    };
    let server_url = match config::server_url() {
        Some(s) => s,
        None => {
            tracing::warn!("chart-widget not initialized (call chart_widget::init first)");
            return rx;
        }
    };

    runtime.spawn(async move {
        let bearer = match auth.bearer_boxed().await {
            Ok(token) => token,
            Err(msg) => {
                tracing::warn!(symbol = %symbol, error = %msg, "tick subscribe: not authenticated");
                return;
            }
        };

        let client = ApiClient::new(&server_url).with_bearer(bearer);
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
