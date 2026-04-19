use eframe::egui::{Color32, Painter, Pos2, Rect, Stroke};
use serde::{Deserialize, Serialize};
use zaned_chart_core::Rgba;

pub const DEFAULT_COLOR: Rgba = Rgba([255, 255, 255, 255]);

/// Visual style for a committed drawing. Preview rendering does NOT read this
/// — drafts use hard-coded faded constants because they have no committed
/// style yet.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct DrawingStyle {
    pub color: Rgba,
    pub width: f32,
    pub dash: DashStyle,
    pub opacity: f32,
    pub extend_left: bool,
    pub extend_right: bool,
}

impl Default for DrawingStyle {
    fn default() -> Self {
        Self {
            color: DEFAULT_COLOR,
            width: 1.5,
            dash: DashStyle::Solid,
            opacity: 1.0,
            extend_left: false,
            extend_right: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum DashStyle {
    Solid,
    Dashed,
    Dotted,
}

/// Width presets surfaced in the toolbar width popup.
pub const WIDTH_PRESETS: &[f32] = &[1.0, 1.5, 2.5];

/// Color palette surfaced in the toolbar color popup. Ordered warm → cool.
pub const COLOR_PALETTE: &[Rgba] = &[
    Rgba([255, 255, 255, 255]), // white (default)
    Rgba([255, 193, 7, 255]),   // amber
    Rgba([255, 107, 107, 255]), // red
    Rgba([236, 72, 153, 255]),  // pink
    Rgba([168, 85, 247, 255]),  // purple
    Rgba([88, 166, 255, 255]),  // blue
    Rgba([34, 211, 238, 255]),  // cyan
    Rgba([78, 205, 196, 255]),  // teal
];

/// Convert a chart-core `Rgba` into an `egui::Color32`, scaling alpha by
/// `opacity` (clamped to 0.0..=1.0). egui premultiplies internally.
pub fn style_color(color: Rgba, opacity: f32) -> Color32 {
    let [r, g, b, a] = color.0;
    let a_scaled = ((a as f32) * opacity.clamp(0.0, 1.0)) as u8;
    Color32::from_rgba_unmultiplied(r, g, b, a_scaled)
}

/// Draw a line segment honoring `DrawingStyle::dash`.
pub fn paint_line(painter: &Painter, a: Pos2, b: Pos2, style: &DrawingStyle) {
    let stroke = Stroke::new(style.width, style_color(style.color, style.opacity));
    match style.dash {
        DashStyle::Solid => {
            painter.line_segment([a, b], stroke);
        }
        DashStyle::Dashed => paint_dashed_segment(painter, a, b, stroke, 8.0, 4.0),
        DashStyle::Dotted => paint_dashed_segment(painter, a, b, stroke, 2.0, 3.0),
    }
}

/// Draw a horizontal line spanning `rect.x_range()` at screen-space `y`,
/// honoring dash style.
pub fn paint_hline(painter: &Painter, rect: Rect, y: f32, style: &DrawingStyle) {
    paint_line(
        painter,
        Pos2::new(rect.left(), y),
        Pos2::new(rect.right(), y),
        style,
    );
}

/// Draw a vertical line spanning `rect.y_range()` at screen-space `x`,
/// honoring dash style.
pub fn paint_vline(painter: &Painter, rect: Rect, x: f32, style: &DrawingStyle) {
    paint_line(
        painter,
        Pos2::new(x, rect.top()),
        Pos2::new(x, rect.bottom()),
        style,
    );
}

fn paint_dashed_segment(
    painter: &Painter,
    a: Pos2,
    b: Pos2,
    stroke: Stroke,
    dash_len: f32,
    gap_len: f32,
) {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let total = (dx * dx + dy * dy).sqrt();
    if total < 1e-3 {
        return;
    }
    let period = dash_len + gap_len;
    let ux = dx / total;
    let uy = dy / total;
    let mut t = 0.0_f32;
    while t < total {
        let start = Pos2::new(a.x + ux * t, a.y + uy * t);
        let end_t = (t + dash_len).min(total);
        let end = Pos2::new(a.x + ux * end_t, a.y + uy * end_t);
        painter.line_segment([start, end], stroke);
        t += period;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drawing_style_serde_roundtrip() {
        let original = DrawingStyle {
            color: Rgba([10, 20, 30, 255]),
            width: 2.5,
            dash: DashStyle::Dashed,
            opacity: 0.75,
            extend_left: true,
            extend_right: false,
        };
        let json = serde_json::to_string(&original).unwrap();
        let parsed: DrawingStyle = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, original);
    }
}
