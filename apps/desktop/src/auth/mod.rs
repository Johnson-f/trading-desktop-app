//! Desktop authentication: OAuth 2.0 Authorization Code + PKCE against
//! Clerk, with a one-shot localhost callback listener and OS-keychain
//! persistence of the refresh token.
//!
//! The egui main loop reads `AuthState` (held in `App::auth_state`) every
//! frame. When `Unauthenticated`, it renders `screen::LoginScreen`; when
//! `Authenticated`, it renders the chart UI. The current access token is
//! attached to every outbound request to `zaned-server`.

pub mod config;
pub mod flow;
pub mod pkce;
pub mod refresh;
pub mod screen;
pub mod signin;
pub mod state;
pub mod storage;
pub mod tokens;

pub use state::{AuthState, AuthStateHandle};
