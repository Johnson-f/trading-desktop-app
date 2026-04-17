use egui::{Color32, Painter, Rect, Stroke};

use super::super::super::camera::Camera;
use super::super::trait_def::{
    line_rect_intersection, world_to_screen, DrawingDraft, DrawingTool, InputResult, WorldPoint,
};
use super::shared::two_click_input;

pub const ID: &str = "extended";
pub const NAME: &str = "Extended";
pub const ICON: &str = egui_phosphor::regular::LINE_SEGMENTS;

const LINE_COLOR: Color32 = Color32::from_rgb(255, 193, 7);
const PREVIEW_COLOR: Color32 = Color32::from_rgba_premultiplied(255, 193, 7, 120);
const STROKE_WIDTH: f32 = 1.5;

pub struct Extended;

pub fn factory() -> Box<dyn DrawingTool> {
    Box::new(Extended)
}

impl DrawingTool for Extended {
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

    fn render(
        &self,
        painter: &Painter,
        chart_rect: Rect,
        _full_rect: Rect,
        camera: &Camera,
        points: &[WorldPoint],
    ) {
        let painter = painter.with_clip_rect(chart_rect);
        paint(&painter, chart_rect, camera, points, LINE_COLOR);
    }

    fn render_preview(
        &self,
        painter: &Painter,
        chart_rect: Rect,
        _full_rect: Rect,
        camera: &Camera,
        points: &[WorldPoint],
    ) {
        let painter = painter.with_clip_rect(chart_rect);
        paint(&painter, chart_rect, camera, points, PREVIEW_COLOR);
    }
}

fn paint(painter: &Painter, chart_rect: Rect, camera: &Camera, points: &[WorldPoint], color: Color32) {
    if points.len() < 2 {
        return;
    }
    let a = world_to_screen(chart_rect, camera, points[0]);
    let b = world_to_screen(chart_rect, camera, points[1]);
    if let Some((p1, p2)) = line_rect_intersection(a, b, chart_rect) {
        painter.line_segment([p1, p2], Stroke::new(STROKE_WIDTH, color));
    }
}
