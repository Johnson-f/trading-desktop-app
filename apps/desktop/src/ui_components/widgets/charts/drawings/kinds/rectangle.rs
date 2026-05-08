use egui::{Color32, CornerRadius, Painter, Rect, Stroke};
use kurbo::{BezPath, Point};

use super::super::super::camera::Camera;
use super::super::hit_test::hit_shape;
use super::super::kind_style::KindStyle;
use super::super::style::{DrawingStyle, style_color};
use super::super::trait_def::{
    DrawingDraft, DrawingTool, InputResult, WorldPoint, world_to_screen,
};
use super::shared::two_click_input;

pub const ID: &str = "rectangle";
pub const NAME: &str = "Rectangle";
pub const ICON: &str = egui_phosphor::regular::RECTANGLE;

const PREVIEW_COLOR: Color32 = Color32::from_rgba_premultiplied(255, 255, 255, 160);
const PREVIEW_WIDTH: f32 = 1.5;
const DEFAULT_FILL_ALPHA: u8 = 40;

pub struct Rectangle;

pub fn factory() -> Box<dyn DrawingTool> {
    Box::new(Rectangle)
}

fn to_rect(chart_rect: Rect, camera: &Camera, points: &[WorldPoint]) -> Option<Rect> {
    if points.len() < 2 {
        return None;
    }
    let a = world_to_screen(chart_rect, camera, points[0]);
    let b = world_to_screen(chart_rect, camera, points[1]);
    Some(Rect::from_two_pos(a, b))
}

impl DrawingTool for Rectangle {
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
        let (fill_enabled, fill_alpha) = match kind_style {
            KindStyle::FilledRect {
                fill_enabled,
                fill_alpha,
            } => (*fill_enabled, *fill_alpha),
            _ => (true, DEFAULT_FILL_ALPHA),
        };
        if fill_enabled {
            let [r, g, b, _] = style.color.0;
            let fill = Color32::from_rgba_unmultiplied(
                r,
                g,
                b,
                ((fill_alpha as f32) * style.opacity.clamp(0.0, 1.0)) as u8,
            );
            painter.rect_filled(rect, CornerRadius::ZERO, fill);
        }
        painter.rect_stroke(
            rect,
            CornerRadius::ZERO,
            Stroke::new(style.width, stroke_color),
            egui::StrokeKind::Middle,
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
        let Some(rect) = to_rect(chart_rect, camera, points) else {
            return;
        };
        let painter = painter.with_clip_rect(chart_rect);
        painter.rect_stroke(
            rect,
            CornerRadius::ZERO,
            Stroke::new(PREVIEW_WIDTH, PREVIEW_COLOR),
            egui::StrokeKind::Middle,
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
        let Some(rect) = to_rect(chart_rect, camera, points) else {
            return false;
        };
        let mut path = BezPath::new();
        let tl = Point::new(rect.left() as f64, rect.top() as f64);
        let tr = Point::new(rect.right() as f64, rect.top() as f64);
        let br = Point::new(rect.right() as f64, rect.bottom() as f64);
        let bl = Point::new(rect.left() as f64, rect.bottom() as f64);
        path.move_to(tl);
        path.line_to(tr);
        path.line_to(br);
        path.line_to(bl);
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
        // All four corners are draggable. Handles are indexed in the world-
        // coordinate layout `apply_handle_move` expects:
        //   0 = (p0.index, p0.price)       — stored as points[0]
        //   1 = (p1.index, p0.price)       — synthesized (shares price with 0)
        //   2 = (p1.index, p1.price)       — stored as points[1]
        //   3 = (p0.index, p1.price)       — synthesized (shares price with 2)
        // This layout is independent of which corner is visually top-left,
        // so the drag math stays correct even after the user flips the rect
        // inside-out by dragging past the opposite corner.
        if points.len() < 2 {
            return Vec::new();
        }
        let a = points[0];
        let b = points[1];
        let corners = [
            WorldPoint {
                index: a.index,
                price: a.price,
            },
            WorldPoint {
                index: b.index,
                price: a.price,
            },
            WorldPoint {
                index: b.index,
                price: b.price,
            },
            WorldPoint {
                index: a.index,
                price: b.price,
            },
        ];
        corners
            .iter()
            .map(|p| world_to_screen(chart_rect, camera, *p))
            .collect()
    }

    fn apply_handle_move(
        &self,
        points: &mut Vec<WorldPoint>,
        handle_idx: usize,
        target: WorldPoint,
    ) {
        if points.len() < 2 {
            return;
        }
        match handle_idx {
            0 => {
                points[0] = target;
            }
            1 => {
                points[1].index = target.index;
                points[0].price = target.price;
            }
            2 => {
                points[1] = target;
            }
            3 => {
                points[0].index = target.index;
                points[1].price = target.price;
            }
            _ => {}
        }
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
        KindStyle::DEFAULT_RECT
    }
}
