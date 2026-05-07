use egui::{Color32, FontId, Pos2, Rect, Stroke, Vec2};

use super::camera::Camera;
use super::candle::CandleData;
use super::grid::AXIS_WIDTH;

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

    // Snap the vertical crosshair to the nearest candle's x-pixel when data
    // is loaded; fall back to the raw cursor x when the chart is empty
    // (e.g. before any symbol has been picked) so the crosshair still tracks
    // the pointer in that state.
    let cursor_x_pixel = cursor_pos.x - chart_rect.left();
    let candle_index_f = camera.x_offset + cursor_x_pixel as f64 / camera.x_scale;
    let candle_index_rounded = candle_index_f.round() as usize;
    let snapped_candle: Option<(usize, f32)> = if !data.is_empty()
        && candle_index_rounded < data.len()
    {
        let snapped_x_pixel =
            ((candle_index_rounded as f64 - camera.x_offset) * camera.x_scale) as f32;
        Some((candle_index_rounded, chart_rect.left() + snapped_x_pixel))
    } else {
        None
    };
    let (candle_index, snapped_x) = match snapped_candle {
        Some((idx, x)) => (Some(idx), x),
        None => (None, cursor_pos.x),
    };

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
        // Auto-size the badge to its text content, capped so it can never
        // exceed the gutter width (AXIS_WIDTH) minus the right-padding gap.
        const PRICE_LABEL_RIGHT_PAD: f32 = 10.0;
        const BADGE_PAD_X: f32 = 6.0;
        let badge_height = 18.0_f32;
        let text_galley = chart_painter.layout_no_wrap(
            price_text.clone(),
            font.clone(),
            LABEL_TEXT,
        );
        let badge_width = (text_galley.size().x + BADGE_PAD_X * 2.0)
            .min(AXIS_WIDTH - PRICE_LABEL_RIGHT_PAD - 2.0);
        let badge_right = chart_rect.right() - PRICE_LABEL_RIGHT_PAD;
        let price_label_rect = Rect::from_min_size(
            Pos2::new(badge_right - badge_width, cursor_pos.y - badge_height / 2.0),
            Vec2::new(badge_width, badge_height),
        );
        chart_painter.rect_filled(price_label_rect, 3.0, LABEL_BG);
        chart_painter.galley(
            Pos2::new(
                price_label_rect.center().x - text_galley.size().x / 2.0,
                cursor_pos.y - text_galley.size().y / 2.0,
            ),
            text_galley,
            LABEL_TEXT,
        );
    }

    let date_text = match candle_index {
        Some(idx) if idx < data.dates.len() => {
            let full = &data.dates[idx];
            if full.len() >= 10 {
                let yyyy = &full[0..4];
                let mm = &full[5..7];
                let dd = &full[8..10];
                format!("{}/{}/{}", mm, dd, yyyy)
            } else {
                full.to_string()
            }
        }
        _ => String::new(),
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
    let _ = candle_index;
}

/// Paint a dashed vertical line at the world index corresponding to `date`.
/// No OHLC label, no horizontal price line — this is for sync, not focus.
pub fn paint_ghost(
    painter: &egui::Painter,
    chart_rect: Rect,
    camera: &super::camera::Camera,
    data: &super::candle::CandleData,
    date: &str,
) {
    let Some(idx) = data.nearest_index_for_date(date) else {
        return;
    };
    let pixel_x = chart_rect.left() + ((idx as f64 - camera.x_offset) * camera.x_scale) as f32;
    if pixel_x < chart_rect.left() || pixel_x > chart_rect.right() {
        return;
    }
    let stroke = Stroke::new(1.0, Color32::from_rgba_unmultiplied(180, 180, 200, 120));
    let mut y = chart_rect.top();
    while y < chart_rect.bottom() {
        let y2 = (y + 4.0).min(chart_rect.bottom());
        painter.line_segment([egui::pos2(pixel_x, y), egui::pos2(pixel_x, y2)], stroke);
        y += 8.0;
    }
}
