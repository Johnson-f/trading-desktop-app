use egui::{Color32, Painter, Rect, Stroke};
use kurbo::Line;

use super::super::super::camera::Camera;
use super::super::hit_test::{hit_shape, to_kurbo};
use super::super::style::{DrawingStyle, paint_line};
use super::super::trait_def::{
    DrawingDraft, DrawingTool, InputResult, WorldPoint, world_to_screen,
};
use super::shared::two_click_input;

pub const ID: &str = "trend_line";
pub const NAME: &str = "Trendline";
pub const ICON: &str = egui_phosphor::regular::LINE_SEGMENT;

const PREVIEW_COLOR: Color32 = Color32::from_rgba_premultiplied(255, 193, 7, 160);
const PREVIEW_WIDTH: f32 = 1.5;

pub struct TrendLine;

pub fn factory() -> Box<dyn DrawingTool> {
    Box::new(TrendLine)
}

impl DrawingTool for TrendLine {
    fn id(&self) -> &'static str {
        ID
    }
    fn display_name(&self) -> &'static str {
        NAME
    }
    fn icon(&self) -> &'static str {
        ICON
    }

    fn handle_input(
        &self,
        ui: &egui::Ui,
        chart_rect: Rect,
        camera: &Camera,
        draft: &mut Option<DrawingDraft>,
    ) -> InputResult {
        two_click_input(ui, chart_rect, camera, draft, ID)
    }

    fn render(
        &self,
        painter: &Painter,
        chart_rect: Rect,
        _full_rect: Rect,
        camera: &Camera,
        points: &[WorldPoint],
        style: &DrawingStyle,
    ) {
        if points.len() < 2 {
            return;
        }
        let painter = painter.with_clip_rect(chart_rect);
        let a = world_to_screen(chart_rect, camera, points[0]);
        let b = world_to_screen(chart_rect, camera, points[1]);
        paint_line(&painter, a, b, style);
    }

    fn render_preview(
        &self,
        painter: &Painter,
        chart_rect: Rect,
        _full_rect: Rect,
        camera: &Camera,
        points: &[WorldPoint],
    ) {
        if points.len() < 2 {
            return;
        }
        let painter = painter.with_clip_rect(chart_rect);
        let a = world_to_screen(chart_rect, camera, points[0]);
        let b = world_to_screen(chart_rect, camera, points[1]);
        painter.line_segment([a, b], Stroke::new(PREVIEW_WIDTH, PREVIEW_COLOR));
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
        if points.len() < 2 {
            return false;
        }
        let a = world_to_screen(chart_rect, camera, points[0]);
        let b = world_to_screen(chart_rect, camera, points[1]);
        hit_shape(&Line::new(to_kurbo(a), to_kurbo(b)), px, tolerance_px)
    }

    fn handles(
        &self,
        chart_rect: Rect,
        _full_rect: Rect,
        camera: &Camera,
        points: &[WorldPoint],
    ) -> Vec<egui::Pos2> {
        if points.len() < 2 {
            return Vec::new();
        }
        vec![
            world_to_screen(chart_rect, camera, points[0]),
            world_to_screen(chart_rect, camera, points[1]),
        ]
    }

    fn bounds(
        &self,
        chart_rect: Rect,
        _full_rect: Rect,
        camera: &Camera,
        points: &[WorldPoint],
    ) -> Rect {
        if points.len() < 2 {
            return chart_rect;
        }
        let a = world_to_screen(chart_rect, camera, points[0]);
        let b = world_to_screen(chart_rect, camera, points[1]);
        Rect::from_two_pos(a, b)
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::super::camera::Camera;
    use super::*;
    use egui::{Pos2, Rect};

    fn test_camera() -> Camera {
        let mut c = Camera::default();
        c.x_offset = 0.0;
        c.x_scale = 10.0; // 10 pixels per candle index
        c.y_offset = 0.0;
        c.y_scale = 10.0; // 10 pixels per price unit
        c.viewport = egui::Vec2::new(100.0, 100.0);
        c
    }

    fn test_rect() -> Rect {
        Rect::from_min_max(Pos2::ZERO, Pos2::new(100.0, 100.0))
    }

    // Trend line from world (0, 5) → (5, 5). Screen-space: (0, 50) → (50, 50).
    fn test_points() -> Vec<WorldPoint> {
        vec![
            WorldPoint {
                index: 0.0,
                price: 5.0,
            },
            WorldPoint {
                index: 5.0,
                price: 5.0,
            },
        ]
    }

    #[test]
    fn hit_test_on_line() {
        let tool = TrendLine;
        let rect = test_rect();
        let camera = test_camera();
        let points = test_points();
        // Directly on the line at (25, 50).
        assert!(tool.hit_test(rect, rect, &camera, &points, Pos2::new(25.0, 50.0), 6.0));
    }

    #[test]
    fn hit_test_near_edge_within_tolerance() {
        let tool = TrendLine;
        let rect = test_rect();
        let camera = test_camera();
        let points = test_points();
        // 4 pixels off the line, tolerance 6 → hit.
        assert!(tool.hit_test(rect, rect, &camera, &points, Pos2::new(25.0, 54.0), 6.0));
    }

    #[test]
    fn hit_test_off_line() {
        let tool = TrendLine;
        let rect = test_rect();
        let camera = test_camera();
        let points = test_points();
        // 20 pixels off the line → miss.
        assert!(!tool.hit_test(rect, rect, &camera, &points, Pos2::new(25.0, 70.0), 6.0));
    }

    #[test]
    fn hit_test_past_segment_endpoint_misses() {
        let tool = TrendLine;
        let rect = test_rect();
        let camera = test_camera();
        let points = test_points();
        // Beyond the segment's endpoint at x=50; even at y=50 it's off.
        assert!(!tool.hit_test(rect, rect, &camera, &points, Pos2::new(80.0, 50.0), 6.0));
    }

    #[test]
    fn handles_returns_two_endpoints() {
        let tool = TrendLine;
        let rect = test_rect();
        let camera = test_camera();
        let points = test_points();
        let h = tool.handles(rect, rect, &camera, &points);
        assert_eq!(h.len(), 2);
        assert!((h[0] - Pos2::new(0.0, 50.0)).length() < 1e-3);
        assert!((h[1] - Pos2::new(50.0, 50.0)).length() < 1e-3);
    }

    #[test]
    fn bounds_spans_endpoints() {
        let tool = TrendLine;
        let rect = test_rect();
        let camera = test_camera();
        let points = test_points();
        let b = tool.bounds(rect, rect, &camera, &points);
        assert!((b.left() - 0.0).abs() < 1e-3);
        assert!((b.right() - 50.0).abs() < 1e-3);
        assert!((b.top() - 50.0).abs() < 1e-3);
        assert!((b.bottom() - 50.0).abs() < 1e-3);
    }
}
