use egui::{Color32, Painter, Rect, Stroke};

use super::super::super::camera::Camera;
use super::super::trait_def::{world_to_screen, DrawingDraft, DrawingTool, InputResult, WorldPoint};
use super::shared::one_click_input;

pub const ID: &str = "horizontal_line";
pub const NAME: &str = "Horizontal Line";
pub const ICON: &str = egui_phosphor::regular::MINUS;

const LINE_COLOR: Color32 = Color32::from_rgb(255, 193, 7);
const PREVIEW_COLOR: Color32 = Color32::from_rgba_premultiplied(255, 193, 7, 120);
const STROKE_WIDTH: f32 = 1.5;

pub struct HorizontalLine;

pub fn factory() -> Box<dyn DrawingTool> {
    Box::new(HorizontalLine)
}

impl DrawingTool for HorizontalLine {
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
        one_click_input(ui, chart_rect, camera, draft, ID)
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
    let Some(p) = points.first() else { return };
    let y = world_to_screen(chart_rect, camera, *p).y;
    if y < chart_rect.top() || y > chart_rect.bottom() {
        return;
    }
    painter.hline(chart_rect.x_range(), y, Stroke::new(STROKE_WIDTH, color));
}
