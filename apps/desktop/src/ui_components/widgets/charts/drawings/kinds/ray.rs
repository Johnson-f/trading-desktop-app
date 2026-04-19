use egui::{Color32, Painter, Rect, Stroke};
use kurbo::Line;

use super::super::super::camera::Camera;
use super::super::hit_test::{hit_shape, to_kurbo};
use super::super::style::{DrawingStyle, paint_line};
use super::super::trait_def::{
    DrawingDraft, DrawingTool, InputResult, WorldPoint, ray_to_rect_edge, world_to_screen,
};
use super::shared::two_click_input;

pub const ID: &str = "ray";
pub const NAME: &str = "Ray";
pub const ICON: &str = egui_phosphor::regular::ARROW_UP_RIGHT;

const PREVIEW_COLOR: Color32 = Color32::from_rgba_premultiplied(255, 193, 7, 120);
const PREVIEW_WIDTH: f32 = 1.5;

pub struct Ray;

pub fn factory() -> Box<dyn DrawingTool> {
    Box::new(Ray)
}

impl DrawingTool for Ray {
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
        if (b - a).length_sq() < 0.01 {
            return;
        }
        let (start, end) = ray_to_rect_edge(a, b, chart_rect);
        paint_line(&painter, start, end, style);
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
        if (b - a).length_sq() < 0.01 {
            return;
        }
        let (start, end) = ray_to_rect_edge(a, b, chart_rect);
        painter.line_segment([start, end], Stroke::new(PREVIEW_WIDTH, PREVIEW_COLOR));
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
        if (b - a).length_sq() < 0.01 {
            return false;
        }
        let (start, end) = ray_to_rect_edge(a, b, chart_rect);
        hit_shape(&Line::new(to_kurbo(start), to_kurbo(end)), px, tolerance_px)
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
        if (b - a).length_sq() < 0.01 {
            return chart_rect;
        }
        let (start, end) = ray_to_rect_edge(a, b, chart_rect);
        Rect::from_two_pos(start, end)
    }

    fn extend_capabilities(&self) -> super::super::trait_def::ExtendCapabilities {
        super::super::trait_def::ExtendCapabilities {
            left: false,
            right: true,
        }
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

    // Ray from world (2, 5) through (5, 5) — extends rightward.
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
    fn hit_on_ray_body() {
        let t = Ray;
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
    fn miss_before_ray_start() {
        let t = Ray;
        // x=5 is before the ray's starting x=20, but the ray_to_rect_edge segment
        // starts at a, so points left of a miss.
        assert!(!t.hit_test(rect(), rect(), &camera(), &pts(), Pos2::new(5.0, 50.0), 6.0));
    }

    #[test]
    fn handles_returns_two_points() {
        assert_eq!(Ray.handles(rect(), rect(), &camera(), &pts()).len(), 2);
    }

    #[test]
    fn extend_capabilities_right_only() {
        let caps = Ray.extend_capabilities();
        assert!(!caps.left);
        assert!(caps.right);
    }
}
