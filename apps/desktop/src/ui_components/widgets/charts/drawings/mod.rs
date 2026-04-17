mod manager;
mod registry;
mod trait_def;

pub mod kinds;

pub use manager::DrawingsManager;
pub use registry::{all, get, DrawingToolDef};
pub use trait_def::{
    line_rect_intersection, ray_to_rect_edge, screen_to_world, world_to_screen, CommittedDrawing,
    DrawingDraft, DrawingTool, InputResult, WorldPoint,
};
