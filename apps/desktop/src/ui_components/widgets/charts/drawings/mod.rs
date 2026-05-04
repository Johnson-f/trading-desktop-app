mod handles_render;
mod hit_test;
mod kind_style;
mod manager;
pub(crate) mod persistence;
mod registry;
mod selection;
mod style;
mod toolbar;
mod trait_def;

pub mod kinds;

pub use handles_render::paint_handles;
pub use kind_style::{FIB_RATIO_COUNT, KindStyle};
pub use manager::{DrawingsManager, SelectionDrag};
pub use registry::all;
pub use selection::{SelectionInput, step as selection_step};
pub use style::{
    COLOR_PALETTE, DashStyle, DrawingStyle, get_user_default, init_with_database,
    reset_to_system_default, set_user_default,
};
pub use toolbar::{ToolbarEvent, anchor_rect_for as toolbar_anchor_rect, show as show_toolbar};
pub use trait_def::{
    CommittedDrawing, ExtendCapabilities, HIT_TOLERANCE_PX, InputResult, WorldPoint,
};

// Convenience alias for setting user default style
pub fn set_user_default_style(style: DrawingStyle) -> Result<(), String> {
    set_user_default(style)
}
