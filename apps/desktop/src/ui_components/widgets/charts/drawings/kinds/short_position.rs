//! Short Position — same geometry as LongPosition but drawn with `is_long =
//! false` label. The colored fills fall out naturally from the y_stop vs
//! y_target ordering, so the renderer stays identical.

use egui::{Color32, Painter, Pos2, Rect, Stroke};
use kurbo::{BezPath, Point};

use super::super::super::camera::Camera;
use super::super::hit_test::hit_shape;
use super::super::kind_style::KindStyle;
use super::super::style::DrawingStyle;
use super::super::trait_def::{
    DrawingDraft, DrawingTool, InputResult, WorldPoint, world_to_screen,
};
use super::long_position::{
    colors_from_kind_style, geometry, paint_position, rr_label, show_label_from_kind_style,
};
use super::shared::three_click_input;

pub const ID: &str = "short_position";
pub const NAME: &str = "Short Position";
pub const ICON: &str = egui_phosphor::regular::TREND_DOWN;

const PREVIEW_COLOR: Color32 = Color32::from_rgba_premultiplied(255, 255, 255, 160);

pub struct ShortPosition;

pub fn factory() -> Box<dyn DrawingTool> {
    Box::new(ShortPosition)
}

impl DrawingTool for ShortPosition {
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
            rr_label(points, false)
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
