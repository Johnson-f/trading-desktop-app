use egui::{Painter, Rect};

use super::super::camera::Camera;
use super::kind_style::KindStyle;
use super::style::DrawingStyle;

/// A point anchored in chart world coordinates — candle index (x) and price (y).
/// Drawings store points in world space so they stay attached to the underlying
/// data as the user pans/zooms.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WorldPoint {
    pub index: f32,
    pub price: f32,
}

/// A finalized drawing. Emitted once the user finishes placing points.
///
/// Derives `Serialize` but not `Deserialize` because `def_id: &'static str`
/// cannot be materialized by deserialization. When persistence lands, a
/// separate DTO with `def_id: String` will round-trip via a registry lookup.
#[derive(Clone, Debug, serde::Serialize)]
pub struct CommittedDrawing {
    pub id: u64,
    pub def_id: &'static str,
    pub points: Vec<WorldPoint>,
    /// ISO date (YYYY-MM-DD) of the raw daily candle each point was placed on.
    /// Used to remap indices when the user switches timeframes.
    #[serde(default)]
    pub point_dates: Vec<String>,
    #[serde(default)]
    pub style: DrawingStyle,
    #[serde(default)]
    pub kind_style: KindStyle,
    #[serde(default)]
    pub locked: bool,
}

/// The in-progress drawing the active tool is building up before commit.
#[derive(Clone, Debug, Default)]
pub struct DrawingDraft {
    pub def_id: &'static str,
    pub points: Vec<WorldPoint>,
}

pub enum InputResult {
    Continue,
    Commit(Vec<WorldPoint>),
}

pub trait DrawingTool: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;

    /// Process input while this tool is active. The tool mutates `draft` to
    /// track partial input (e.g. first click placed, cursor previewing the
    /// second). Returns `Commit(points)` when the drawing is complete.
    fn handle_input(
        &self,
        ui: &egui::Ui,
        chart_rect: Rect,
        camera: &Camera,
        draft: &mut Option<DrawingDraft>,
    ) -> InputResult;

    /// Render a committed drawing using its stored style. `full_rect` spans
    /// the main chart + sub-panes; tools whose shape should span all panes
    /// (vertical line) paint against it, others narrow their painter clip back
    /// to chart_rect. `kind_style` carries tool-specific appearance knobs
    /// (fills, label toggles, etc.); tools that don't need one get
    /// `KindStyle::None` and ignore it.
    fn render(
        &self,
        painter: &Painter,
        chart_rect: Rect,
        full_rect: Rect,
        camera: &Camera,
        points: &[WorldPoint],
        style: &DrawingStyle,
        kind_style: &KindStyle,
    );

    /// Render an in-progress preview. Defaults to the same look as a committed
    /// drawing; override if a tool wants a dashed / faded preview.
    fn render_preview(
        &self,
        painter: &Painter,
        chart_rect: Rect,
        full_rect: Rect,
        camera: &Camera,
        points: &[WorldPoint],
    ) {
        self.render(
            painter,
            chart_rect,
            full_rect,
            camera,
            points,
            &DrawingStyle::default(),
            &KindStyle::None,
        );
    }

    /// Return true if `px` (screen space) is within `tolerance_px` of the
    /// drawing's rendered shape. Default: false (tool doesn't participate in
    /// selection). Tasks 5–10 override this per tool.
    fn hit_test(
        &self,
        _chart_rect: Rect,
        _full_rect: Rect,
        _camera: &Camera,
        _points: &[WorldPoint],
        _px: egui::Pos2,
        _tolerance_px: f32,
    ) -> bool {
        false
    }

    /// Screen-space positions of endpoint handles, index-aligned with `points`.
    /// Default: empty (no handles rendered).
    fn handles(
        &self,
        _chart_rect: Rect,
        _full_rect: Rect,
        _camera: &Camera,
        _points: &[WorldPoint],
    ) -> Vec<egui::Pos2> {
        Vec::new()
    }

    /// Screen-space bounding box used to anchor the floating toolbar. Default:
    /// chart_rect (safe fallback).
    fn bounds(
        &self,
        chart_rect: Rect,
        _full_rect: Rect,
        _camera: &Camera,
        _points: &[WorldPoint],
    ) -> Rect {
        chart_rect
    }

    /// Which extend toggles the settings modal should show for this tool.
    fn extend_capabilities(&self) -> ExtendCapabilities {
        ExtendCapabilities::default()
    }

    /// Starting `KindStyle` for a freshly committed drawing of this kind. Tools
    /// that have no per-kind knobs leave this at `KindStyle::None`.
    fn default_kind_style(&self) -> KindStyle {
        KindStyle::None
    }

    /// Apply a handle drag. Default: replace `points[handle_idx]` with `target`
    /// — fine for every tool whose handles correspond 1:1 with stored control
    /// points. Tools that expose synthesized handles (e.g. Rectangle's 4
    /// corners driven by 2 stored points) override this.
    fn apply_handle_move(
        &self,
        points: &mut Vec<WorldPoint>,
        handle_idx: usize,
        target: WorldPoint,
    ) {
        if let Some(pt) = points.get_mut(handle_idx) {
            *pt = target;
        }
    }
}

/// Convert a world point to screen space using the chart's camera. Shared
/// helper so every tool uses identical arithmetic.
pub fn world_to_screen(chart_rect: Rect, camera: &Camera, p: WorldPoint) -> egui::Pos2 {
    let x = chart_rect.left() + ((p.index as f64 - camera.x_offset) * camera.x_scale) as f32;
    let y = chart_rect.bottom() - ((p.price as f64 - camera.y_offset) * camera.y_scale) as f32;
    egui::Pos2::new(x, y)
}

/// Convert a screen position back to chart world coordinates.
pub fn screen_to_world(chart_rect: Rect, camera: &Camera, pos: egui::Pos2) -> WorldPoint {
    let index = camera.x_offset + (pos.x - chart_rect.left()) as f64 / camera.x_scale;
    let price = camera.y_offset + (chart_rect.bottom() - pos.y) as f64 / camera.y_scale;
    WorldPoint {
        index: index as f32,
        price: price as f32,
    }
}

/// Extend a line defined by `a` and `b` to the edges of `rect` in both
/// directions. Returns `None` if the line misses the rect entirely.
/// Liang–Barsky parametric clip.
pub fn line_rect_intersection(
    a: egui::Pos2,
    b: egui::Pos2,
    rect: Rect,
) -> Option<(egui::Pos2, egui::Pos2)> {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let mut t_min = f32::NEG_INFINITY;
    let mut t_max = f32::INFINITY;

    if dx.abs() > 1e-6 {
        let t1 = (rect.left() - a.x) / dx;
        let t2 = (rect.right() - a.x) / dx;
        t_min = t_min.max(t1.min(t2));
        t_max = t_max.min(t1.max(t2));
    } else if a.x < rect.left() || a.x > rect.right() {
        return None;
    }

    if dy.abs() > 1e-6 {
        let t1 = (rect.top() - a.y) / dy;
        let t2 = (rect.bottom() - a.y) / dy;
        t_min = t_min.max(t1.min(t2));
        t_max = t_max.min(t1.max(t2));
    } else if a.y < rect.top() || a.y > rect.bottom() {
        return None;
    }

    if t_min > t_max {
        return None;
    }
    Some((
        egui::Pos2::new(a.x + t_min * dx, a.y + t_min * dy),
        egui::Pos2::new(a.x + t_max * dx, a.y + t_max * dy),
    ))
}

/// Clip a ray starting at `a` in the direction of `b` to the chart rect.
/// Assumes `a` is inside `rect`; the far endpoint is the rect edge in the
/// direction of `b`.
pub fn ray_to_rect_edge(a: egui::Pos2, b: egui::Pos2, rect: Rect) -> (egui::Pos2, egui::Pos2) {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let mut t_far = f32::INFINITY;
    if dx > 1e-6 {
        t_far = t_far.min((rect.right() - a.x) / dx);
    } else if dx < -1e-6 {
        t_far = t_far.min((rect.left() - a.x) / dx);
    }
    if dy > 1e-6 {
        t_far = t_far.min((rect.bottom() - a.y) / dy);
    } else if dy < -1e-6 {
        t_far = t_far.min((rect.top() - a.y) / dy);
    }
    if !t_far.is_finite() || t_far <= 0.0 {
        return (a, b);
    }
    (a, egui::Pos2::new(a.x + t_far * dx, a.y + t_far * dy))
}

/// Click tolerance for hit-testing drawings, in screen pixels.
pub const HIT_TOLERANCE_PX: f32 = 6.0;

/// Radius of the blue selection handles rendered at each control point.
pub const HANDLE_RADIUS_PX: f32 = 5.0;

/// Per-tool declaration of which "extend" toggles apply in the settings
/// modal. Default: neither. Tools override in their trait impl.
#[derive(Default, Clone, Copy, Debug)]
pub struct ExtendCapabilities {
    pub left: bool,
    pub right: bool,
}
