use egui::{Color32, Painter, Pos2, Rect, Stroke};

use super::super::super::camera::Camera;
use super::super::trait_def::{world_to_screen, DrawingDraft, DrawingTool, InputResult, WorldPoint};
use super::shared::two_click_input;

pub const ID: &str = "trend_line";
pub const NAME: &str = "Trendline";
pub const ICON: &str = egui_phosphor::regular::LINE_SEGMENT;

const STROKE_WIDTH: f32 = 1.5;
const LINE_COLOR: Color32 = Color32::from_rgb(255, 193, 7);
const PREVIEW_COLOR: Color32 = Color32::from_rgba_premultiplied(255, 193, 7, 160);
const ENDPOINT_FILL: Color32 = Color32::from_rgb(18, 18, 22);
const ENDPOINT_STROKE: Color32 = Color32::from_rgb(255, 193, 7);
const ENDPOINT_RADIUS: f32 = 4.0;

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
    ) {
        if points.len() < 2 {
            return;
        }
        let painter = painter.with_clip_rect(chart_rect);
        let a = world_to_screen(chart_rect, camera, points[0]);
        let b = world_to_screen(chart_rect, camera, points[1]);
        painter.line_segment([a, b], Stroke::new(STROKE_WIDTH, LINE_COLOR));
        paint_endpoint(&painter, a);
        paint_endpoint(&painter, b);
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
        painter.line_segment([a, b], Stroke::new(STROKE_WIDTH, PREVIEW_COLOR));
    }
}

fn paint_endpoint(painter: &Painter, pos: Pos2) {
    painter.circle_filled(pos, ENDPOINT_RADIUS, ENDPOINT_FILL);
    painter.circle_stroke(pos, ENDPOINT_RADIUS, Stroke::new(1.5, ENDPOINT_STROKE));
}
