use egui::{Color32, Painter, Rect, Stroke};

use super::super::super::camera::Camera;
use super::super::trait_def::{world_to_screen, DrawingDraft, DrawingTool, InputResult, WorldPoint};
use super::shared::one_click_input;

pub const ID: &str = "vertical_line";
pub const NAME: &str = "Vertical Line";
pub const ICON: &str = egui_phosphor::regular::DIVIDE;

const LINE_COLOR: Color32 = Color32::from_rgb(255, 193, 7);
const PREVIEW_COLOR: Color32 = Color32::from_rgba_premultiplied(255, 193, 7, 120);
const STROKE_WIDTH: f32 = 1.5;

pub struct VerticalLine;

pub fn factory() -> Box<dyn DrawingTool> {
    Box::new(VerticalLine)
}

impl DrawingTool for VerticalLine {
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
        full_rect: Rect,
        camera: &Camera,
        points: &[WorldPoint],
    ) {
        paint(painter, chart_rect, full_rect, camera, points, LINE_COLOR);
    }

    fn render_preview(
        &self,
        painter: &Painter,
        chart_rect: Rect,
        full_rect: Rect,
        camera: &Camera,
        points: &[WorldPoint],
    ) {
        paint(painter, chart_rect, full_rect, camera, points, PREVIEW_COLOR);
    }
}

fn paint(
    painter: &Painter,
    chart_rect: Rect,
    full_rect: Rect,
    camera: &Camera,
    points: &[WorldPoint],
    color: Color32,
) {
    let Some(p) = points.first() else { return };
    let x = world_to_screen(chart_rect, camera, *p).x;
    if x < full_rect.left() || x > full_rect.right() {
        return;
    }
    painter.vline(x, full_rect.y_range(), Stroke::new(STROKE_WIDTH, color));
}
