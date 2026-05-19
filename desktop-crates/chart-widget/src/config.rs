//! Boot-time configuration for the chart widget.
//!
//! `chart-widget` is a UI crate; it has no business knowing how the
//! desktop app authenticates with the gateway. The desktop app passes
//! a [`BearerProvider`] impl + a server URL + a tokio runtime handle
//! at startup; the loader sub-modules (`loader::candle_loader`,
//! `loader::tick_stream`, added in later tasks) consult these globals
//! every fetch.
//!
//! All slots are `Option<...>` until [`init`] is called; the loaders
//! return a "not initialized" error if accessed first. Multiple
//! `init` calls overwrite — the latest values win.
//!
//! `BearerProvider::bearer` returns the current access token or a
//! human-readable error. The desktop app's `AuthStateHandle` impl
//! returns `Err("not authenticated: <state>")` when the user is
//! signed out, which the loaders forward to the chart's per-frame
//! poll path as a stringly-typed error.

use std::sync::{Arc, RwLock};

/// Exposes the current access token. Implemented by the desktop app
/// over its `AuthStateHandle`.
pub trait BearerProvider: Send + Sync + 'static {
    /// Resolve to a bearer token string, or a human-readable error.
    /// Called per fetch — implementations should be cheap (snapshot
    /// from a shared state, not a network round-trip).
    ///
    /// Returns an `impl Future + Send` (rather than `async fn`) so
    /// the future can be erased into the object-safe
    /// [`BearerProviderObj`] dispatcher below; the blanket impl
    /// requires `Send` to satisfy the trait-object bound.
    fn bearer(&self) -> impl std::future::Future<Output = Result<String, String>> + Send;
}

/// Object-safe dispatcher used in the static slot. We can't store
/// `dyn BearerProvider` directly because of the async method — we
/// erase it as `Arc<dyn BearerProviderObj>` instead, with a blanket
/// impl so any `BearerProvider` implementor coerces in.
pub trait BearerProviderObj: Send + Sync + 'static {
    fn bearer_boxed<'a>(
        &'a self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, String>> + Send + 'a>>;
}

impl<T: BearerProvider> BearerProviderObj for T {
    fn bearer_boxed<'a>(
        &'a self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, String>> + Send + 'a>>
    {
        Box::pin(self.bearer())
    }
}

/// Boot-time config. Pass to [`init`].
pub struct Config {
    pub runtime: tokio::runtime::Handle,
    pub server_url: String,
    pub auth: Arc<dyn BearerProviderObj>,
}

static RUNTIME_HANDLE: RwLock<Option<tokio::runtime::Handle>> = RwLock::new(None);
static SERVER_URL: RwLock<Option<String>> = RwLock::new(None);
static AUTH: RwLock<Option<Arc<dyn BearerProviderObj>>> = RwLock::new(None);

/// Wire up the chart-widget. Must be called once at app boot before
/// any chart is constructed (or the first historical-bars fetch will
/// return a "not initialized" error and the chart will render empty).
pub fn init(cfg: Config) {
    if let Ok(mut g) = RUNTIME_HANDLE.write() {
        *g = Some(cfg.runtime);
    }
    if let Ok(mut g) = SERVER_URL.write() {
        *g = Some(cfg.server_url);
    }
    if let Ok(mut g) = AUTH.write() {
        *g = Some(cfg.auth);
    }
}

/// Read the runtime handle. Returns `None` before `init`.
#[allow(dead_code)]
pub(crate) fn runtime_handle() -> Option<tokio::runtime::Handle> {
    RUNTIME_HANDLE.read().ok().and_then(|g| g.clone())
}

/// Read the server URL. Returns `None` before `init`.
#[allow(dead_code)]
pub(crate) fn server_url() -> Option<String> {
    SERVER_URL.read().ok().and_then(|g| g.clone())
}

/// Read the auth provider. Returns `None` before `init`.
#[allow(dead_code)]
pub(crate) fn auth() -> Option<Arc<dyn BearerProviderObj>> {
    AUTH.read().ok().and_then(|g| g.clone())
}
