use egui::{Color32, FontId, Pos2, Rect, Vec2};

use super::super::camera::Camera;
use super::super::candle::CandleData;

const VOL_UP: Color32 = Color32::from_rgba_premultiplied(78, 205, 196, 128);
const VOL_DOWN: Color32 = Color32::from_rgba_premultiplied(255, 107, 107, 128);
const LABEL_COLOR: Color32 = Color32::from_rgb(100, 100, 110);
const PANE_BG: Color32 = Color32::from_rgb(18, 18, 22);

pub fn paint_volume(ui: &egui::Ui, volume_rect: Rect, camera: &Camera, data: &CandleData) {
    if data.len() == 0 {
        return;
    }

    let painter = ui.painter_at(volume_rect);
    painter.rect_filled(volume_rect, 0.0, PANE_BG);

    let start = (camera.x_offset as usize).max(0);
    let visible_count = (volume_rect.width() as f64 / camera.x_scale).ceil() as usize;
    let end = (start + visible_count + 2).min(data.len());

    if start >= end {
        return;
    }

    let mut max_vol: f32 = 0.0;
    for i in start..end {
        let v = data.instances[i].volume;
        if v > max_vol { max_vol = v; }
    }
    if max_vol <= 0.0 {
        return;
    }

    let vol_height = volume_rect.height();

    for i in start..end {
        let candle = &data.instances[i];
        let x_pixel = ((i as f64 - camera.x_offset) * camera.x_scale) as f32;
        let bar_width = (camera.x_scale as f32 * 0.6).max(1.0);
        let bar_height = (candle.volume / max_vol) * vol_height * 0.9;

        let bar_left = volume_rect.left() + x_pixel - bar_width / 2.0;
        let bar_top = volume_rect.bottom() - bar_height;

        let bar_rect = Rect::from_min_size(
            Pos2::new(bar_left, bar_top),
            Vec2::new(bar_width, bar_height),
        );

        if bar_rect.right() < volume_rect.left() || bar_rect.left() > volume_rect.right() {
            continue;
        }

        let color = if candle.close >= candle.open { VOL_UP } else { VOL_DOWN };
        painter.rect_filled(bar_rect, 0.0, color);
    }

    // Y-axis labels
    let label_font = FontId::monospace(9.0);
    let label_x = volume_rect.right() - 50.0;

    painter.text(
        Pos2::new(label_x, volume_rect.top() + 10.0),
        egui::Align2::LEFT_CENTER,
        format_volume(max_vol),
        label_font.clone(),
        LABEL_COLOR,
    );

    painter.text(
        Pos2::new(label_x, volume_rect.center().y),
        egui::Align2::LEFT_CENTER,
        format_volume(max_vol / 2.0),
        label_font,
        LABEL_COLOR,
    );
}

pub fn format_volume(vol: f32) -> String {
    if vol >= 1_000_000_000.0 {
        format!("{:.1}B", vol / 1_000_000_000.0)
    } else if vol >= 1_000_000.0 {
        format!("{:.1}M", vol / 1_000_000.0)
    } else if vol >= 1_000.0 {
        format!("{:.0}K", vol / 1_000.0)
    } else {
        format!("{}", vol as i64)
    }
}
