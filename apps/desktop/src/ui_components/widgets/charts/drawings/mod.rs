mod handles_render;
mod hit_test;
mod manager;
mod registry;
mod selection;
mod style;
mod toolbar;
mod trait_def;

pub mod kinds;

pub use handles_render::paint_handles;
pub use manager::{DrawingsManager, SelectionDrag};
pub use registry::all;
pub use selection::{SelectionInput, step as selection_step};
pub use style::{COLOR_PALETTE, DashStyle, DrawingStyle};
pub use toolbar::{ToolbarEvent, anchor_rect_for as toolbar_anchor_rect, show as show_toolbar};
pub use trait_def::{CommittedDrawing, ExtendCapabilities, InputResult, WorldPoint};
