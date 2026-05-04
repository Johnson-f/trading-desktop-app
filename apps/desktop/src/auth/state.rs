//! Shared authentication state held by the egui app and the background
//! refresh worker. Reads happen on the egui frame thread (every 16ms),
//! writes happen on the tokio runtime when sign-in/refresh/logout fires.
//!
//! Uses `std::sync` locks (not `tokio::sync`) because the egui main thread
//! is *inside* the tokio runtime (via `#[tokio::main]`) and tokio's
//! `blocking_read` panics in that context. Critical sections here are
//! short and never span an `.await`, so a sync lock is correct and faster.

use std::sync::{Arc, Mutex, RwLock};

use chrono::{DateTime, Utc};
use tokio::task::AbortHandle;

use super::tokens::TokenSet;

#[derive(Debug, Clone)]
pub enum AuthState {
    Unauthenticated,
    Loading,
    Authenticated { #[allow(dead_code)] access_token: String, expires_at: DateTime<Utc> },
    Failed { message: String },
}

#[derive(Clone)]
pub struct AuthStateHandle {
    inner: Arc<RwLock<AuthState>>,
    /// Tracks the in-flight sign-in task so Cancel/Logout can abort it.
    /// Without this, a spawned `signin::run` would overwrite the cancelled
    /// state when its OAuth flow eventually completes.
    in_flight: Arc<Mutex<Option<AbortHandle>>>,
}

impl AuthStateHandle {
    pub fn new(initial: AuthState) -> Self {
        Self {
            inner: Arc::new(RwLock::new(initial)),
            in_flight: Arc::new(Mutex::new(None)),
        }
    }

    /// Async-flavored read. The body is sync; the `async fn` shape is
    /// preserved so existing callers using `.await` still compile.
    pub async fn snapshot(&self) -> AuthState {
        self.inner.read().expect("auth state poisoned").clone()
    }

    /// Sync read. Safe to call from the egui frame thread.
    pub fn blocking_snapshot(&self) -> AuthState {
        self.inner.read().expect("auth state poisoned").clone()
    }

    pub async fn set(&self, state: AuthState) {
        *self.inner.write().expect("auth state poisoned") = state;
    }

    /// Register the abort handle of an in-flight sign-in task. If a previous
    /// task is still tracked, abort it first (only one flow runs at a time).
    pub async fn register_in_flight(&self, handle: AbortHandle) {
        let mut slot = self.in_flight.lock().expect("in_flight poisoned");
        if let Some(prev) = slot.take() {
            prev.abort();
        }
        *slot = Some(handle);
    }

    /// Combined cancel: abort the in-flight task AND set Unauthenticated.
    /// Used by Cancel and the "Try again" button on the failure screen.
    pub fn cancel_and_reset_from_sync(&self) {
        if let Some(h) = self.in_flight.lock().expect("in_flight poisoned").take() {
            h.abort();
        }
        *self.inner.write().expect("auth state poisoned") = AuthState::Unauthenticated;
    }
}

impl From<TokenSet> for AuthState {
    fn from(t: TokenSet) -> Self {
        AuthState::Authenticated {
            access_token: t.access_token,
            expires_at: t.expires_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn handle_round_trip() {
        let h = AuthStateHandle::new(AuthState::Unauthenticated);
        h.set(AuthState::Loading).await;
        match h.snapshot().await {
            AuthState::Loading => {}
            other => panic!("expected Loading, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn register_in_flight_aborts_previous() {
        let h = AuthStateHandle::new(AuthState::Unauthenticated);
        let task_a = tokio::spawn(async {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        });
        let task_b = tokio::spawn(async {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        });
        let handle_a = task_a.abort_handle();
        h.register_in_flight(handle_a).await;
        let handle_b = task_b.abort_handle();
        h.register_in_flight(handle_b).await;
        let res_a = task_a.await;
        assert!(res_a.is_err());
        // task_b is still tracked; cancel_and_reset_from_sync will abort it.
        h.cancel_and_reset_from_sync();
        let _ = task_b.await;
    }
}
