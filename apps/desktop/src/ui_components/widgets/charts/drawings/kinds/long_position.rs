//! Long Position tool. Three anchors:
//!   p0 = entry (left edge x, entry price)
//!   p1 = stop  (x shares with p0, only price matters — drawn as horizontal)
//!   p2 = target (right edge x, target price)
//!
//! The rendered shape spans p0.x..p2.x with three horizontal levels and
//! green-shaded profit zone (entry → target) and red-shaded loss zone
//! (entry → stop). A compact label shows R:R.

use egui::{Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Stroke};
use kurbo::{BezPath, Point};

use super::super::super::camera::Camera;
use super::super::hit_test::hit_shape;
use super::super::kind_style::KindStyle;
use super::super::style::DrawingStyle;
use super::super::trait_def::{
    DrawingDraft, DrawingTool, InputResult, WorldPoint, world_to_screen,
};
use super::shared::three_click_input;

pub const ID: &str = "long_position";
pub const NAME: &str = "Long Position";
pub const ICON: &str = egui_phosphor::regular::TREND_UP;

const PREVIEW_COLOR: Color32 = Color32::from_rgba_premultiplied(255, 255, 255, 160);
const LABEL_BG: Color32 = Color32::from_rgba_premultiplied(30, 30, 36, 220);
const LABEL_FG: Color32 = Color32::from_rgb(225, 225, 230);
const FILL_ALPHA: u8 = 50;

pub(super) struct PositionColors {
    pub profit: Color32,
    pub loss: Color32,
    pub entry: Color32,
}

pub(super) fn colors_from_kind_style(kind_style: &KindStyle) -> PositionColors {
    match kind_style {
        KindStyle::Position {
            profit_color,
            loss_color,
            entry_color,
            ..
        } => PositionColors {
            profit: Color32::from_rgba_unmultiplied(
                profit_color.0[0],
                profit_color.0[1],
                profit_color.0[2],
                255,
            ),
            loss: Color32::from_rgba_unmultiplied(
                loss_color.0[0],
                loss_color.0[1],
                loss_color.0[2],
                255,
            ),
            entry: Color32::from_rgba_unmultiplied(
                entry_color.0[0],
                entry_color.0[1],
                entry_color.0[2],
                255,
            ),
        },
        _ => PositionColors {
            profit: Color32::from_rgb(76, 200, 110),
            loss: Color32::from_rgb(230, 100, 90),
            entry: Color32::from_rgb(200, 200, 210),
        },
    }
}

pub(super) fn show_label_from_kind_style(kind_style: &KindStyle) -> bool {
    matches!(
        kind_style,
        KindStyle::Position {
            show_label: true,
            ..
        }
    ) || !matches!(kind_style, KindStyle::Position { .. })
}

pub struct LongPosition;

pub fn factory() -> Box<dyn DrawingTool> {
    Box::new(LongPosition)
}

/// Screen-space key points: (left, right, y_entry, y_stop, y_target).
pub(super) fn geometry(
    chart_rect: Rect,
    camera: &Camera,
    points: &[WorldPoint],
) -> Option<(f32, f32, f32, f32, f32)> {
    if points.len() < 2 {
        return None;
    }
    let p0 = world_to_screen(chart_rect, camera, points[0]);
    let p1 = world_to_screen(chart_rect, camera, points[1]);
    let (x_left, x_right, y_target) = if points.len() >= 3 {
        let p2 = world_to_screen(chart_rect, camera, points[2]);
        let (lo, hi) = if p0.x <= p2.x {
            (p0.x, p2.x)
        } else {
            (p2.x, p0.x)
        };
        (lo, hi, p2.y)
    } else {
        // While placing: fall back so the render at click-2 still shows something.
        (p0.x.min(p1.x), p0.x.max(p1.x) + 60.0, p1.y)
    };
    Some((x_left, x_right, p0.y, p1.y, y_target))
}

pub(super) fn rr_label(points: &[WorldPoint], is_long: bool) -> String {
    if points.len() < 3 {
        return String::new();
    }
    let entry = points[0].price;
    let stop = points[1].price;
    let target = points[2].price;
    let risk = (entry - stop).abs();
    let reward = (target - entry).abs();
    let rr = if risk > 1e-6 { reward / risk } else { 0.0 };
    let dir = if is_long { "Long" } else { "Short" };
    format!(
        "{}  R:R 1:{:.2}  risk {:.2}  reward {:.2}",
        dir, rr, risk, reward
    )
}

pub(super) fn paint_position(
    painter: &Painter,
    x_left: f32,
    x_right: f32,
    y_entry: f32,
    y_stop: f32,
    y_target: f32,
    label: &str,
    colors: &PositionColors,
) {
    let profit_fill = Color32::from_rgba_unmultiplied(
        colors.profit.r(),
        colors.profit.g(),
        colors.profit.b(),
        FILL_ALPHA,
    );
    let loss_fill = Color32::from_rgba_unmultiplied(
        colors.loss.r(),
        colors.loss.g(),
        colors.loss.b(),
        FILL_ALPHA,
    );
    let profit_rect = Rect::from_min_max(
        Pos2::new(x_left, y_target.min(y_entry)),
        Pos2::new(x_right, y_target.max(y_entry)),
    );
    let loss_rect = Rect::from_min_max(
        Pos2::new(x_left, y_stop.min(y_entry)),
        Pos2::new(x_right, y_stop.max(y_entry)),
    );
    painter.rect_filled(profit_rect, CornerRadius::ZERO, profit_fill);
    painter.rect_filled(loss_rect, CornerRadius::ZERO, loss_fill);

    let entry_stroke = Stroke::new(1.5, colors.entry);
    let target_stroke = Stroke::new(1.0, colors.profit);
    let stop_stroke = Stroke::new(1.0, colors.loss);
    painter.line_segment(
        [Pos2::new(x_left, y_entry), Pos2::new(x_right, y_entry)],
        entry_stroke,
    );
    painter.line_segment(
        [Pos2::new(x_left, y_target), Pos2::new(x_right, y_target)],
        target_stroke,
    );
    painter.line_segment(
        [Pos2::new(x_left, y_stop), Pos2::new(x_right, y_stop)],
        stop_stroke,
    );

    if !label.is_empty() {
        let font = FontId::proportional(11.0);
        let galley = painter.layout_no_wrap(label.to_string(), font, LABEL_FG);
        let size = galley.size() + egui::vec2(10.0, 6.0);
        let anchor = Pos2::new(x_left + 6.0, y_entry - size.y - 6.0);
        let rect = Rect::from_min_size(anchor, size);
        painter.rect_filled(rect, CornerRadius::same(3), LABEL_BG);
        painter.galley(rect.min + egui::vec2(5.0, 3.0), galley, LABEL_FG);
    }
    let _ = Align2::LEFT_TOP;
}

impl DrawingTool for LongPosition {
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
        three_click_input(ui, chart_rect, camera, draft, ID)
    }

    fn render(
        &self,
        painter: &Painter,
        chart_rect: Rect,
        _full_rect: Rect,
        camera: &Camera,
        points: &[WorldPoint],
        _style: &DrawingStyle,
        kind_style: &KindStyle,
    ) {
        let Some((xl, xr, ye, ys, yt)) = geometry(chart_rect, camera, points) else {
            return;
        };
        let painter = painter.with_clip_rect(chart_rect);
        let colors = colors_from_kind_style(kind_style);
        let label = if show_label_from_kind_style(kind_style) {
            rr_label(points, true)
        } else {
            String::new()
        };
        paint_position(&painter, xl, xr, ye, ys, yt, &label, &colors);
    }

    fn render_preview(
        &self,
        painter: &Painter,
        chart_rect: Rect,
        _full_rect: Rect,
        camera: &Camera,
        points: &[WorldPoint],
    ) {
        let Some((xl, xr, ye, ys, yt)) = geometry(chart_rect, camera, points) else {
            return;
        };
        let painter = painter.with_clip_rect(chart_rect);
        let stroke = Stroke::new(1.0, PREVIEW_COLOR);
        painter.line_segment([Pos2::new(xl, ye), Pos2::new(xr, ye)], stroke);
        if points.len() >= 2 {
            painter.line_segment([Pos2::new(xl, ys), Pos2::new(xr, ys)], stroke);
        }
        if points.len() >= 3 {
            painter.line_segment([Pos2::new(xl, yt), Pos2::new(xr, yt)], stroke);
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
        let Some((xl, xr, ye, ys, yt)) = geometry(chart_rect, camera, points) else {
            return false;
        };
        let mut path = BezPath::new();
        let k = |x: f32, y: f32| Point::new(x as f64, y as f64);
        path.move_to(k(xl, ye));
        path.line_to(k(xr, ye));
        path.move_to(k(xl, ys));
        path.line_to(k(xr, ys));
        path.move_to(k(xl, yt));
        path.line_to(k(xr, yt));
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
        let Some((xl, xr, ye, ys, yt)) = geometry(chart_rect, camera, points) else {
            return chart_rect;
        };
        let top = ye.min(ys).min(yt);
        let bottom = ye.max(ys).max(yt);
        Rect::from_min_max(Pos2::new(xl, top), Pos2::new(xr, bottom))
    }

    fn default_kind_style(&self) -> KindStyle {
        KindStyle::default_position()
    }
}
