use egui::{Color32, Pos2, Rect, Stroke, Vec2};

const DIVIDER_COLOR: Color32 = Color32::from_rgb(40, 40, 46);
const DIVIDER_HOVER: Color32 = Color32::from_rgb(99, 102, 241);
const DIVIDER_HEIGHT: f32 = 4.0;
const MIN_MAIN_RATIO: f32 = 0.40;
const MAX_TOTAL_SUB_RATIO: f32 = 0.60;
const MIN_PANE_PX: f32 = 40.0;

pub struct SubPaneStack {
    pub total_ratio: f32,
    pub weights: Vec<f32>,
    pub dragging_main_divider: bool,
    pub dragging_pane_divider: Option<usize>,
}

impl Default for SubPaneStack {
    fn default() -> Self {
        Self {
            total_ratio: 0.20,
            weights: Vec::new(),
            dragging_main_divider: false,
            dragging_pane_divider: None,
        }
    }
}

impl SubPaneStack {
    pub fn sync_len(&mut self, n: usize) {
        while self.weights.len() < n {
            self.weights.push(1.0);
        }
        if self.weights.len() > n {
            self.weights.truncate(n);
        }
    }

    pub fn split(&self, total_rect: Rect, n_panes: usize) -> (Rect, Vec<(Rect, Rect)>) {
        if n_panes == 0 {
            return (total_rect, Vec::new());
        }
        let total_h = total_rect.height();
        let sub_area_h = (total_h * self.total_ratio).clamp(0.0, total_h * MAX_TOTAL_SUB_RATIO);
        let main_h = (total_h - sub_area_h - DIVIDER_HEIGHT * n_panes as f32)
            .max(total_h * MIN_MAIN_RATIO);
        let actual_sub_h = (total_h - main_h - DIVIDER_HEIGHT * n_panes as f32).max(0.0);

        let sum: f32 = self.weights.iter().take(n_panes).sum::<f32>().max(f32::EPSILON);

        let main_rect = Rect::from_min_max(
            total_rect.min,
            Pos2::new(total_rect.right(), total_rect.top() + main_h),
        );

        let mut out = Vec::with_capacity(n_panes);
        let mut y = total_rect.top() + main_h;
        for i in 0..n_panes {
            let divider = Rect::from_min_size(
                Pos2::new(total_rect.left(), y),
                Vec2::new(total_rect.width(), DIVIDER_HEIGHT),
            );
            y += DIVIDER_HEIGHT;
            let pane_h = (actual_sub_h * self.weights[i] / sum).max(MIN_PANE_PX.min(actual_sub_h));
            let pane = Rect::from_min_size(
                Pos2::new(total_rect.left(), y),
                Vec2::new(total_rect.width(), pane_h),
            );
            y += pane_h;
            out.push((divider, pane));
        }
        (main_rect, out)
    }

    pub fn handle_main_divider_drag(
        &mut self,
        ui: &egui::Ui,
        divider_rect: Rect,
        total_rect: Rect,
        id_salt: &str,
    ) {
        let response = ui.interact(
            divider_rect,
            ui.id().with(id_salt).with("main"),
            egui::Sense::click_and_drag(),
        );
        if response.hovered() || response.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        }
        if response.dragged() {
            self.dragging_main_divider = true;
            let delta = response.drag_delta().y;
            let total_h = total_rect.height().max(1.0);
            self.total_ratio = (self.total_ratio - delta / total_h).clamp(0.0, MAX_TOTAL_SUB_RATIO);
        } else {
            self.dragging_main_divider = false;
        }
    }

    pub fn handle_pane_divider_drag(
        &mut self,
        ui: &egui::Ui,
        pane_idx: usize,
        divider_rect: Rect,
        id_salt: &str,
    ) {
        if pane_idx == 0 {
            return;
        }
        let response = ui.interact(
            divider_rect,
            ui.id().with(id_salt).with("pane").with(pane_idx),
            egui::Sense::click_and_drag(),
        );
        if response.hovered() || response.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        }
        if response.dragged() {
            self.dragging_pane_divider = Some(pane_idx);
            let delta = response.drag_delta().y;
            let transfer = delta * 0.05;
            let a = (self.weights[pane_idx - 1] + transfer).max(0.1);
            let b = (self.weights[pane_idx] - transfer).max(0.1);
            self.weights[pane_idx - 1] = a;
            self.weights[pane_idx] = b;
        } else {
            self.dragging_pane_divider = None;
        }
    }

    pub fn paint_divider(&self, ui: &egui::Ui, divider_rect: Rect, hot: bool) {
        let color = if hot { DIVIDER_HOVER } else { DIVIDER_COLOR };
        let center_y = divider_rect.center().y;
        ui.painter().line_segment(
            [Pos2::new(divider_rect.left(), center_y), Pos2::new(divider_rect.right(), center_y)],
            Stroke::new(1.0, color),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(w: f32, h: f32) -> Rect {
        Rect::from_min_size(Pos2::ZERO, Vec2::new(w, h))
    }

    #[test]
    fn zero_panes_gives_full_main() {
        let s = SubPaneStack::default();
        let (main, dividers) = s.split(rect(800.0, 600.0), 0);
        assert_eq!(main, rect(800.0, 600.0));
        assert!(dividers.is_empty());
    }

    #[test]
    fn one_pane_splits_by_total_ratio() {
        let mut s = SubPaneStack::default();
        s.total_ratio = 0.2;
        s.sync_len(1);
        let (main, panes) = s.split(rect(800.0, 600.0), 1);
        assert_eq!(panes.len(), 1);
        assert!((main.height() - 476.0).abs() < 1.0);
        let (_, pane) = panes[0];
        assert!(pane.height() > 0.0);
    }

    #[test]
    fn equal_weights_give_equal_pane_heights() {
        let mut s = SubPaneStack::default();
        s.total_ratio = 0.4;
        s.sync_len(3);
        let (_, panes) = s.split(rect(800.0, 600.0), 3);
        let heights: Vec<_> = panes.iter().map(|(_, p)| p.height()).collect();
        assert!((heights[0] - heights[1]).abs() < 0.5);
        assert!((heights[1] - heights[2]).abs() < 0.5);
    }

    #[test]
    fn sync_len_extends_and_truncates() {
        let mut s = SubPaneStack::default();
        s.sync_len(3);
        assert_eq!(s.weights, vec![1.0, 1.0, 1.0]);
        s.sync_len(1);
        assert_eq!(s.weights, vec![1.0]);
    }
}
