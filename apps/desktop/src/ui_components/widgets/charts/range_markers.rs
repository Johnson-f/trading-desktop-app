use egui::{Align2, Color32, FontId, Pos2, Rect};

use super::camera::Camera;
use super::candle::CandleData;

const LABEL_COLOR: Color32 = Color32::from_rgb(200, 200, 210);

pub fn paint(ui: &egui::Ui, chart_rect: Rect, camera: &Camera, data: &CandleData) {
    if data.is_empty() {
        return;
    }

    let start = camera.x_offset as usize;
    let visible_count = (camera.viewport.x as f64 / camera.x_scale).ceil() as usize;
    let end = (start + visible_count + 2).min(data.len());
    if start >= end {
        return;
    }

    let mut max_high = f32::MIN;
    let mut max_high_idx = start;
    let mut min_low = f32::MAX;
    let mut min_low_idx = start;

    for i in start..end {
        let c = &data.instances[i];
        if c.high > max_high {
            max_high = c.high;
            max_high_idx = i;
        }
        if c.low < min_low {
            min_low = c.low;
            min_low_idx = i;
        }
    }

    let painter = ui.painter_at(chart_rect);
    let font = FontId::proportional(10.0);

    let x_h = chart_rect.left() + ((max_high_idx as f64 - camera.x_offset) * camera.x_scale) as f32;
    let y_h = chart_rect.bottom() - ((max_high as f64 - camera.y_offset) * camera.y_scale) as f32;

    let x_l = chart_rect.left() + ((min_low_idx as f64 - camera.x_offset) * camera.x_scale) as f32;
    let y_l = chart_rect.bottom() - ((min_low as f64 - camera.y_offset) * camera.y_scale) as f32;

    // Top of the chart is reserved for the OHLCV header (anchored at top + 8,
    // ~12 px tall). Label flips to below the candle when it would land in that
    // strip; symmetrically for the L label and the bottom edge.
    const HEADER_RESERVED: f32 = 28.0;
    const FOOTER_RESERVED: f32 = 12.0;

    if chart_rect.x_range().contains(x_h) && chart_rect.y_range().contains(y_h) {
        let above = y_h - 4.0 >= chart_rect.top() + HEADER_RESERVED;
        let (pos, align) = if above {
            (Pos2::new(x_h, y_h - 4.0), Align2::CENTER_BOTTOM)
        } else {
            (Pos2::new(x_h, y_h + 4.0), Align2::CENTER_TOP)
        };
        painter.text(
            pos,
            align,
            format!("H {:.2}", max_high),
            font.clone(),
            LABEL_COLOR,
        );
    }

    if chart_rect.x_range().contains(x_l) && chart_rect.y_range().contains(y_l) {
        let below = y_l + 4.0 <= chart_rect.bottom() - FOOTER_RESERVED;
        let (pos, align) = if below {
            (Pos2::new(x_l, y_l + 4.0), Align2::CENTER_TOP)
        } else {
            (Pos2::new(x_l, y_l - 4.0), Align2::CENTER_BOTTOM)
        };
        painter.text(pos, align, format!("L {:.2}", min_low), font, LABEL_COLOR);
    }
}
