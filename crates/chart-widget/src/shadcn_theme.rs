//! egui-shadcn `Theme` instance built from `zaned-theme` constants.
//!
//! Constructed once via [`OnceLock`] and read via [`theme()`] from any
//! call site that wants a `&'static Theme`. Avoids threading `&Theme`
//! through every component function — `Theme` is `Sync + Send` and is
//! only constructed once per process, so a static reference is fine.
//!
//! The mapping from `theme::*` constants to shadcn's
//! `ColorPalette` fields is opinionated; fields without a direct
//! `theme` equivalent fall back to the shadcn-default dark
//! palette via `ColorPalette::dark()`.

use std::sync::OnceLock;

use egui_shadcn::Theme;
use egui_shadcn::tokens::ColorPalette;

static THEME: OnceLock<Theme> = OnceLock::new();

/// Read the shared `Theme`, constructing it on first call.
pub(crate) fn theme() -> &'static Theme {
    THEME.get_or_init(|| Theme::new(palette()))
}

/// Build a `ColorPalette` that matches the rest of the desktop app's
/// `zaned-theme` constants. Fields without a direct equivalent inherit
/// from `ColorPalette::dark()` defaults.
fn palette() -> ColorPalette {
    let mut p = ColorPalette::dark();
    p.background = theme::BG;
    p.foreground = theme::TEXT_PRIMARY;
    p.card = theme::SURFACE;
    p.card_foreground = theme::TEXT_PRIMARY;
    p.popover = theme::SURFACE;
    p.popover_foreground = theme::TEXT_PRIMARY;
    p.border = theme::BORDER;
    p.input = theme::SURFACE_HIGH;
    p.muted = theme::SURFACE_HIGH;
    p.muted_foreground = theme::TEXT_MUTED;
    p.accent = theme::ACCENT_TEAL;
    p.accent_foreground = theme::TEXT_PRIMARY;
    p.destructive = theme::DOWN_RED;
    p.destructive_foreground = theme::TEXT_PRIMARY;
    p
}
