use egui::{Pos2, Rect};

use super::camera::Camera;
use super::candle::CandleData;

pub struct InteractionState;

impl Default for InteractionState {
    fn default() -> Self {
        Self
    }
}

impl InteractionState {
    /// `interact_rect`: full area that accepts input (chart + volume)
    /// `chart_rect`: just the chart area (for X-pixel → candle-index math)
    pub fn handle_input(
        &mut self,
        ui: &egui::Ui,
        interact_rect: Rect,
        chart_rect: Rect,
        camera: &mut Camera,
        data: &CandleData,
    ) {
        let response = ui.interact(interact_rect, ui.id().with("chart_interaction"), egui::Sense::click_and_drag());
        let in_area = interact_rect.contains(ui.input(|i| i.pointer.latest_pos().unwrap_or(Pos2::ZERO)));

        // Double-click to reset view (fit all data)
        if response.double_clicked() {
            camera.fit_to_data(data);
        }

        // Click + drag to pan (X and Y)
        if response.dragged() {
            let delta = response.drag_delta();

            // Pan X (time)
            let dx_candles = delta.x as f64 / camera.x_scale;
            camera.x_offset -= dx_candles;
            let max_offset = (data.len() as f64 - 1.0).max(0.0);
            camera.x_offset = camera.x_offset.clamp(0.0, max_offset);

            // Pan Y (price) — drag up = see higher prices, drag down = see lower
            if delta.y != 0.0 {
                let dy_price = delta.y as f64 / camera.y_scale;
                camera.y_offset += dy_price;
                camera.auto_scale_y = false;
            }
        }

        // Trackpad / scroll: X = pan, Y = zoom
        let scroll = ui.input(|i| i.smooth_scroll_delta);
        if in_area && (scroll.x != 0.0 || scroll.y != 0.0) {
            if scroll.x != 0.0 {
                let dx_candles = scroll.x as f64 * 0.4 / camera.x_scale;
                camera.x_offset -= dx_candles;
                let max_offset = (data.len() as f64 - 1.0).max(0.0);
                camera.x_offset = camera.x_offset.clamp(0.0, max_offset);
            }

            if scroll.y != 0.0 {
                let zoom_factor = 1.0 + (scroll.y as f64 * 0.002);

                if let Some(cursor_pos) = ui.input(|i| i.pointer.latest_pos()) {
                    let cursor_x_pixel = cursor_pos.x - chart_rect.left();
                    let cursor_index = camera.x_offset + cursor_x_pixel as f64 / camera.x_scale;

                    camera.x_scale *= zoom_factor;
                    camera.x_scale = camera.x_scale.clamp(4.0, 80.0);

                    camera.x_offset = cursor_index - cursor_x_pixel as f64 / camera.x_scale;
                    let max_offset = (data.len() as f64 - 1.0).max(0.0);
                    camera.x_offset = camera.x_offset.clamp(0.0, max_offset);
                }
            }

            camera.auto_scale_y(data);
        }

        // Pinch-to-zoom
        let zoom_delta = ui.input(|i| i.zoom_delta());
        if in_area && zoom_delta != 1.0 {
            if let Some(cursor_pos) = ui.input(|i| i.pointer.latest_pos()) {
                let cursor_x_pixel = cursor_pos.x - chart_rect.left();
                let cursor_index = camera.x_offset + cursor_x_pixel as f64 / camera.x_scale;

                camera.x_scale *= zoom_delta as f64;
                camera.x_scale = camera.x_scale.clamp(4.0, 80.0);

                camera.x_offset = cursor_index - cursor_x_pixel as f64 / camera.x_scale;
                let max_offset = (data.len() as f64 - 1.0).max(0.0);
                camera.x_offset = camera.x_offset.clamp(0.0, max_offset);

                camera.auto_scale_y(data);
            }
        }
    }
}
