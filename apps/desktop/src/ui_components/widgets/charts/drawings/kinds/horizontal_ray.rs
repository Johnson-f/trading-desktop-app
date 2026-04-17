use egui::{Color32, Painter, Pos2, Rect, Stroke};

use super::super::super::camera::Camera;
use super::super::trait_def::{world_to_screen, DrawingDraft, DrawingTool, InputResult, WorldPoint};
use super::shared::one_click_input;

pub const ID: &str = "horizontal_ray";
pub const NAME: &str = "Horizontal Ray";
pub const ICON: &str = egui_phosphor::regular::ARROW_RIGHT;

const LINE_COLOR: Color32 = Color32::from_rgb(255, 193, 7);
const PREVIEW_COLOR: Color32 = Color32::from_rgba_premultiplied(255, 193, 7, 120);
const STROKE_WIDTH: f32 = 1.5;
const ENDPOINT_FILL: Color32 = Color32::from_rgb(18, 18, 22);
const ENDPOINT_STROKE: Color32 = Color32::from_rgb(255, 193, 7);
const ENDPOINT_RADIUS: f32 = 4.0;

pub struct HorizontalRay;

pub fn factory() -> Box<dyn DrawingTool> {
    Box::new(HorizontalRay)
}

impl DrawingTool for HorizontalRay {
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
        paint(&painter, chart_rect, camera, points, LINE_COLOR, true);
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
        paint(&painter, chart_rect, camera, points, PREVIEW_COLOR, false);
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
    let Some(p) = points.first() else { return };
    let start = world_to_screen(chart_rect, camera, *p);
    if start.y < chart_rect.top() || start.y > chart_rect.bottom() {
        return;
    }
    painter.line_segment(
        [start, Pos2::new(chart_rect.right(), start.y)],
        Stroke::new(STROKE_WIDTH, color),
    );
    if draw_endpoint {
        painter.circle_filled(start, ENDPOINT_RADIUS, ENDPOINT_FILL);
        painter.circle_stroke(start, ENDPOINT_RADIUS, Stroke::new(1.5, ENDPOINT_STROKE));
    }
}
