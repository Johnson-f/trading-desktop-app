use egui::{Color32, Painter, Rect, Stroke};
use kurbo::{BezPath, Point};

use super::super::super::camera::Camera;
use super::super::hit_test::hit_shape;
use super::super::kind_style::KindStyle;
use super::super::style::{DrawingStyle, paint_line};
use super::super::trait_def::{
    DrawingDraft, DrawingTool, InputResult, WorldPoint, world_to_screen,
};
use super::shared::polyline_input;

pub const ID: &str = "polyline";
pub const NAME: &str = "Polyline";
pub const ICON: &str = egui_phosphor::regular::POLYGON;

const PREVIEW_COLOR: Color32 = Color32::from_rgba_premultiplied(255, 255, 255, 160);
const PREVIEW_WIDTH: f32 = 1.5;

pub struct Polyline;

pub fn factory() -> Box<dyn DrawingTool> {
    Box::new(Polyline)
}

impl DrawingTool for Polyline {
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
        polyline_input(ui, chart_rect, camera, draft, ID)
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
        if points.len() < 2 {
            return;
        }
        let painter = painter.with_clip_rect(chart_rect);
        for pair in points.windows(2) {
            let a = world_to_screen(chart_rect, camera, pair[0]);
            let b = world_to_screen(chart_rect, camera, pair[1]);
            paint_line(&painter, a, b, style);
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
        let stroke = Stroke::new(PREVIEW_WIDTH, PREVIEW_COLOR);
        for pair in points.windows(2) {
            let a = world_to_screen(chart_rect, camera, pair[0]);
            let b = world_to_screen(chart_rect, camera, pair[1]);
            painter.line_segment([a, b], stroke);
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
        let mut path = BezPath::new();
        let first = world_to_screen(chart_rect, camera, points[0]);
        path.move_to(Point::new(first.x as f64, first.y as f64));
        for p in &points[1..] {
            let s = world_to_screen(chart_rect, camera, *p);
            path.line_to(Point::new(s.x as f64, s.y as f64));
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
        if points.is_empty() {
            return chart_rect;
        }
        let mut min_x = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        for p in points {
            let s = world_to_screen(chart_rect, camera, *p);
            min_x = min_x.min(s.x);
            max_x = max_x.max(s.x);
            min_y = min_y.min(s.y);
            max_y = max_y.max(s.y);
        }
        Rect::from_min_max(egui::Pos2::new(min_x, min_y), egui::Pos2::new(max_x, max_y))
    }
}
