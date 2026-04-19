use egui::{Color32, Painter, Pos2, Rect, Stroke};
use kurbo::Line;

use super::super::super::camera::Camera;
use super::super::hit_test::{hit_shape, to_kurbo};
use super::super::style::{DrawingStyle, paint_hline};
use super::super::trait_def::{
    DrawingDraft, DrawingTool, InputResult, WorldPoint, world_to_screen,
};
use super::shared::one_click_input;

pub const ID: &str = "horizontal_line";
pub const NAME: &str = "Horizontal Line";
pub const ICON: &str = egui_phosphor::regular::MINUS;

const PREVIEW_COLOR: Color32 = Color32::from_rgba_premultiplied(255, 193, 7, 120);
const PREVIEW_WIDTH: f32 = 1.5;

pub struct HorizontalLine;

pub fn factory() -> Box<dyn DrawingTool> {
    Box::new(HorizontalLine)
}

impl DrawingTool for HorizontalLine {
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
        chart_rect: Rect,
        _full_rect: Rect,
        camera: &Camera,
        points: &[WorldPoint],
        style: &DrawingStyle,
    ) {
        let Some(p) = points.first() else { return };
        let painter = painter.with_clip_rect(chart_rect);
        let y = world_to_screen(chart_rect, camera, *p).y;
        if y < chart_rect.top() || y > chart_rect.bottom() {
            return;
        }
        paint_hline(&painter, chart_rect, y, style);
    }

    fn render_preview(
        &self,
        painter: &Painter,
        chart_rect: Rect,
        _full_rect: Rect,
        camera: &Camera,
        points: &[WorldPoint],
    ) {
        let Some(p) = points.first() else { return };
        let painter = painter.with_clip_rect(chart_rect);
        let y = world_to_screen(chart_rect, camera, *p).y;
        if y < chart_rect.top() || y > chart_rect.bottom() {
            return;
        }
        painter.hline(
            chart_rect.x_range(),
            y,
            Stroke::new(PREVIEW_WIDTH, PREVIEW_COLOR),
        );
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
        let Some(p) = points.first() else {
            return false;
        };
        if px.x < chart_rect.left() || px.x > chart_rect.right() {
            return false;
        }
        let y = world_to_screen(chart_rect, camera, *p).y;
        let line = Line::new(
            to_kurbo(Pos2::new(chart_rect.left(), y)),
            to_kurbo(Pos2::new(chart_rect.right(), y)),
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
        chart_rect: Rect,
        _full_rect: Rect,
        camera: &Camera,
        points: &[WorldPoint],
    ) -> Rect {
        let Some(p) = points.first() else {
            return chart_rect;
        };
        let y = world_to_screen(chart_rect, camera, *p).y;
        Rect::from_min_max(
            egui::Pos2::new(chart_rect.left(), y),
            egui::Pos2::new(chart_rect.right(), y),
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

    fn test_rect() -> Rect {
        Rect::from_min_max(Pos2::ZERO, Pos2::new(100.0, 100.0))
    }

    // Horizontal line at price 5 → screen y = 50 (bottom=100 - 5*10).
    fn points() -> Vec<WorldPoint> {
        vec![WorldPoint {
            index: 2.0,
            price: 5.0,
        }]
    }

    #[test]
    fn hit_on_line() {
        let t = HorizontalLine;
        assert!(t.hit_test(
            test_rect(),
            test_rect(),
            &test_camera(),
            &points(),
            Pos2::new(40.0, 50.0),
            6.0
        ));
    }

    #[test]
    fn hit_near_line_within_tolerance() {
        let t = HorizontalLine;
        assert!(t.hit_test(
            test_rect(),
            test_rect(),
            &test_camera(),
            &points(),
            Pos2::new(40.0, 54.0),
            6.0
        ));
    }

    #[test]
    fn miss_off_line() {
        let t = HorizontalLine;
        assert!(!t.hit_test(
            test_rect(),
            test_rect(),
            &test_camera(),
            &points(),
            Pos2::new(40.0, 70.0),
            6.0
        ));
    }

    #[test]
    fn handles_returns_single_anchor() {
        let t = HorizontalLine;
        let h = t.handles(test_rect(), test_rect(), &test_camera(), &points());
        assert_eq!(h.len(), 1);
        assert!((h[0] - Pos2::new(20.0, 50.0)).length() < 1e-3);
    }

    #[test]
    fn bounds_spans_chart_width_at_line_y() {
        let t = HorizontalLine;
        let b = t.bounds(test_rect(), test_rect(), &test_camera(), &points());
        assert!((b.left() - 0.0).abs() < 1e-3);
        assert!((b.right() - 100.0).abs() < 1e-3);
        assert!((b.top() - 50.0).abs() < 1e-3);
        assert!((b.bottom() - 50.0).abs() < 1e-3);
    }
}
