use egui::{Color32, FontId, Pos2, Rect, Stroke, Vec2};

use super::camera::Camera;
use super::candle::CandleData;

const CROSSHAIR_COLOR: Color32 = Color32::from_rgb(80, 80, 90);
const LABEL_BG: Color32 = Color32::from_rgb(40, 40, 46);
const LABEL_TEXT: Color32 = Color32::from_rgb(200, 200, 210);

/// Paints the crosshair across the main chart and the sub-pane stack.
/// `sub_rect` is the combined rectangle covering all sub-panes (or equals
/// `chart_rect` when there are no sub-panes).
pub fn paint_crosshair(
    ui: &egui::Ui,
    chart_rect: Rect,
    sub_rect: Rect,
    camera: &Camera,
    data: &CandleData,
) {
    let full_rect = Rect::from_min_max(chart_rect.min, sub_rect.max);
    let cursor_pos = match ui.input(|i| i.pointer.latest_pos()) {
        Some(pos) if full_rect.contains(pos) => pos,
        _ => return,
    };

    let cursor_x_pixel = cursor_pos.x - chart_rect.left();
    let candle_index_f = camera.x_offset + cursor_x_pixel as f64 / camera.x_scale;
    let candle_index = candle_index_f.round() as usize;
    if candle_index >= data.len() {
        return;
    }
    let candle = &data.instances[candle_index];
    let snapped_x_pixel = ((candle_index as f64 - camera.x_offset) * camera.x_scale) as f32;
    let snapped_x = chart_rect.left() + snapped_x_pixel;

    let full_painter = ui.painter_at(full_rect);
    full_painter.line_segment(
        [
            Pos2::new(snapped_x, full_rect.top()),
            Pos2::new(snapped_x, full_rect.bottom()),
        ],
        Stroke::new(0.5, CROSSHAIR_COLOR),
    );

    let in_chart = chart_rect.contains(cursor_pos);
    if in_chart {
        let chart_painter = ui.painter_at(chart_rect);
        chart_painter.line_segment(
            [
                Pos2::new(chart_rect.left(), cursor_pos.y),
                Pos2::new(chart_rect.right(), cursor_pos.y),
            ],
            Stroke::new(0.5, CROSSHAIR_COLOR),
        );
        let cursor_y_pixel = chart_rect.bottom() - cursor_pos.y;
        let price = camera.y_offset + cursor_y_pixel as f64 / camera.y_scale;
        let price_text = format!("{:.2}", price);
        let font = FontId::monospace(11.0);
        let label_size = Vec2::new(70.0, 18.0);
        let price_label_rect = Rect::from_min_size(
            Pos2::new(
                chart_rect.right() - label_size.x - 4.0,
                cursor_pos.y - label_size.y / 2.0,
            ),
            label_size,
        );
        chart_painter.rect_filled(price_label_rect, 3.0, LABEL_BG);
        chart_painter.text(
            price_label_rect.center(),
            egui::Align2::CENTER_CENTER,
            &price_text,
            font,
            LABEL_TEXT,
        );
    }

    let date_text = if candle_index < data.dates.len() {
        let full = &data.dates[candle_index];
        if full.len() >= 10 {
            let yyyy = &full[0..4];
            let mm = &full[5..7];
            let dd = &full[8..10];
            format!("{}/{}/{}", mm, dd, yyyy)
        } else {
            full.to_string()
        }
    } else {
        String::new()
    };
    if !date_text.is_empty() {
        let font = FontId::monospace(11.0);
        let date_label_size = Vec2::new(80.0, 18.0);
        let date_label_rect = Rect::from_min_size(
            Pos2::new(
                snapped_x - date_label_size.x / 2.0,
                full_rect.bottom() - date_label_size.y - 4.0,
            ),
            date_label_size,
        );
        full_painter.rect_filled(date_label_rect, 3.0, LABEL_BG);
        full_painter.text(
            date_label_rect.center(),
            egui::Align2::CENTER_CENTER,
            date_text,
            font,
            LABEL_TEXT,
        );
    }

    // OHLCV tooltip intentionally omitted — `paint_ohlc_row` in the chart's
    // main overlay draws a persistent header that already updates with the
    // cursor candle, so drawing a second tooltip here would just overlap it.
    let _ = candle;
}
