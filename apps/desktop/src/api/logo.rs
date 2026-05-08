//! Stock logo helper — URL templating + render shortcut.
//!
//! Logos are served from Parqet's free CDN at:
//!   `https://assets.parqet.com/logos/symbol/{SYMBOL}`
//!
//! The CDN is symbol-keyed (no API key, no per-symbol metadata lookup
//! needed), so we just template the ticker into the URL. Symbols
//! Parqet doesn't host return 404 — egui's image loader drops them
//! silently, so non-listed symbols render as nothing.
//!
//! Loading + caching is handled by `egui_extras::install_image_loaders`,
//! which must be called once at boot (see `main.rs`). After that, every
//! `egui::Image::from_uri` call resolves through `ehttp` + the `image`
//! crate and is cached for the life of the process.
//!
//! Reusable across modules (search dropdown, watchlist rows, ticker
//! info bar, detail panels). Two surfaces:
//!   - [`url_for`]   — the URL string. Useful as a cache key or for
//!                     debug logs.
//!   - [`image_for`] — an `egui::Image` already pointed at the URL.
//!                     The caller chooses how to size and place it.

use egui::Image;

/// The Parqet CDN URL for `symbol`. Returns even when the symbol has
/// no hosted logo — egui will silently drop the image on 404.
pub fn url_for(symbol: &str) -> String {
    format!("https://assets.parqet.com/logos/symbol/{}", symbol)
}

/// Build an `egui::Image` widget for `symbol`. The caller is expected
/// to size it (e.g. `.fit_to_exact_size(Vec2::splat(28.0))`) and add it
/// via `ui.add(...)` or `ui.put(rect, ...)`.
///
/// Returns an `Image<'static>` because the underlying URL is owned —
/// callers can hold this widget across frames or ship it to wherever
/// it's painted without lifetime concerns.
pub fn image_for(symbol: &str) -> Image<'static> {
    Image::from_uri(url_for(symbol))
}
