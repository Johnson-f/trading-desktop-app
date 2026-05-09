use egui::{Color32, Painter, Pos2, Rect, Stroke};
use kurbo::Line;

use super::super::super::camera::Camera;
use super::super::hit_test::{hit_shape, to_kurbo};
use super::super::kind_style::KindStyle;
use super::super::style::{DrawingStyle, paint_line};
use super::super::trait_def::{
    DrawingDraft, DrawingTool, InputResult, WorldPoint, world_to_screen,
};
use super::shared::one_click_input;

pub const ID: &str = "horizontal_ray";
pub const NAME: &str = "Horizontal Ray";
pub const ICON: &str = egui_phosphor::regular::ARROW_RIGHT;

const PREVIEW_COLOR: Color32 = Color32::from_rgba_premultiplied(255, 255, 255, 160);
const PREVIEW_WIDTH: f32 = 1.5;

pub struct HorizontalRay;

pub fn factory() -> Box<dyn DrawingTool> {
    Box::new(HorizontalRay)
}

impl DrawingTool for HorizontalRay {
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
        _kind_style: &KindStyle,
    ) {
        let Some(p) = points.first() else { return };
        let painter = painter.with_clip_rect(chart_rect);
        let start = world_to_screen(chart_rect, camera, *p);
        if start.y < chart_rect.top() || start.y > chart_rect.bottom() {
            return;
        }
        paint_line(
            &painter,
            start,
            Pos2::new(chart_rect.right(), start.y),
            style,
        );
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
        let start = world_to_screen(chart_rect, camera, *p);
        if start.y < chart_rect.top() || start.y > chart_rect.bottom() {
            return;
        }
        painter.line_segment(
            [start, Pos2::new(chart_rect.right(), start.y)],
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
        let start = world_to_screen(chart_rect, camera, *p);
        let line = Line::new(
            to_kurbo(start),
            to_kurbo(Pos2::new(chart_rect.right(), start.y)),
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
        let start = world_to_screen(chart_rect, camera, *p);
        Rect::from_min_max(start, egui::Pos2::new(chart_rect.right(), start.y))
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::super::camera::Camera;
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

    // Horizontal ray from world (3, 5) — screen (30, 50), extends right to x=100.
    fn pts() -> Vec<WorldPoint> {
        vec![WorldPoint {
            index: 3.0,
            price: 5.0,
        }]
    }

    #[test]
    fn hit_right_of_anchor() {
        let t = HorizontalRay;
        assert!(t.hit_test(
            rect(),
            rect(),
            &camera(),
            &pts(),
            Pos2::new(60.0, 50.0),
            6.0
        ));
    }

    #[test]
    fn miss_left_of_anchor() {
        let t = HorizontalRay;
        assert!(!t.hit_test(
            rect(),
            rect(),
            &camera(),
            &pts(),
            Pos2::new(15.0, 50.0),
            6.0
        ));
    }

    #[test]
    fn handles_returns_single_anchor() {
        assert_eq!(
            HorizontalRay
                .handles(rect(), rect(), &camera(), &pts())
                .len(),
            1
        );
    }

    #[test]
    fn bounds_spans_right_extension() {
        let b = HorizontalRay.bounds(rect(), rect(), &camera(), &pts());
        assert!((b.left() - 30.0).abs() < 1e-3);
        assert!((b.right() - 100.0).abs() < 1e-3);
        assert!((b.top() - 50.0).abs() < 1e-3);
        assert!((b.bottom() - 50.0).abs() < 1e-3);
    }
}
