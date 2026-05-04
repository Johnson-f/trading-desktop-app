//! Two parallel lines. Points 0 and 1 define the base line; point 2 defines
//! the offset of the second line — the second line passes through point 2 and
//! is parallel to the base.

use egui::{Color32, Painter, Pos2, Rect, Stroke};
use kurbo::{BezPath, Point};

use super::super::super::camera::Camera;
use super::super::hit_test::hit_shape;
use super::super::kind_style::KindStyle;
use super::super::style::{DrawingStyle, paint_line, style_color};
use super::super::trait_def::{
    DrawingDraft, DrawingTool, InputResult, WorldPoint, world_to_screen,
};
use super::shared::three_click_input;

pub const ID: &str = "parallel_channel";
pub const NAME: &str = "Parallel Channel";
pub const ICON: &str = egui_phosphor::regular::EQUALS;

const PREVIEW_COLOR: Color32 = Color32::from_rgba_premultiplied(255, 255, 255, 160);
const PREVIEW_WIDTH: f32 = 1.5;
const FILL_ALPHA: u8 = 28;

pub struct ParallelChannel;

pub fn factory() -> Box<dyn DrawingTool> {
    Box::new(ParallelChannel)
}

/// Returns the two parallel line segments' screen-space endpoints.
/// `(a1, b1)` is the base line, `(a2, b2)` is the offset line.
fn segments(
    chart_rect: Rect,
    camera: &Camera,
    points: &[WorldPoint],
) -> Option<(Pos2, Pos2, Pos2, Pos2)> {
    if points.len() < 2 {
        return None;
    }
    let a1 = world_to_screen(chart_rect, camera, points[0]);
    let b1 = world_to_screen(chart_rect, camera, points[1]);
    if points.len() < 3 {
        return Some((a1, b1, a1, b1));
    }
    let third = world_to_screen(chart_rect, camera, points[2]);
    // Offset in screen space is simply third - nearest-point-on-base.
    // For a parallel translation we just shift both endpoints by the same
    // vertical offset (perpendicular offset would also work; Webull's tool
    // uses the vertical delta from the base line to the pointer).
    let dx = b1.x - a1.x;
    let t = if dx.abs() > 1e-6 {
        ((third.x - a1.x) / dx).clamp(-10.0, 10.0)
    } else {
        0.0
    };
    let on_base = Pos2::new(a1.x + t * dx, a1.y + t * (b1.y - a1.y));
    let offset_y = third.y - on_base.y;
    let a2 = Pos2::new(a1.x, a1.y + offset_y);
    let b2 = Pos2::new(b1.x, b1.y + offset_y);
    Some((a1, b1, a2, b2))
}

impl DrawingTool for ParallelChannel {
    fn id(&self) -> &'static str {
        ID
    }
    fn display_name(&self) -> &'static str {
        NAME
    }
    fn handle_input(
        &self,
        ui: &egui::Ui,
        chart_rect: Rect,
        camera: &Camera,
        draft: &mut Option<DrawingDraft>,
    ) -> InputResult {
        three_click_input(ui, chart_rect, camera, draft, ID)
    }

    fn render(
        &self,
        painter: &Painter,
        chart_rect: Rect,
        _full_rect: Rect,
        camera: &Camera,
        points: &[WorldPoint],
        style: &DrawingStyle,
        _kind_style: &KindStyle,
    ) {
        let Some((a1, b1, a2, b2)) = segments(chart_rect, camera, points) else {
            return;
        };
        let painter = painter.with_clip_rect(chart_rect);
        paint_line(&painter, a1, b1, style);
        if points.len() >= 3 {
            paint_line(&painter, a2, b2, style);
            // Fill between the two lines.
            let [r, g, b, _] = style.color.0;
            let fill = Color32::from_rgba_unmultiplied(
                r,
                g,
                b,
                ((FILL_ALPHA as f32) * style.opacity.clamp(0.0, 1.0)) as u8,
            );
            painter.add(egui::Shape::convex_polygon(
                vec![a1, b1, b2, a2],
                fill,
                Stroke::NONE,
            ));
        }
        let _ = style_color; // silence unused-import if feature gates elide
    }

    fn render_preview(
        &self,
        painter: &Painter,
        chart_rect: Rect,
        _full_rect: Rect,
        camera: &Camera,
        points: &[WorldPoint],
    ) {
        let Some((a1, b1, a2, b2)) = segments(chart_rect, camera, points) else {
            return;
        };
        let painter = painter.with_clip_rect(chart_rect);
        let stroke = Stroke::new(PREVIEW_WIDTH, PREVIEW_COLOR);
        painter.line_segment([a1, b1], stroke);
        if points.len() >= 3 {
            painter.line_segment([a2, b2], stroke);
        }
    }

    fn hit_test(
        &self,
        chart_rect: Rect,
        _full_rect: Rect,
        camera: &Camera,
        points: &[WorldPoint],
        px: egui::Pos2,
        tolerance_px: f32,
    ) -> bool {
        let Some((a1, b1, a2, b2)) = segments(chart_rect, camera, points) else {
            return false;
        };
        let mut path = BezPath::new();
        path.move_to(Point::new(a1.x as f64, a1.y as f64));
        path.line_to(Point::new(b1.x as f64, b1.y as f64));
        if points.len() >= 3 {
            path.move_to(Point::new(a2.x as f64, a2.y as f64));
            path.line_to(Point::new(b2.x as f64, b2.y as f64));
        }
        hit_shape(&path, px, tolerance_px)
    }

    fn handles(
        &self,
        chart_rect: Rect,
        _full_rect: Rect,
        camera: &Camera,
        points: &[WorldPoint],
    ) -> Vec<egui::Pos2> {
        points
            .iter()
            .map(|p| world_to_screen(chart_rect, camera, *p))
            .collect()
    }

    fn bounds(
        &self,
        chart_rect: Rect,
        _full_rect: Rect,
        camera: &Camera,
        points: &[WorldPoint],
    ) -> Rect {
        let Some((a1, b1, a2, b2)) = segments(chart_rect, camera, points) else {
            return chart_rect;
        };
        let mut rect = Rect::from_two_pos(a1, b1);
        if points.len() >= 3 {
            rect = rect.union(Rect::from_two_pos(a2, b2));
        }
        rect
    }
}
