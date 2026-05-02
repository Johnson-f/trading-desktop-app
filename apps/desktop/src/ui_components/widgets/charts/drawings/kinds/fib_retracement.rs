use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Stroke};
use kurbo::{BezPath, Point};

use super::super::super::camera::Camera;
use super::super::hit_test::hit_shape;
use super::super::kind_style::KindStyle;
use super::super::style::{DrawingStyle, style_color};
use super::super::trait_def::{
    DrawingDraft, DrawingTool, InputResult, WorldPoint, world_to_screen,
};
use super::shared::two_click_input;

pub const ID: &str = "fib_retracement";
pub const NAME: &str = "Fib Retracement";
pub const ICON: &str = egui_phosphor::regular::CHART_LINE_DOWN;

const RATIOS: &[f32] = &[0.0, 0.236, 0.382, 0.5, 0.618, 0.786, 1.0];
const LABEL_COLOR: Color32 = Color32::from_rgb(200, 200, 210);
const PREVIEW_COLOR: Color32 = Color32::from_rgba_premultiplied(255, 255, 255, 160);
const PREVIEW_WIDTH: f32 = 1.0;

pub struct FibRetracement;

pub fn factory() -> Box<dyn DrawingTool> {
    Box::new(FibRetracement)
}

impl DrawingTool for FibRetracement {
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
        let Some(levels) = level_screen_ys(chart_rect, camera, points) else {
            return;
        };
        let painter = painter.with_clip_rect(chart_rect);
        let color = style_color(style.color, style.opacity);
        let stroke = Stroke::new(style.width, color);
        let font = FontId::monospace(10.0);
        let show_labels = matches!(kind_style, KindStyle::Fib { show_labels, .. } if *show_labels)
            || !matches!(kind_style, KindStyle::Fib { .. });
        for (idx, (ratio, price, y)) in levels.iter().enumerate() {
            if !kind_style.fib_ratio_enabled(idx) {
                continue;
            }
            let y = *y;
            if y < chart_rect.top() || y > chart_rect.bottom() {
                continue;
            }
            painter.line_segment(
                [
                    Pos2::new(chart_rect.left(), y),
                    Pos2::new(chart_rect.right(), y),
                ],
                stroke,
            );
            if show_labels {
                painter.text(
                    Pos2::new(chart_rect.left() + 4.0, y - 2.0),
                    Align2::LEFT_BOTTOM,
                    format!("{:.3}  {:.2}", ratio, price),
                    font.clone(),
                    LABEL_COLOR,
                );
            }
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
        let Some(levels) = level_screen_ys(chart_rect, camera, points) else {
            return;
        };
        let painter = painter.with_clip_rect(chart_rect);
        let stroke = Stroke::new(PREVIEW_WIDTH, PREVIEW_COLOR);
        for (_, _, y) in levels {
            if y < chart_rect.top() || y > chart_rect.bottom() {
                continue;
            }
            painter.line_segment(
                [
                    Pos2::new(chart_rect.left(), y),
                    Pos2::new(chart_rect.right(), y),
                ],
                stroke,
            );
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
        let Some(levels) = level_screen_ys(chart_rect, camera, points) else {
            return false;
        };
        let mut path = BezPath::new();
        for (_, _, y) in levels {
            path.move_to(Point::new(chart_rect.left() as f64, y as f64));
            path.line_to(Point::new(chart_rect.right() as f64, y as f64));
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
        if points.len() < 2 {
            return Vec::new();
        }
        vec![
            world_to_screen(chart_rect, camera, points[0]),
            world_to_screen(chart_rect, camera, points[1]),
        ]
    }

    fn bounds(
        &self,
        chart_rect: Rect,
        _full_rect: Rect,
        camera: &Camera,
        points: &[WorldPoint],
    ) -> Rect {
        let Some(levels) = level_screen_ys(chart_rect, camera, points) else {
            return chart_rect;
        };
        let (mut top, mut bottom) = (f32::INFINITY, f32::NEG_INFINITY);
        for (_, _, y) in levels {
            top = top.min(y);
            bottom = bottom.max(y);
        }
        Rect::from_min_max(
            Pos2::new(chart_rect.left(), top),
            Pos2::new(chart_rect.right(), bottom),
        )
    }

    fn default_kind_style(&self) -> KindStyle {
        KindStyle::DEFAULT_FIB
    }
}

/// Return `(ratio, price, screen_y)` for each Fibonacci level between the two
/// anchors. Returns `None` if fewer than two anchors are set. Anchor order is
/// normalised internally — the higher price is always the "100%" end.
fn level_screen_ys(
    chart_rect: Rect,
    camera: &Camera,
    points: &[WorldPoint],
) -> Option<Vec<(f32, f32, f32)>> {
    if points.len() < 2 {
        return None;
    }
    let (lo, hi) = {
        let a = points[0].price;
        let b = points[1].price;
        if a <= b { (a, b) } else { (b, a) }
    };
    let range = hi - lo;
    let out: Vec<(f32, f32, f32)> = RATIOS
        .iter()
        .map(|&ratio| {
            let price = lo + ratio * range;
            let y =
                chart_rect.bottom() - ((price as f64 - camera.y_offset) * camera.y_scale) as f32;
            (ratio, price, y)
        })
        .collect();
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::super::super::super::camera::Camera;
    use super::*;
    use egui::{Pos2, Rect};

    fn test_camera() -> Camera {
        let mut c = Camera::default();
        c.x_offset = 0.0;
        c.x_scale = 10.0;
        c.y_offset = 0.0;
        c.y_scale = 10.0;
        c.viewport = egui::Vec2::new(100.0, 100.0);
        c
    }
    fn rect() -> Rect {
        Rect::from_min_max(Pos2::ZERO, Pos2::new(100.0, 100.0))
    }
    // Anchors at prices 2 and 8 → screen y 80 (100%) and 20 (0%).
    fn pts() -> Vec<WorldPoint> {
        vec![
            WorldPoint {
                index: 1.0,
                price: 2.0,
            },
            WorldPoint {
                index: 6.0,
                price: 8.0,
            },
        ]
    }

    #[test]
    fn hit_test_lands_on_zero_level() {
        let t = FibRetracement;
        // 0% ratio sits at price=2 → screen y = 100 - 20 = 80.
        assert!(t.hit_test(
            rect(),
            rect(),
            &test_camera(),
            &pts(),
            Pos2::new(50.0, 80.0),
            4.0
        ));
    }

    #[test]
    fn hit_test_lands_on_full_level() {
        let t = FibRetracement;
        // 100% ratio sits at price=8 → screen y = 100 - 80 = 20.
        assert!(t.hit_test(
            rect(),
            rect(),
            &test_camera(),
            &pts(),
            Pos2::new(50.0, 20.0),
            4.0
        ));
    }

    #[test]
    fn hit_test_lands_on_half_level() {
        let t = FibRetracement;
        // 50% ratio sits at price=5 → screen y = 100 - 50 = 50.
        assert!(t.hit_test(
            rect(),
            rect(),
            &test_camera(),
            &pts(),
            Pos2::new(50.0, 50.0),
            4.0
        ));
    }

    #[test]
    fn hit_test_misses_between_levels() {
        let t = FibRetracement;
        // Between 23.6% (y≈65.84) and 0% (y=80) — gap of ~14 screen pixels.
        // At y=73 the nearest level is ~7px away, well outside tolerance=2.
        assert!(!t.hit_test(
            rect(),
            rect(),
            &test_camera(),
            &pts(),
            Pos2::new(50.0, 73.0),
            2.0
        ));
    }

    #[test]
    fn anchor_order_does_not_matter() {
        let t = FibRetracement;
        let reversed = vec![pts()[1], pts()[0]];
        // Same 0% level whether anchors were entered low-first or high-first.
        assert!(t.hit_test(
            rect(),
            rect(),
            &test_camera(),
            &reversed,
            Pos2::new(50.0, 80.0),
            4.0
        ));
    }

    #[test]
    fn handles_returns_two_anchors() {
        let t = FibRetracement;
        assert_eq!(t.handles(rect(), rect(), &test_camera(), &pts()).len(), 2);
    }
}
