use egui::{Color32, Painter, Rect, Stroke};
use kurbo::Line;

use super::super::super::camera::Camera;
use super::super::hit_test::{hit_shape, to_kurbo};
use super::super::style::{DrawingStyle, paint_line};
use super::super::trait_def::{
    DrawingDraft, DrawingTool, InputResult, WorldPoint, line_rect_intersection, world_to_screen,
};
use super::shared::two_click_input;

pub const ID: &str = "extended";
pub const NAME: &str = "Extended";
pub const ICON: &str = egui_phosphor::regular::LINE_SEGMENTS;

const PREVIEW_COLOR: Color32 = Color32::from_rgba_premultiplied(255, 193, 7, 120);
const PREVIEW_WIDTH: f32 = 1.5;

pub struct Extended;

pub fn factory() -> Box<dyn DrawingTool> {
    Box::new(Extended)
}

impl DrawingTool for Extended {
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
        if let Some((p1, p2)) = line_rect_intersection(a, b, chart_rect) {
            paint_line(&painter, p1, p2, style);
        }
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
        if let Some((p1, p2)) = line_rect_intersection(a, b, chart_rect) {
            painter.line_segment([p1, p2], Stroke::new(PREVIEW_WIDTH, PREVIEW_COLOR));
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
        if points.len() < 2 {
            return false;
        }
        let a = world_to_screen(chart_rect, camera, points[0]);
        let b = world_to_screen(chart_rect, camera, points[1]);
        let Some((p1, p2)) = line_rect_intersection(a, b, chart_rect) else {
            return false;
        };
        hit_shape(&Line::new(to_kurbo(p1), to_kurbo(p2)), px, tolerance_px)
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
        line_rect_intersection(a, b, chart_rect)
            .map(|(p1, p2)| Rect::from_two_pos(p1, p2))
            .unwrap_or(chart_rect)
    }

    fn extend_capabilities(&self) -> super::super::trait_def::ExtendCapabilities {
        super::super::trait_def::ExtendCapabilities {
            left: true,
            right: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::super::camera::Camera;
    use super::super::super::trait_def::line_rect_intersection;
    use super::*;
    use egui::{Pos2, Rect};

    fn camera() -> Camera {
        let mut c = Camera::default();
        c.x_offset = 0.0;
        c.x_scale = 10.0;
        c.y_offset = 0.0;
        c.y_scale = 10.0;
        c.viewport = egui::Vec2::new(100.0, 100.0);
        c
    }
    fn rect() -> Rect {
        Rect::from_min_max(Pos2::ZERO, Pos2::new(100.0, 100.0))
    }

    // Line through world (2, 5) and (5, 5) — horizontal, extends full width.
    fn pts() -> Vec<WorldPoint> {
        vec![
            WorldPoint {
                index: 2.0,
                price: 5.0,
            },
            WorldPoint {
                index: 5.0,
                price: 5.0,
            },
        ]
    }

    #[test]
    fn hit_on_extended_line() {
        let t = Extended;
        // Far left of chart — outside the original segment but on the extended line.
        assert!(t.hit_test(rect(), rect(), &camera(), &pts(), Pos2::new(5.0, 50.0), 6.0));
        // Far right of chart — same.
        assert!(t.hit_test(
            rect(),
            rect(),
            &camera(),
            &pts(),
            Pos2::new(95.0, 50.0),
            6.0
        ));
    }

    #[test]
    fn miss_off_line() {
        let t = Extended;
        assert!(!t.hit_test(
            rect(),
            rect(),
            &camera(),
            &pts(),
            Pos2::new(50.0, 80.0),
            6.0
        ));
    }

    #[test]
    fn handles_returns_two_points() {
        let t = Extended;
        let h = t.handles(rect(), rect(), &camera(), &pts());
        assert_eq!(h.len(), 2);
    }

    #[test]
    fn bounds_spans_clipped_extension() {
        let t = Extended;
        let b = t.bounds(rect(), rect(), &camera(), &pts());
        // Horizontal line: clipped extension spans full chart width at y=50.
        assert!((b.left() - 0.0).abs() < 1e-3);
        assert!((b.right() - 100.0).abs() < 1e-3);
        let _ = line_rect_intersection; // keep import used
    }

    #[test]
    fn extend_capabilities_both_sides() {
        let t = Extended;
        let caps = t.extend_capabilities();
        assert!(caps.left);
        assert!(caps.right);
    }
}
