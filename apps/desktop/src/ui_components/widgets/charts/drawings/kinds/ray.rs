use egui::{Color32, Painter, Pos2, Rect, Stroke};

use super::super::super::camera::Camera;
use super::super::trait_def::{
    ray_to_rect_edge, world_to_screen, DrawingDraft, DrawingTool, InputResult, WorldPoint,
};
use super::shared::two_click_input;

pub const ID: &str = "ray";
pub const NAME: &str = "Ray";
pub const ICON: &str = egui_phosphor::regular::ARROW_UP_RIGHT;

const LINE_COLOR: Color32 = Color32::from_rgb(255, 193, 7);
const PREVIEW_COLOR: Color32 = Color32::from_rgba_premultiplied(255, 193, 7, 120);
const STROKE_WIDTH: f32 = 1.5;
const ENDPOINT_FILL: Color32 = Color32::from_rgb(18, 18, 22);
const ENDPOINT_STROKE: Color32 = Color32::from_rgb(255, 193, 7);
const ENDPOINT_RADIUS: f32 = 4.0;

pub struct Ray;

pub fn factory() -> Box<dyn DrawingTool> {
    Box::new(Ray)
}

impl DrawingTool for Ray {
    fn id(&self) -> &'static str { ID }
    fn display_name(&self) -> &'static str { NAME }
    fn icon(&self) -> &'static str { ICON }

    fn handle_input(
        &self,
        ui: &egui::Ui,
        chart_rect: Rect,
        camera: &Camera,
        draft: &mut Option<DrawingDraft>,
    ) -> InputResult {
        two_click_input(ui, chart_rect, camera, draft, ID)
    }

    fn render(&self, painter: &Painter, chart_rect: Rect, camera: &Camera, points: &[WorldPoint]) {
        paint(painter, chart_rect, camera, points, LINE_COLOR, true);
    }

    fn render_preview(
        &self,
        painter: &Painter,
        chart_rect: Rect,
        camera: &Camera,
        points: &[WorldPoint],
    ) {
        paint(painter, chart_rect, camera, points, PREVIEW_COLOR, false);
    }
}

fn paint(
    painter: &Painter,
    chart_rect: Rect,
    camera: &Camera,
    points: &[WorldPoint],
    color: Color32,
    draw_endpoint: bool,
) {
    if points.len() < 2 {
        return;
    }
    let a = world_to_screen(chart_rect, camera, points[0]);
    let b = world_to_screen(chart_rect, camera, points[1]);
    if (b - a).length_sq() < 0.01 {
        return;
    }
    let (start, end) = ray_to_rect_edge(a, b, chart_rect);
    painter.line_segment([start, end], Stroke::new(STROKE_WIDTH, color));
    if draw_endpoint {
        paint_endpoint(painter, a);
    }
}

fn paint_endpoint(painter: &Painter, pos: Pos2) {
    painter.circle_filled(pos, ENDPOINT_RADIUS, ENDPOINT_FILL);
    painter.circle_stroke(pos, ENDPOINT_RADIUS, Stroke::new(1.5, ENDPOINT_STROKE));
}
