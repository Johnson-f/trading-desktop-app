use egui::{Color32, Pos2, Rect, Stroke, Vec2};

const DIVIDER_COLOR: Color32 = Color32::from_rgb(40, 40, 46);
const DIVIDER_HOVER: Color32 = Color32::from_rgb(99, 102, 241);
const DIVIDER_HEIGHT: f32 = 4.0;

const MIN_PANE_RATIO: f32 = 0.10;
const MAX_PANE_RATIO: f32 = 0.50;

/// A resizable sub-pane that sits below the main chart.
/// Reusable for volume, ATR, RSI, MACD, or any indicator pane.
pub struct SubPane {
    pub height_ratio: f32,
    pub dragging_divider: bool,
}

impl SubPane {
    pub fn new(height_ratio: f32) -> Self {
        Self {
            height_ratio,
            dragging_divider: false,
        }
    }

    /// Split the total rect into (main_rect, divider_rect, sub_pane_rect)
    pub fn split_rect(&self, total_rect: Rect) -> (Rect, Rect, Rect) {
        let total_h = total_rect.height();
        let pane_h = total_h * self.height_ratio;
        let divider_y = total_rect.bottom() - pane_h - DIVIDER_HEIGHT;

        let main_rect = Rect::from_min_max(
            total_rect.min,
            Pos2::new(total_rect.right(), divider_y),
        );

        let divider_rect = Rect::from_min_size(
            Pos2::new(total_rect.left(), divider_y),
            Vec2::new(total_rect.width(), DIVIDER_HEIGHT),
        );

        let pane_rect = Rect::from_min_max(
            Pos2::new(total_rect.left(), divider_y + DIVIDER_HEIGHT),
            total_rect.max,
        );

        (main_rect, divider_rect, pane_rect)
    }

    /// Handle divider drag to resize
    pub fn handle_divider_drag(&mut self, ui: &egui::Ui, divider_rect: Rect, total_rect: Rect, id_salt: &str) {
        let response = ui.interact(divider_rect, ui.id().with(id_salt), egui::Sense::click_and_drag());

        if response.hovered() || response.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        }

        if response.dragged() {
            self.dragging_divider = true;
            let delta = response.drag_delta().y;
            let total_h = total_rect.height();
            self.height_ratio -= delta / total_h;
            self.height_ratio = self.height_ratio.clamp(MIN_PANE_RATIO, MAX_PANE_RATIO);
        } else {
            self.dragging_divider = false;
        }
    }

    /// Paint the divider line between panes
    pub fn paint_divider(&self, ui: &egui::Ui, divider_rect: Rect) {
        let color = if self.dragging_divider { DIVIDER_HOVER } else { DIVIDER_COLOR };
        let center_y = divider_rect.center().y;
        ui.painter().line_segment(
            [
                Pos2::new(divider_rect.left(), center_y),
                Pos2::new(divider_rect.right(), center_y),
            ],
            Stroke::new(1.0, color),
        );
    }
}
