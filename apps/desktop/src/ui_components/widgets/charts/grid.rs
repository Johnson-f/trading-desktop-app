use egui::{Color32, FontId, Pos2, Rect, Stroke, Vec2};

use super::camera::Camera;
use super::candle::CandleData;

const LABEL_COLOR: Color32 = Color32::from_rgb(120, 120, 130);
const CURRENT_PRICE_BG: Color32 = Color32::from_rgb(78, 205, 196);
const CURRENT_PRICE_TEXT: Color32 = Color32::from_rgb(0, 0, 0);
const AXIS_WIDTH: f32 = 65.0;

/// Paint price labels on the right edge (no grid lines)
pub fn paint_price_grid(ui: &egui::Ui, chart_rect: Rect, camera: &Camera, data: &CandleData) {
    if data.len() == 0 {
        return;
    }

    let painter = ui.painter_at(chart_rect);
    let font = FontId::monospace(10.0);

    let price_bottom = camera.y_offset;
    let price_top = camera.y_offset + chart_rect.height() as f64 / camera.y_scale;
    let price_range = price_top - price_bottom;

    if price_range <= 0.0 {
        return;
    }

    let target_label_count = (chart_rect.height() / 60.0) as usize;
    let interval = nice_interval(price_range, target_label_count.max(2));

    let first = (price_bottom / interval).ceil() * interval;

    let mut price = first;
    while price < price_top {
        let y_pixel = ((price - camera.y_offset) * camera.y_scale) as f32;
        let y = chart_rect.bottom() - y_pixel;

        if y > chart_rect.top() + 10.0 && y < chart_rect.bottom() - 10.0 {
            // Price label on right edge only — no grid line
            let label_text = format_price(price, interval);
            painter.text(
                Pos2::new(chart_rect.right() - AXIS_WIDTH + 8.0, y),
                egui::Align2::LEFT_CENTER,
                &label_text,
                font.clone(),
                LABEL_COLOR,
            );
        }

        price += interval;
    }

    // Current price: short dashed line stub + highlighted label
    if let Some(last) = data.instances.last() {
        let close = last.close as f64;
        let y_pixel = ((close - camera.y_offset) * camera.y_scale) as f32;
        let y = chart_rect.bottom() - y_pixel;

        if y > chart_rect.top() && y < chart_rect.bottom() {
            // Dashed line from last candle to the price label
            let last_index = (data.len() - 1) as f64;
            let last_x_pixel = ((last_index - camera.x_offset) * camera.x_scale) as f32;
            let dash_start_x = chart_rect.left() + last_x_pixel;
            let dash_end_x = chart_rect.right() - AXIS_WIDTH;
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

            // Highlighted price label
            let label_text = format!("{:.2}", close);
            let label_rect = Rect::from_min_size(
                Pos2::new(chart_rect.right() - AXIS_WIDTH, y - 10.0),
                Vec2::new(AXIS_WIDTH, 20.0),
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

/// Paint time labels along the bottom — month names + year markers
pub fn paint_time_grid(ui: &egui::Ui, chart_rect: Rect, camera: &Camera, data: &CandleData) {
    if data.len() == 0 {
        return;
    }

    let painter = ui.painter_at(chart_rect);
    let font = FontId::monospace(9.0);
    let year_font = FontId::monospace(10.0);

    let start = (camera.x_offset as usize).max(0);
    let visible_count = (chart_rect.width() as f64 / camera.x_scale).ceil() as usize;
    let end = (start + visible_count + 1).min(data.len());

    if start >= end {
        return;
    }

    // Track what we've already labeled to avoid duplicates
    let mut last_month = String::new();
    let mut last_year = String::new();
    let mut last_label_x: f32 = f32::MIN;
    let min_label_gap = 60.0; // minimum pixels between labels

    for i in start..end {
        if i >= data.dates.len() {
            continue;
        }

        let date = &data.dates[i];
        if date.len() < 10 {
            continue;
        }

        let yyyy = &date[0..4];
        let mm = &date[5..7];

        let x_pixel = ((i as f64 - camera.x_offset) * camera.x_scale) as f32;
        let x = chart_rect.left() + x_pixel;

        if x < chart_rect.left() + 20.0 || x > chart_rect.right() - AXIS_WIDTH - 10.0 {
            continue;
        }

        let month_key = format!("{}-{}", yyyy, mm);

        // Year marker: first candle of a new year
        if yyyy != last_year && x - last_label_x > min_label_gap {
            last_year = yyyy.to_string();
            last_month = month_key.clone();
            last_label_x = x;

            painter.text(
                Pos2::new(x, chart_rect.bottom() - 4.0),
                egui::Align2::CENTER_BOTTOM,
                yyyy,
                year_font.clone(),
                Color32::from_rgb(180, 180, 190), // brighter for years
            );
            continue;
        }

        // Month marker: first candle of a new month
        if month_key != last_month && x - last_label_x > min_label_gap {
            last_month = month_key;
            last_label_x = x;

            let month_name = match mm {
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
            };

            painter.text(
                Pos2::new(x, chart_rect.bottom() - 4.0),
                egui::Align2::CENTER_BOTTOM,
                month_name,
                font.clone(),
                LABEL_COLOR,
            );
        }
    }
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

/// Paint "Auto" button at the bottom-right of the chart. Returns true if clicked.
pub fn paint_auto_button(ui: &egui::Ui, chart_rect: Rect, camera: &mut Camera) {
    let btn_size = Vec2::new(36.0, 16.0);
    let btn_rect = Rect::from_min_size(
        Pos2::new(
            chart_rect.right() - AXIS_WIDTH + (AXIS_WIDTH - btn_size.x) / 2.0,
            chart_rect.bottom() - btn_size.y - 6.0,
        ),
        btn_size,
    );

    let response = ui.interact(
        btn_rect,
        ui.id().with("auto_scale_btn"),
        egui::Sense::click(),
    );
    let painter = ui.painter_at(chart_rect);

    let is_auto = camera.auto_scale_y;

    let bg = if is_auto {
        Color32::TRANSPARENT
    } else if response.hovered() {
        Color32::from_rgb(35, 35, 40)
    } else {
        Color32::from_rgb(28, 28, 32)
    };

    let text_color = if is_auto {
        Color32::from_rgb(78, 205, 196) // teal when active
    } else {
        LABEL_COLOR
    };

    let border = if is_auto {
        Stroke::new(1.0, Color32::from_rgb(78, 205, 196))
    } else {
        Stroke::new(0.5, Color32::from_rgb(50, 50, 55))
    };

    painter.rect_filled(btn_rect, 3.0, bg);
    painter.rect_stroke(btn_rect, 3.0, border, egui::StrokeKind::Outside);
    painter.text(
        btn_rect.center(),
        egui::Align2::CENTER_CENTER,
        "Auto",
        FontId::proportional(9.0),
        text_color,
    );

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    if response.clicked() && !is_auto {
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
