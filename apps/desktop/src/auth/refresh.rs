//! Background tokio task that refreshes the access token at 80% of its
//! remaining lifetime. Run forever, sleeping until the next refresh point.
//! On any failure the worker logs and retries after a short backoff;
//! persistent failure (refresh token revoked) transitions auth state to
//! `Failed`, prompting the UI to ask the user to re-sign-in.

use std::time::Duration;

use anyhow::Result;
use chrono::Utc;

use super::config::ClerkConfig;
use super::flow;
use super::state::{AuthState, AuthStateHandle};
use super::storage;

/// Spawn the refresh worker. Returns immediately; the worker runs until
/// the host process exits.
pub fn spawn(cfg: ClerkConfig, state: AuthStateHandle) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            match refresh_once(&cfg, &state).await {
                Ok(sleep_secs) => {
                    tokio::time::sleep(Duration::from_secs(sleep_secs)).await;
                }
                Err(e) => {
                    tracing::warn!(error = %e, "refresh worker: error; backing off 60s");
                    tokio::time::sleep(Duration::from_secs(60)).await;
                }
            }
        }
    })
}

/// One iteration: read current state, decide whether to refresh now or
/// wait, do the refresh if needed, return how many seconds to sleep.
async fn refresh_once(cfg: &ClerkConfig, state: &AuthStateHandle) -> Result<u64> {
    let snapshot = state.snapshot().await;

    // Clerk access tokens have a 7200s lifetime. Refresh proactively when
    // 20% or less of that lifetime remains (≤ 1440s left). Fixed threshold
    // — using a fraction of `remaining` would be self-referential and
    // never hit the floor.
    const REFRESH_THRESHOLD_SECS: i64 = 1440;

    match snapshot {
        AuthState::Authenticated { expires_at, .. } => {
            let remaining = (expires_at - Utc::now()).num_seconds();
            if remaining > REFRESH_THRESHOLD_SECS {
                // Sleep until we cross the threshold (clamped at 30s min
                // so we don't busy-loop near the boundary).
                return Ok((remaining - REFRESH_THRESHOLD_SECS).max(30) as u64);
            }
            // Fall through and refresh now.
        }
        // Not authenticated — poll every 30s in case sign-in completes.
        _ => return Ok(30),
    }

    // Time to refresh.
    let refresh_token = match storage::load_refresh_token()? {
        Some(rt) => rt,
        None => {
            state
                .set(AuthState::Failed {
                    message: "refresh token missing — please sign in again".into(),
                })
                .await;
            return Ok(60);
        }
    };

    match flow::refresh(cfg, &refresh_token).await {
        Ok(token_set) => {
            tracing::info!("refresh worker: refreshed access token");
            state.set(AuthState::from(token_set)).await;
            // Sleep most of the new token's lifetime before next check.
            Ok((7200 - REFRESH_THRESHOLD_SECS) as u64)
        }
        Err(e) => {
            let msg = e.to_string();
            tracing::warn!(error = %msg, "refresh failed");
            if msg.contains("400") || msg.contains("401") || msg.contains("403") {
                let _ = storage::delete_refresh_token();
                state
                    .set(AuthState::Failed {
                        message: "session expired — please sign in again".into(),
                    })
                    .await;
            }
            Ok(60)
        }
    }
}
