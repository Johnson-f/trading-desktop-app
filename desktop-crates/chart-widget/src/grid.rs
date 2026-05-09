use egui::{Color32, FontId, Pos2, Rect, Stroke, Vec2};

use super::camera::Camera;
use super::candle::CandleData;

const LABEL_COLOR: Color32 = Color32::from_rgb(120, 120, 130);
const CURRENT_PRICE_BG: Color32 = Color32::from_rgb(78, 205, 196);
const CURRENT_PRICE_TEXT: Color32 = Color32::from_rgb(0, 0, 0);
pub const AXIS_WIDTH: f32 = 58.0;
/// Padding between the rightmost price digit and the window edge (Webull-style).
const PRICE_LABEL_RIGHT_PAD: f32 = 10.0;
/// Horizontal padding inside the last-price badge (per side).
const BADGE_PAD_X: f32 = 8.0;

/// Paint price labels on the right edge (no grid lines).
/// The actual gutter chrome (mask + divider + labels + badge) has been moved
/// to `paint_price_axis`, which must be called AFTER all other chart-area
/// paint passes so the gutter always renders on top.
pub fn paint_price_grid(_ui: &egui::Ui, _chart_rect: Rect, _camera: &Camera, _data: &CandleData) {
    // No-op: horizontal grid lines are not drawn in this chart style.
    // The gutter chrome is painted by `paint_price_axis` at the end of show().
}

/// Paint the full price-scale gutter chrome: opaque BG mask, vertical hairline
/// divider, tick price labels, and the last-price highlighted badge.
///
/// **Must be called LAST** in the `show()` paint sequence — after drawings,
/// crosshair, and ghost — so the gutter always renders on top of anything that
/// bleeds into the right-side gutter region.
pub fn paint_price_axis(ui: &egui::Ui, chart_rect: Rect, camera: &Camera, data: &CandleData) {
    if data.len() == 0 {
        return;
    }

    let painter = ui.painter_at(chart_rect);
    let font = FontId::monospace(10.0);

    // Vertical hairline divider at the left edge of the price-scale gutter.
    let gutter_x = chart_rect.right() - AXIS_WIDTH;

    // 1. Opaque fill over the gutter to mask any pixels that bleed in
    //    (WGPU candles, drawings, crosshair lines, ghost cursors, …).
    let gutter_rect = egui::Rect::from_min_max(
        Pos2::new(gutter_x, chart_rect.top()),
        Pos2::new(chart_rect.right(), chart_rect.bottom()),
    );
    painter.rect_filled(gutter_rect, 0.0, zaned_theme::BG);

    // 2. Vertical hairline divider on the gutter's left edge.
    painter.line_segment(
        [
            Pos2::new(gutter_x, chart_rect.top()),
            Pos2::new(gutter_x, chart_rect.bottom()),
        ],
        Stroke::new(1.0, zaned_theme::BORDER),
    );

    let price_bottom = camera.y_offset;
    let price_top = camera.y_offset + chart_rect.height() as f64 / camera.y_scale;
    let price_range = price_top - price_bottom;

    if price_range <= 0.0 {
        return;
    }

    let target_label_count = (chart_rect.height() / 60.0) as usize;
    let interval = nice_interval(price_range, target_label_count.max(2));

    let first = (price_bottom / interval).ceil() * interval;

    // 3. Tick price labels (290, 280, 270, …).
    let mut price = first;
    while price < price_top {
        let y_pixel = ((price - camera.y_offset) * camera.y_scale) as f32;
        let y = chart_rect.bottom() - y_pixel;

        if y > chart_rect.top() + 10.0 && y < chart_rect.bottom() - 10.0 {
            // Price label right-aligned with PRICE_LABEL_RIGHT_PAD from the window edge.
            let label_text = format_price(price, interval);
            painter.text(
                Pos2::new(chart_rect.right() - PRICE_LABEL_RIGHT_PAD, y),
                egui::Align2::RIGHT_CENTER,
                &label_text,
                font.clone(),
                LABEL_COLOR,
            );
        }

        price += interval;
    }

    // 4. Last-price highlighted badge (e.g. mint-green "276.83").
    if let Some(last) = data.instances.last() {
        let close = last.close as f64;
        let y_pixel = ((close - camera.y_offset) * camera.y_scale) as f32;
        let y = chart_rect.bottom() - y_pixel;

        if y > chart_rect.top() && y < chart_rect.bottom() {
            // Dashed line from last candle to the price label
            let last_index = (data.len() - 1) as f64;
            let last_x_pixel = ((last_index - camera.x_offset) * camera.x_scale) as f32;
            let dash_start_x = chart_rect.left() + last_x_pixel;
            let dash_end_x = gutter_x;
            let dash_len = 1.0;
            let gap_len = 2.0;
            let mut x = dash_start_x;
            while x < dash_end_x {
                let seg_end = (x + dash_len).min(dash_end_x);
                painter.line_segment(
                    [Pos2::new(x, y), Pos2::new(seg_end, y)],
                    Stroke::new(0.2, CURRENT_PRICE_BG),
                );
                x += dash_len + gap_len;
            }

            // Highlighted price label — auto-sized to fit the price text with
            // tight padding, right-aligned with PRICE_LABEL_RIGHT_PAD inset.
            let label_text = format!("{:.2}", close);
            let text_galley =
                painter.layout_no_wrap(label_text.clone(), font.clone(), CURRENT_PRICE_TEXT);
            let badge_width = text_galley.size().x + BADGE_PAD_X * 2.0;
            let badge_right = chart_rect.right() - PRICE_LABEL_RIGHT_PAD;
            let label_rect = Rect::from_min_size(
                Pos2::new(badge_right - badge_width, y - 10.0),
                Vec2::new(badge_width, 20.0),
            );
            painter.rect_filled(label_rect, 3.0, CURRENT_PRICE_BG);
            painter.text(
                label_rect.center(),
                egui::Align2::CENTER_CENTER,
                &label_text,
                font.clone(),
                CURRENT_PRICE_TEXT,
            );
        }
    }
}

/// Paint time labels along the bottom of the chart. The label format
/// depends on the active [`BaseScale`]:
///   - `Daily`  → month names + year markers (e.g. "Jan", "Feb",
///                "2025"). Labels span large date ranges cleanly.
///   - `Minute` → time-of-day (e.g. "09:30", "10:00") with date
///                markers on day boundaries (e.g. "May 8"). Labels
///                give intraday charts the H:MM granularity users
///                expect.
///
/// Both modes share the same visual chrome — fade-in around the
/// minimum-label-gap threshold so labels don't pop as the user zooms.
pub fn paint_time_grid(
    ui: &egui::Ui,
    chart_rect: Rect,
    camera: &Camera,
    data: &CandleData,
    scale: zaned_chart_core::BaseScale,
) {
    if data.len() == 0 {
        return;
    }

    let painter = ui.painter_at(chart_rect);
    let font = FontId::monospace(9.0);
    let primary_font = FontId::monospace(10.0);

    let start = (camera.x_offset as usize).max(0);
    let visible_count = (chart_rect.width() as f64 / camera.x_scale).ceil() as usize;
    let end = (start + visible_count + 1).min(data.len());

    if start >= end {
        return;
    }

    // Smooth zoom transitions: instead of a binary "draw if gap > 60px",
    // we fade labels in/out across a 30px window starting at the minimum
    // gap. As the user zooms, a label whose gap to its predecessor is
    // just above the threshold ramps from invisible → fully opaque,
    // instead of popping in. Same idea on zoom-out.
    let min_label_gap: f32 = 60.0;
    let fade_range: f32 = 30.0;
    let primary_color = Color32::from_rgb(180, 180, 190);

    // Per-scale state. For daily we track (year, month). For minute we
    // track (date, hour-bucket) so the date label promotes to "May 8"
    // on day boundaries and times align to clean clock points (every
    // minute, every 5 min, etc., depending on zoom).
    let mut last_year = String::new();
    let mut last_month = String::new();
    let mut last_date = String::new();
    let mut last_hour_label = String::new();
    let mut last_label_x: f32 = f32::MIN;

    for i in start..end {
        if i >= data.dates.len() {
            continue;
        }

        let date = &data.dates[i];
        if date.len() < 10 {
            continue;
        }

        let x_pixel = ((i as f64 - camera.x_offset) * camera.x_scale) as f32;
        let x = chart_rect.left() + x_pixel;

        if x < chart_rect.left() + 20.0 || x > chart_rect.right() - AXIS_WIDTH - 10.0 {
            continue;
        }

        let gap = x - last_label_x;
        let alpha = ((gap - min_label_gap) / fade_range).clamp(0.0, 1.0);
        if alpha <= 0.0 {
            continue;
        }

        match scale {
            zaned_chart_core::BaseScale::Daily => {
                let yyyy = &date[0..4];
                let mm = &date[5..7];

                // Year marker takes priority over month marker on a
                // Jan candle.
                if yyyy != last_year {
                    last_year = yyyy.to_string();
                    last_month = format!("{yyyy}-{mm}");
                    last_label_x = x;

                    painter.text(
                        Pos2::new(x, chart_rect.bottom() - 4.0),
                        egui::Align2::CENTER_BOTTOM,
                        yyyy,
                        primary_font.clone(),
                        fade_color(primary_color, alpha),
                    );
                    continue;
                }

                let month_key = format!("{yyyy}-{mm}");
                if month_key != last_month {
                    last_month = month_key;
                    last_label_x = x;
                    painter.text(
                        Pos2::new(x, chart_rect.bottom() - 4.0),
                        egui::Align2::CENTER_BOTTOM,
                        month_short(mm),
                        font.clone(),
                        fade_color(LABEL_COLOR, alpha),
                    );
                }
            }
            zaned_chart_core::BaseScale::Minute => {
                // Expect `YYYY-MM-DD HH:MM` (16 chars). If shorter,
                // fall back to date-only and skip the time component.
                let day = &date[0..10];
                let time = if date.len() >= 16 { &date[11..16] } else { "" };

                // Day boundary takes priority — render "May 8" so the
                // user can see the calendar boundary even on intraday.
                if day != last_date {
                    last_date = day.to_string();
                    last_hour_label = time.to_string();
                    last_label_x = x;

                    let mm = &day[5..7];
                    let dd_str = &day[8..10];
                    let dd_trim = dd_str.trim_start_matches('0');
                    let label = format!("{} {}", month_short(mm), dd_trim);
                    painter.text(
                        Pos2::new(x, chart_rect.bottom() - 4.0),
                        egui::Align2::CENTER_BOTTOM,
                        label,
                        primary_font.clone(),
                        fade_color(primary_color, alpha),
                    );
                    continue;
                }

                if !time.is_empty() && time != last_hour_label {
                    last_hour_label = time.to_string();
                    last_label_x = x;
                    painter.text(
                        Pos2::new(x, chart_rect.bottom() - 4.0),
                        egui::Align2::CENTER_BOTTOM,
                        time,
                        font.clone(),
                        fade_color(LABEL_COLOR, alpha),
                    );
                }
            }
        }
    }
}

/// Short English month name from a `MM` string. Returns the input on
/// unknown values so callers fall back gracefully on garbage data.
fn month_short(mm: &str) -> &str {
    match mm {
        "01" => "Jan",
        "02" => "Feb",
        "03" => "Mar",
        "04" => "Apr",
        "05" => "May",
        "06" => "Jun",
        "07" => "Jul",
        "08" => "Aug",
        "09" => "Sep",
        "10" => "Oct",
        "11" => "Nov",
        "12" => "Dec",
        _ => mm,
    }
}

/// Multiply a color's alpha by `factor` (clamped 0..1). Used to fade
/// time-axis labels in/out as the user zooms.
fn fade_color(base: Color32, factor: f32) -> Color32 {
    let f = factor.clamp(0.0, 1.0);
    let a = (base.a() as f32 * f) as u8;
    Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), a)
}

/// Handle drag on the price axis (right 65px) to manually scale Y. Returns true if auto_scale should be disabled.
pub fn handle_price_axis_drag(ui: &egui::Ui, chart_rect: Rect, camera: &mut Camera) {
    let axis_rect = Rect::from_min_max(
        Pos2::new(chart_rect.right() - AXIS_WIDTH, chart_rect.top()),
        Pos2::new(chart_rect.right(), chart_rect.bottom()),
    );

    let response = ui.interact(
        axis_rect,
        ui.id().with("price_axis_drag"),
        egui::Sense::click_and_drag(),
    );

    if response.hovered() || response.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }

    if response.dragged() {
        camera.auto_scale_y = false;
        let delta = response.drag_delta().y;
        // Drag up = zoom in (increase y_scale), drag down = zoom out
        let factor = 1.0 + (delta as f64 * 0.003);
        let center_price = camera.y_offset + (chart_rect.height() as f64 / 2.0) / camera.y_scale;

        camera.y_scale *= factor;
        camera.y_scale = camera.y_scale.clamp(0.01, 100000.0);

        // Keep center price stable
        camera.y_offset = center_price - (chart_rect.height() as f64 / 2.0) / camera.y_scale;
    }

    // Double-click on price axis = reset auto-scale
    if response.double_clicked() {
        camera.auto_scale_y = true;
    }
}

fn nice_interval(range: f64, target_count: usize) -> f64 {
    let rough = range / target_count as f64;
    let magnitude = 10.0_f64.powf(rough.log10().floor());
    let residual = rough / magnitude;

    let nice = if residual <= 1.5 {
        1.0
    } else if residual <= 3.0 {
        2.0
    } else if residual <= 7.0 {
        5.0
    } else {
        10.0
    };

    nice * magnitude
}

fn format_price(price: f64, interval: f64) -> String {
    if interval >= 1.0 {
        format!("{:.0}", price)
    } else if interval >= 0.1 {
        format!("{:.1}", price)
    } else {
        format!("{:.2}", price)
    }
}
