use egui::{Color32, Painter, Pos2, Rect, Stroke};
use kurbo::Line;

use super::super::super::camera::Camera;
use super::super::hit_test::{hit_shape, to_kurbo};
use super::super::style::{DrawingStyle, paint_vline};
use super::super::trait_def::{
    DrawingDraft, DrawingTool, InputResult, WorldPoint, world_to_screen,
};
use super::shared::one_click_input;

pub const ID: &str = "vertical_line";
pub const NAME: &str = "Vertical Line";
pub const ICON: &str = egui_phosphor::regular::DIVIDE;

const PREVIEW_COLOR: Color32 = Color32::from_rgba_premultiplied(255, 193, 7, 120);
const PREVIEW_WIDTH: f32 = 1.5;

pub struct VerticalLine;

pub fn factory() -> Box<dyn DrawingTool> {
    Box::new(VerticalLine)
}

impl DrawingTool for VerticalLine {
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
        one_click_input(ui, chart_rect, camera, draft, ID)
    }

    fn render(
        &self,
        painter: &Painter,
        _chart_rect: Rect,
        full_rect: Rect,
        camera: &Camera,
        points: &[WorldPoint],
        style: &DrawingStyle,
    ) {
        let Some(p) = points.first() else { return };
        let x = world_to_screen(full_rect, camera, *p).x;
        if x < full_rect.left() || x > full_rect.right() {
            return;
        }
        // Painter passed in already has full_rect clip (set by paint_drawings),
        // so dash segments render through sub-panes too.
        paint_vline(painter, full_rect, x, style);
    }

    fn render_preview(
        &self,
        painter: &Painter,
        _chart_rect: Rect,
        full_rect: Rect,
        camera: &Camera,
        points: &[WorldPoint],
    ) {
        let Some(p) = points.first() else { return };
        let x = world_to_screen(full_rect, camera, *p).x;
        if x < full_rect.left() || x > full_rect.right() {
            return;
        }
        painter.vline(
            x,
            full_rect.y_range(),
            Stroke::new(PREVIEW_WIDTH, PREVIEW_COLOR),
        );
    }

    fn hit_test(
        &self,
        _chart_rect: Rect,
        full_rect: Rect,
        camera: &Camera,
        points: &[WorldPoint],
        px: egui::Pos2,
        tolerance_px: f32,
    ) -> bool {
        let Some(p) = points.first() else {
            return false;
        };
        if px.y < full_rect.top() || px.y > full_rect.bottom() {
            return false;
        }
        let x = world_to_screen(full_rect, camera, *p).x;
        let line = Line::new(
            to_kurbo(Pos2::new(x, full_rect.top())),
            to_kurbo(Pos2::new(x, full_rect.bottom())),
        );
        hit_shape(&line, px, tolerance_px)
    }

    fn handles(
        &self,
        chart_rect: Rect,
        _full_rect: Rect,
        camera: &Camera,
        points: &[WorldPoint],
    ) -> Vec<egui::Pos2> {
        points
            .first()
            .map(|p| vec![world_to_screen(chart_rect, camera, *p)])
            .unwrap_or_default()
    }

    fn bounds(
        &self,
        _chart_rect: Rect,
        full_rect: Rect,
        camera: &Camera,
        points: &[WorldPoint],
    ) -> Rect {
        let Some(p) = points.first() else {
            return full_rect;
        };
        let x = world_to_screen(full_rect, camera, *p).x;
        Rect::from_min_max(
            egui::Pos2::new(x, full_rect.top()),
            egui::Pos2::new(x, full_rect.bottom()),
        )
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
        c.x_scale = 10.0;
        c.y_offset = 0.0;
        c.y_scale = 10.0;
        c.viewport = egui::Vec2::new(100.0, 100.0);
        c
    }

    fn rect() -> Rect {
        Rect::from_min_max(Pos2::ZERO, Pos2::new(100.0, 100.0))
    }

    // Vertical line at index 3 → screen x = 30.
    fn points() -> Vec<WorldPoint> {
        vec![WorldPoint {
            index: 3.0,
            price: 5.0,
        }]
    }

    #[test]
    fn hit_on_line() {
        let t = VerticalLine;
        assert!(t.hit_test(
            rect(),
            rect(),
            &test_camera(),
            &points(),
            Pos2::new(30.0, 80.0),
            6.0
        ));
    }

    #[test]
    fn hit_near_line_within_tolerance() {
        let t = VerticalLine;
        assert!(t.hit_test(
            rect(),
            rect(),
            &test_camera(),
            &points(),
            Pos2::new(34.0, 80.0),
            6.0
        ));
    }

    #[test]
    fn miss_off_line() {
        let t = VerticalLine;
        assert!(!t.hit_test(
            rect(),
            rect(),
            &test_camera(),
            &points(),
            Pos2::new(50.0, 80.0),
            6.0
        ));
    }

    #[test]
    fn handles_returns_single_anchor() {
        let t = VerticalLine;
        let h = t.handles(rect(), rect(), &test_camera(), &points());
        assert_eq!(h.len(), 1);
        assert!((h[0] - Pos2::new(30.0, 50.0)).length() < 1e-3);
    }

    #[test]
    fn bounds_spans_full_rect_height() {
        let t = VerticalLine;
        let full = Rect::from_min_max(Pos2::ZERO, Pos2::new(100.0, 150.0));
        let b = t.bounds(rect(), full, &test_camera(), &points());
        assert!((b.left() - 30.0).abs() < 1e-3);
        assert!((b.right() - 30.0).abs() < 1e-3);
        assert!((b.top() - 0.0).abs() < 1e-3);
        assert!((b.bottom() - 150.0).abs() < 1e-3);
    }
}
