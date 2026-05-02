//! "Data and Price Range" measurement tool. Two points define a rect; the
//! tool renders a faded fill and a label with Δbars, Δ$, and Δ%.

use egui::{Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Stroke};
use kurbo::{BezPath, Point};

use super::super::super::camera::Camera;
use super::super::hit_test::hit_shape;
use super::super::kind_style::KindStyle;
use super::super::style::{DrawingStyle, style_color};
use super::super::trait_def::{
    DrawingDraft, DrawingTool, InputResult, WorldPoint, world_to_screen,
};
use super::shared::two_click_input;

pub const ID: &str = "range_measure";
pub const NAME: &str = "Data and Price Range";
pub const ICON: &str = egui_phosphor::regular::RULER;

const PREVIEW_COLOR: Color32 = Color32::from_rgba_premultiplied(255, 255, 255, 160);
const PREVIEW_WIDTH: f32 = 1.0;
const FILL_ALPHA: u8 = 32;
const LABEL_BG: Color32 = Color32::from_rgba_premultiplied(30, 30, 36, 220);
const LABEL_FG: Color32 = Color32::from_rgb(225, 225, 230);

pub struct RangeMeasure;

pub fn factory() -> Box<dyn DrawingTool> {
    Box::new(RangeMeasure)
}

fn to_rect(chart_rect: Rect, camera: &Camera, points: &[WorldPoint]) -> Option<Rect> {
    if points.len() < 2 {
        return None;
    }
    let a = world_to_screen(chart_rect, camera, points[0]);
    let b = world_to_screen(chart_rect, camera, points[1]);
    Some(Rect::from_two_pos(a, b))
}

fn label_text(points: &[WorldPoint]) -> String {
    if points.len() < 2 {
        return String::new();
    }
    let bars = (points[1].index - points[0].index).abs() as i64;
    let d_price = points[1].price - points[0].price;
    let base = points[0].price.abs().max(1e-6);
    let pct = (d_price / base) * 100.0;
    format!("{} bars  {:+.2}  ({:+.2}%)", bars, d_price, pct)
}

fn paint_label(painter: &Painter, anchor: Pos2, text: &str) {
    let font = FontId::proportional(11.0);
    // Measure via layout: the crude approach of a fixed padding is fine here,
    // since text is short and fixed-shape.
    let galley = painter.layout_no_wrap(text.to_string(), font.clone(), LABEL_FG);
    let size = galley.size() + egui::vec2(10.0, 6.0);
    let rect = Rect::from_min_size(anchor + egui::vec2(6.0, 6.0), size);
    painter.rect_filled(rect, CornerRadius::same(3), LABEL_BG);
    painter.galley(rect.min + egui::vec2(5.0, 3.0), galley, LABEL_FG);
}

impl DrawingTool for RangeMeasure {
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
        kind_style: &KindStyle,
    ) {
        let Some(rect) = to_rect(chart_rect, camera, points) else {
            return;
        };
        let painter = painter.with_clip_rect(chart_rect);
        let stroke_color = style_color(style.color, style.opacity);
        let [r, g, b, _] = style.color.0;
        let fill = Color32::from_rgba_unmultiplied(
            r,
            g,
            b,
            ((FILL_ALPHA as f32) * style.opacity.clamp(0.0, 1.0)) as u8,
        );
        painter.rect_filled(rect, CornerRadius::ZERO, fill);
        painter.rect_stroke(
            rect,
            CornerRadius::ZERO,
            Stroke::new(style.width, stroke_color),
            egui::StrokeKind::Middle,
        );
        let show_label = matches!(kind_style, KindStyle::LabeledRect { show_label: true })
            || !matches!(kind_style, KindStyle::LabeledRect { .. });
        if show_label {
            paint_label(&painter, rect.left_bottom(), &label_text(points));
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
        let Some(rect) = to_rect(chart_rect, camera, points) else {
            return;
        };
        let painter = painter.with_clip_rect(chart_rect);
        let fill = Color32::from_rgba_unmultiplied(255, 255, 255, 22);
        painter.rect_filled(rect, CornerRadius::ZERO, fill);
        painter.rect_stroke(
            rect,
            CornerRadius::ZERO,
            Stroke::new(PREVIEW_WIDTH, PREVIEW_COLOR),
            egui::StrokeKind::Middle,
        );
        paint_label(&painter, rect.left_bottom(), &label_text(points));
        // Silence the unused import when opacity paths elide.
        let _ = Align2::LEFT_TOP;
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
        let Some(rect) = to_rect(chart_rect, camera, points) else {
            return false;
        };
        let mut path = BezPath::new();
        let k = |p: Pos2| Point::new(p.x as f64, p.y as f64);
        path.move_to(k(rect.left_top()));
        path.line_to(k(rect.right_top()));
        path.line_to(k(rect.right_bottom()));
        path.line_to(k(rect.left_bottom()));
        path.close_path();
        hit_shape(&path, px, tolerance_px)
    }

    fn handles(
        &self,
        chart_rect: Rect,
        _full_rect: Rect,
        camera: &Camera,
        points: &[WorldPoint],
    ) -> Vec<egui::Pos2> {
        let Some(rect) = to_rect(chart_rect, camera, points) else {
            return Vec::new();
        };
        vec![rect.left_top(), rect.right_bottom()]
    }

    fn bounds(
        &self,
        chart_rect: Rect,
        _full_rect: Rect,
        camera: &Camera,
        points: &[WorldPoint],
    ) -> Rect {
        to_rect(chart_rect, camera, points).unwrap_or(chart_rect)
    }

    fn default_kind_style(&self) -> KindStyle {
        KindStyle::DEFAULT_LABELED_RECT
    }
}
