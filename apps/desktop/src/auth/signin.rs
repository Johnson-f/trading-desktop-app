//! Public entry point: kicks off the OAuth PKCE flow and updates
//! `AuthStateHandle` according to the outcome. Called by the egui
//! `LoginScreen` widget when the user clicks "Sign in".

use super::config::ClerkConfig;
use super::flow;
use super::state::{AuthState, AuthStateHandle};

/// Run the sign-in flow end-to-end. Sets the handle to `Loading` while in
/// progress, `Authenticated` on success, or `Failed { message }` on error.
pub async fn run(cfg: ClerkConfig, state: AuthStateHandle) {
    state.set(AuthState::Loading).await;
    match flow::sign_in(&cfg).await {
        Ok(token_set) => {
            tracing::info!("sign-in succeeded");
            state.set(AuthState::from(token_set)).await;
        }
        Err(e) => {
            tracing::warn!(error = %e, "sign-in failed");
            state
                .set(AuthState::Failed {
                    message: e.to_string(),
                })
                .await;
        }
    }
}
