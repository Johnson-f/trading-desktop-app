//! Canonical design tokens for the desktop app. All chrome components
//! (header, tab bar, toolbar, footer, sidebar, modals) should consume
//! these tokens rather than defining local Color32 / dimension
//! constants. Modeled after Webull's dark-trading aesthetic.

use egui::Color32;

// ── Surfaces ───────────────────────────────────────────────────
/// App background. Pure black to match Webull's chart canvas.
pub const BG: Color32 = Color32::from_rgb(0, 0, 0);
/// One step lighter than BG; for header/tab/toolbar bars.
pub const SURFACE: Color32 = Color32::from_rgb(24, 24, 28);
/// Two steps lighter; for inputs, hover backgrounds, modal bodies.
pub const SURFACE_HIGH: Color32 = Color32::from_rgb(30, 30, 33);
/// Hairline divider color between bands of chrome.
pub const BORDER: Color32 = Color32::from_rgb(30, 30, 33);
/// Slightly brightened border for hover states on interactive surfaces.
pub const BORDER_HOVER: Color32 = Color32::from_rgb(50, 50, 55);

// ── Text ───────────────────────────────────────────────────────
/// Primary on-surface text — full opacity off-white.
pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(240, 240, 242);
/// Secondary text — labels, captions, inactive tabs.
pub const TEXT_MUTED: Color32 = Color32::from_rgb(140, 140, 150);
/// Disabled/dim text — placeholders, deemphasized footnotes.
pub const TEXT_DIM: Color32 = Color32::from_rgb(63, 63, 63);

// ── Accents ────────────────────────────────────────────────────
/// Brand accent (currently indigo — may shift toward teal). Used for
/// active pills, focus rings, primary CTAs.
pub const ACCENT: Color32 = Color32::from_rgb(99, 102, 241);
/// Muted accent fill (e.g. active-tab background).
pub const ACCENT_BG: Color32 = Color32::from_rgb(33, 33, 54);
/// Mint/teal — used for "Auto" toggles and chart accents that should
/// not compete with the brand indigo. Matches Webull's chart legend ink.
pub const ACCENT_TEAL: Color32 = Color32::from_rgb(78, 205, 196);

// ── Semantic (chart / P&L) ─────────────────────────────────────
/// Bullish / gain color.
pub const UP_GREEN: Color32 = Color32::from_rgb(38, 201, 160);
/// Bearish / loss color.
pub const DOWN_RED: Color32 = Color32::from_rgb(255, 107, 107);

// ── Icons ──────────────────────────────────────────────────────
/// Inactive icon stroke — 35% white.
pub const ICON_INACTIVE: Color32 = Color32::from_rgba_premultiplied(240, 240, 242, 89);
/// Hovered icon stroke — 60% white.
pub const ICON_HOVER: Color32 = Color32::from_rgba_premultiplied(240, 240, 242, 153);
/// Active icon stroke — full white.
pub const ICON_ACTIVE: Color32 = Color32::from_rgb(240, 240, 242);

// ── States ─────────────────────────────────────────────────────
pub const HOVER_BG: Color32 = Color32::from_rgb(30, 30, 33);
pub const NOTIFICATION_DOT: Color32 = Color32::from_rgb(239, 68, 68);
pub const AVATAR: Color32 = Color32::from_rgb(119, 98, 243);

// ── Chrome rhythm (heights) ────────────────────────────────────
//
// Webull stacks chrome bands at intentionally varied heights so the
// eye reads each as a distinct band: thicker app-nav, thinner tabs,
// thinner toolbar, very-thin info bar. Use these tokens — don't
// hardcode heights per component.

/// Top app nav (logo, search, avatar). Tallest chrome band.
pub const HEADER_HEIGHT: f32 = 44.0;
/// Context tab bar (Chart, Corp Actions, etc.). Thinner.
pub const TAB_BAR_HEIGHT: f32 = 32.0;
/// Drawing tools bar.
pub const TOOLBAR_HEIGHT: f32 = 32.0;
/// Ticker info strip (symbol, name, interval).
pub const INFO_BAR_HEIGHT: f32 = 24.0;

// ── Misc ───────────────────────────────────────────────────────
pub const ICON_ROUNDING: f32 = 6.0;

// ── Indicator modal extras ─────────────────────────────────────
/// Emphasized text — bolder than TEXT_PRIMARY for section titles that need to
/// stand out against SURFACE_HIGH.
pub const TEXT_STRONG: Color32 = Color32::from_rgb(255, 255, 255);

/// Soft blue-tinted hover tint for indicator list rows (translucent accent).
pub const HOVER_BG_BLUE: Color32 = Color32::from_rgba_premultiplied(42, 108, 255, 20);

/// Star/favorite active color (amber).
pub const STAR_ACTIVE: Color32 = Color32::from_rgb(255, 193, 7);

/// Category group header background — slightly lifted from SURFACE.
pub const CATEGORY_HEADER_BG: Color32 = Color32::from_rgb(20, 20, 23);
