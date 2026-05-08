//! Andrews' Pitchfork. Three anchors:
//!   p0 = apex (the pivot before the move)
//!   p1, p2 = the reaction high/low that define the channel width
//!
//! Median line: from p0 through the midpoint of p1-p2.
//! Upper tine:  from p1, parallel to the median.
//! Lower tine:  from p2, parallel to the median.
//! Tines extend forward (right) to the chart edge.

use egui::{Color32, Painter, Pos2, Rect, Stroke};
use kurbo::{BezPath, Point};

use super::super::super::camera::Camera;
use super::super::hit_test::hit_shape;
use super::super::kind_style::KindStyle;
use super::super::style::{DrawingStyle, paint_line};
use super::super::trait_def::{
    DrawingDraft, DrawingTool, InputResult, WorldPoint, world_to_screen,
};
use super::shared::three_click_input;

pub const ID: &str = "pitchfork";
pub const NAME: &str = "Pitchfork";
pub const ICON: &str = egui_phosphor::regular::TREE_STRUCTURE;

const PREVIEW_COLOR: Color32 = Color32::from_rgba_premultiplied(255, 255, 255, 160);
const PREVIEW_WIDTH: f32 = 1.5;
const DEFAULT_FILL_ALPHA: u8 = 22;

pub struct Pitchfork;

pub fn factory() -> Box<dyn DrawingTool> {
    Box::new(Pitchfork)
}

/// Extend a ray from `origin` in the direction of `through` until it exits
/// `rect`. Returns the exit point (or `through` if the direction is zero).
fn extend_ray_to_rect(origin: Pos2, through: Pos2, rect: Rect) -> Pos2 {
    let dx = through.x - origin.x;
    let dy = through.y - origin.y;
    if dx.abs() < 1e-6 && dy.abs() < 1e-6 {
        return through;
    }
    let mut t_far = f32::INFINITY;
    if dx > 1e-6 {
        t_far = t_far.min((rect.right() - origin.x) / dx);
    } else if dx < -1e-6 {
        t_far = t_far.min((rect.left() - origin.x) / dx);
    }
    if dy > 1e-6 {
        t_far = t_far.min((rect.bottom() - origin.y) / dy);
    } else if dy < -1e-6 {
        t_far = t_far.min((rect.top() - origin.y) / dy);
    }
    if !t_far.is_finite() || t_far <= 0.0 {
        return through;
    }
    Pos2::new(origin.x + t_far * dx, origin.y + t_far * dy)
}

/// Return `(p0, p1, p2, median_end, upper_end, lower_end)` in screen space.
fn geometry(
    chart_rect: Rect,
    camera: &Camera,
    points: &[WorldPoint],
) -> Option<(Pos2, Pos2, Pos2, Pos2, Pos2, Pos2)> {
    if points.len() < 2 {
        return None;
    }
    let p0 = world_to_screen(chart_rect, camera, points[0]);
    let p1 = world_to_screen(chart_rect, camera, points[1]);
    if points.len() < 3 {
        // While still placing, show the base segment only.
        return Some((p0, p1, p1, p1, p1, p1));
    }
    let p2 = world_to_screen(chart_rect, camera, points[2]);
    let mid = Pos2::new((p1.x + p2.x) * 0.5, (p1.y + p2.y) * 0.5);

    let median_end = extend_ray_to_rect(p0, mid, chart_rect);

    // Parallel tines: direction p0 → mid, starting from p1 and p2.
    let dx = mid.x - p0.x;
    let dy = mid.y - p0.y;
    let upper_through = Pos2::new(p1.x + dx, p1.y + dy);
    let lower_through = Pos2::new(p2.x + dx, p2.y + dy);
    let upper_end = extend_ray_to_rect(p1, upper_through, chart_rect);
    let lower_end = extend_ray_to_rect(p2, lower_through, chart_rect);

    Some((p0, p1, p2, median_end, upper_end, lower_end))
}

impl DrawingTool for Pitchfork {
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
        style: &DrawingStyle,
        kind_style: &KindStyle,
    ) {
        let Some((p0, p1, p2, m_end, u_end, l_end)) = geometry(chart_rect, camera, points) else {
            return;
        };
        let painter = painter.with_clip_rect(chart_rect);

        // Connector from p1 to p2 (handle bar) shown subtly.
        paint_line(&painter, p1, p2, style);

        if points.len() >= 3 {
            paint_line(&painter, p0, m_end, style);
            paint_line(&painter, p1, u_end, style);
            paint_line(&painter, p2, l_end, style);

            let (fill_enabled, fill_alpha) = match kind_style {
                KindStyle::Pitchfork {
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
                painter.add(egui::Shape::convex_polygon(
                    vec![p1, u_end, l_end, p2],
                    fill,
                    Stroke::NONE,
                ));
            }
        } else {
            // While dragging click 2, show the preview median toward the
            // (single) second point.
            paint_line(&painter, p0, p1, style);
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
        let Some((p0, p1, p2, m_end, u_end, l_end)) = geometry(chart_rect, camera, points) else {
            return;
        };
        let painter = painter.with_clip_rect(chart_rect);
        let stroke = Stroke::new(PREVIEW_WIDTH, PREVIEW_COLOR);
        if points.len() >= 3 {
            painter.line_segment([p1, p2], stroke);
            painter.line_segment([p0, m_end], stroke);
            painter.line_segment([p1, u_end], stroke);
            painter.line_segment([p2, l_end], stroke);
        } else {
            painter.line_segment([p0, p1], stroke);
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
        let Some((p0, p1, p2, m_end, u_end, l_end)) = geometry(chart_rect, camera, points) else {
            return false;
        };
        let mut path = BezPath::new();
        let k = |p: Pos2| Point::new(p.x as f64, p.y as f64);
        path.move_to(k(p1));
        path.line_to(k(p2));
        if points.len() >= 3 {
            path.move_to(k(p0));
            path.line_to(k(m_end));
            path.move_to(k(p1));
            path.line_to(k(u_end));
            path.move_to(k(p2));
            path.line_to(k(l_end));
        }
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
        let Some((p0, p1, p2, m_end, u_end, l_end)) = geometry(chart_rect, camera, points) else {
            return chart_rect;
        };
        let pts = [p0, p1, p2, m_end, u_end, l_end];
        let (mut min_x, mut max_x) = (f32::INFINITY, f32::NEG_INFINITY);
        let (mut min_y, mut max_y) = (f32::INFINITY, f32::NEG_INFINITY);
        for p in pts {
            min_x = min_x.min(p.x);
            max_x = max_x.max(p.x);
            min_y = min_y.min(p.y);
            max_y = max_y.max(p.y);
        }
        Rect::from_min_max(Pos2::new(min_x, min_y), Pos2::new(max_x, max_y))
    }

    fn default_kind_style(&self) -> KindStyle {
        KindStyle::DEFAULT_PITCHFORK
    }
}
