//! egui-shadcn `Theme` instance built from `zaned-theme` constants.
//!
//! Constructed once via [`OnceLock`] and read via [`theme()`] from any
//! call site that wants a `&'static Theme`. Avoids threading `&Theme`
//! through every component function — `Theme` is `Sync + Send` and is
//! only constructed once per process, so a static reference is fine.
//!
//! The mapping from `zaned_theme::*` constants to shadcn's
//! `ColorPalette` fields is opinionated; fields without a direct
//! `zaned_theme` equivalent fall back to the shadcn-default dark
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
    p.background = zaned_theme::BG;
    p.foreground = zaned_theme::TEXT_PRIMARY;
    p.card = zaned_theme::SURFACE;
    p.card_foreground = zaned_theme::TEXT_PRIMARY;
    p.popover = zaned_theme::SURFACE;
    p.popover_foreground = zaned_theme::TEXT_PRIMARY;
    p.border = zaned_theme::BORDER;
    p.input = zaned_theme::SURFACE_HIGH;
    p.muted = zaned_theme::SURFACE_HIGH;
    p.muted_foreground = zaned_theme::TEXT_MUTED;
    p.accent = zaned_theme::ACCENT_TEAL;
    p.accent_foreground = zaned_theme::TEXT_PRIMARY;
    p.destructive = zaned_theme::DOWN_RED;
    p.destructive_foreground = zaned_theme::TEXT_PRIMARY;
    p
}
