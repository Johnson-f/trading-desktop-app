use super::ChartWidget;
use super::drawings::toolbar_anchor_rect;
use super::indicators as ind;
use super::legend;

impl ChartWidget {
    pub(super) fn layout_rects(
        &mut self,
        ui: &mut egui::Ui,
    ) -> (
        egui::Rect,
        egui::Rect,
        Vec<(egui::Rect, egui::Rect)>,
        egui::Rect,
    ) {
        const FOOTER_H: f32 = 28.0;
        let available = ui.available_size();
        let (full_rect, _response) = ui.allocate_exact_size(available, egui::Sense::hover());
        // Reserve a footer strip at the bottom for the Interval / Auto controls.
        // Chart panes shrink accordingly; the rest of the painting code only
        // sees the shrunken total_rect.
        let footer_h = FOOTER_H.min(full_rect.height() * 0.5);
        let total_rect = egui::Rect::from_min_max(
            full_rect.min,
            egui::pos2(full_rect.max.x, full_rect.max.y - footer_h),
        );
        let footer_rect = egui::Rect::from_min_max(
            egui::pos2(full_rect.min.x, full_rect.max.y - footer_h),
            full_rect.max,
        );
        let n_panes = self.manager.sub_pane_count();
        self.sub_stack.sync_len(n_panes);
        let (chart_rect, pane_slots) = self.sub_stack.split(total_rect, n_panes);
        (total_rect, chart_rect, pane_slots, footer_rect)
    }

    pub(super) fn update_camera_viewport(&self, chart_rect: egui::Rect) {
        let mut camera = self.camera.lock();
        camera.viewport = chart_rect.size();
    }

    pub(super) fn handle_chart_interaction(
        &mut self,
        ui: &egui::Ui,
        total_rect: egui::Rect,
        chart_rect: egui::Rect,
    ) {
        let mut camera = self.camera.lock();
        self.interaction
            .handle_input(ui, total_rect, chart_rect, &mut camera, &self.data);
        camera.auto_scale_y(&self.data);
    }

    pub(super) fn compute_cursor_idx(
        &self,
        ui: &egui::Ui,
        chart_rect: egui::Rect,
        pane_slots: &[(egui::Rect, egui::Rect)],
    ) -> Option<usize> {
        let camera = self.camera.lock();
        let pos = ui.input(|i| i.pointer.latest_pos())?;
        let over_chart = chart_rect.contains(pos);
        let over_pane = pane_slots.iter().any(|(_, p)| p.contains(pos));
        if !over_chart && !over_pane {
            return None;
        }
        let cursor_x_pixel = pos.x - chart_rect.left();
        let idx_f = camera.x_offset + cursor_x_pixel as f64 / camera.x_scale;
        let idx = idx_f.round() as isize;
        if idx < 0 {
            return None;
        }
        let idx = idx as usize;
        if idx >= self.data.len() {
            None
        } else {
            Some(idx)
        }
    }

    pub(super) fn build_x_mapper(&self, chart_rect: egui::Rect) -> Box<dyn Fn(f32) -> f32> {
        let camera = self.camera.lock();
        let x_offset = camera.x_offset;
        let x_scale = camera.x_scale;
        let left = chart_rect.left();
        Box::new(move |idx: f32| -> f32 { left + ((idx as f64 - x_offset) * x_scale) as f32 })
    }

    pub(super) fn draw_sub_pane_indicators(
        &self,
        ui: &mut egui::Ui,
        pane_slots: &[(egui::Rect, egui::Rect)],
        x_mapper: &dyn Fn(f32) -> f32,
        cursor_idx: Option<usize>,
    ) -> Vec<legend::LegendItem> {
        let sub_list: Vec<(&ind::ActiveIndicator, &ind::ComputedSeries)> =
            self.manager.sub_panes().collect();
        let mut items: Vec<legend::LegendItem> = Vec::with_capacity(sub_list.len());
        for (i, (_divider_rect, pane_rect)) in pane_slots.iter().enumerate() {
            let Some((active, computed)) = sub_list.get(i) else {
                continue;
            };
            let painter = ui.painter_at(*pane_rect);
            active.indicator().draw_pane(
                &painter,
                *pane_rect,
                x_mapper,
                &self.data,
                computed,
                &active.params,
            );
            items.push(legend::LegendItem {
                instance_id: active.instance_id,
                name: active.indicator().display_name(&active.params),
                entries: active.indicator().legend(
                    &self.data,
                    computed,
                    &active.params,
                    cursor_idx,
                ),
                params: active.params.clone(),
            });
        }
        items
    }

    pub(super) fn handle_and_paint_dividers(
        &mut self,
        ui: &mut egui::Ui,
        total_rect: egui::Rect,
        pane_slots: &[(egui::Rect, egui::Rect)],
        modal_open: bool,
    ) {
        for (i, (divider_rect, _pane_rect)) in pane_slots.iter().enumerate() {
            if !modal_open {
                if i == 0 {
                    self.sub_stack.handle_main_divider_drag(
                        ui,
                        *divider_rect,
                        total_rect,
                        "main_divider",
                    );
                } else {
                    self.sub_stack
                        .handle_pane_divider_drag(ui, i, *divider_rect, "pane_divider");
                }
            }
            let hot = (i == 0 && self.sub_stack.dragging_main_divider)
                || (i > 0 && self.sub_stack.dragging_pane_divider == Some(i));
            self.sub_stack.paint_divider(ui, *divider_rect, hot);
        }
    }

    /// Compute the anchor rect the toolbar will occupy this frame, so the
    /// selection state machine can treat the grip area as a hit target.
    pub(super) fn cached_toolbar_rect(
        &self,
        chart_rect: egui::Rect,
        full_rect: egui::Rect,
    ) -> Option<egui::Rect> {
        let drawing = self.drawings.selected_drawing()?;
        let tool = self.drawings.tool_for(drawing.def_id)?;
        let camera = self.camera.lock();
        let bounds = tool.bounds(chart_rect, full_rect, &camera, &drawing.points);
        Some(toolbar_anchor_rect(
            bounds,
            full_rect,
            drawing.locked,
            self.drawings.toolbar_offset,
        ))
    }

    /// Date under the cursor as of the last frame, if the cursor was over the chart.
    pub fn cursor_date(&self) -> Option<String> {
        let idx = self.last_cursor_idx?;
        self.data.date_for_index(idx)
    }

    /// Date range visible in the chart right now: `(start_date, end_date)`.
    /// Returns `None` if the data is empty or the camera state is degenerate.
    pub fn visible_date_range(&self) -> Option<(String, String)> {
        if self.data.is_empty() {
            return None;
        }
        let camera = self.camera.lock();
        let start_idx = (camera.x_offset.max(0.0) as usize).min(self.data.len() - 1);
        let visible_count = (camera.viewport.x as f64 / camera.x_scale).ceil() as usize;
        let end_idx = (start_idx + visible_count).min(self.data.len() - 1);
        drop(camera);
        let start = self.data.date_for_index(start_idx)?;
        let end = self.data.date_for_index(end_idx)?;
        Some((start, end))
    }

    /// Position the camera so the given date range is exactly visible.
    /// Uses the pane's own data, falling back to the nearest available date
    /// when the target dates aren't in this pane's series (different timeframes
    /// have different dates — a Wednesday exists in Daily but not Weekly).
    pub fn apply_camera_for_date_range(&self, start_date: &str, end_date: &str) {
        if self.data.is_empty() {
            return;
        }
        let Some(start_idx) = self.data.nearest_index_for_date(start_date) else {
            return;
        };
        let Some(end_idx) = self.data.nearest_index_for_date(end_date) else {
            return;
        };
        let span = (end_idx as i64 - start_idx as i64).abs().max(1) as f64;
        let mut camera = self.camera.lock();
        if camera.viewport.x <= 0.0 {
            return;
        }
        camera.x_scale = camera.viewport.x as f64 / span;
        camera.x_offset = start_idx as f64;
        camera.auto_scale_y(&self.data);
    }
}

pub(super) fn full_chart_rect(
    chart_rect: egui::Rect,
    pane_slots: &[(egui::Rect, egui::Rect)],
) -> egui::Rect {
    let bottom = pane_slots
        .last()
        .map(|(_, p)| p.bottom())
        .unwrap_or(chart_rect.bottom());
    egui::Rect::from_min_max(chart_rect.min, egui::Pos2::new(chart_rect.right(), bottom))
}
