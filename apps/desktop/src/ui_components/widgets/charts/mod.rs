mod camera;
mod candle;
mod controls;
mod crosshair;
pub mod drawings;
mod gaps;
mod grid;
mod indicators;
mod interaction;
mod pane;
mod range_markers;
mod renderer;
mod util;

use egui::mutex::Mutex;
use std::sync::Arc;

use camera::Camera;
pub use candle::{CandleData, JsonCandle, Timeframe};
use controls::{ChartToolbar, DrawingSettingsModal, IndicatorBarEvent, SettingsModal};
use drawings::{
    DrawingsManager, SelectionInput, ToolbarEvent, paint_handles, selection_step, show_toolbar,
    toolbar_anchor_rect,
};
use indicators::{self as ind, IndicatorManager, ParamValues};
use interaction::InteractionState;
use pane::SubPaneStack;
use renderer::ChartCallback;

#[derive(Clone, Copy, PartialEq)]
enum LegendAction {
    None,
    OpenSettings,
    Remove,
}

pub struct ChartWidget {
    /// Source of truth — the raw (daily) candles as loaded. Never mutated.
    raw_data: Arc<CandleData>,
    /// Raw candles aggregated to the user-selected timeframe. The bars the
    /// renderer/indicators/grid/crosshair see are always equal to this —
    /// there is no additional count-bucketing on top. Coarser views come
    /// from the user picking a coarser timeframe, not from zoom.
    data: Arc<CandleData>,
    /// Currently selected timeframe (user-driven).
    timeframe: Timeframe,
    camera: Arc<Mutex<Camera>>,
    interaction: InteractionState,
    initialized: Arc<Mutex<bool>>,
    sub_stack: SubPaneStack,
    toolbar: ChartToolbar,
    manager: IndicatorManager,
    settings_modal: SettingsModal,
    drawings: DrawingsManager,
    drawing_settings_modal: DrawingSettingsModal,
    notification: Option<(String, std::time::Instant)>,
}

impl ChartWidget {
    pub fn new(data: CandleData) -> Self {
        let mut camera = Camera::default();
        let data = Arc::new(data);
        camera.fit_to_data(&data);

        let mut manager = IndicatorManager::default();
        if let Some(def) = ind::get("volume") {
            manager.add(def.id, def.params.defaults());
        }

        Self {
            raw_data: data.clone(),
            timeframe: Timeframe::Daily,
            data,
            camera: Arc::new(Mutex::new(camera)),
            interaction: InteractionState::default(),
            initialized: Arc::new(Mutex::new(false)),
            sub_stack: SubPaneStack::default(),
            toolbar: ChartToolbar::default(),
            manager,
            settings_modal: SettingsModal::default(),
            drawings: DrawingsManager::default(),
            drawing_settings_modal: DrawingSettingsModal::default(),
            notification: None,
        }
    }

    /// Switch the base timeframe (Daily / Weekly / Monthly). Rebuilds the
    /// active data from `raw_data` and re-fits the camera so the newly-
    /// aggregated range shows cleanly.
    fn set_timeframe(&mut self, timeframe: Timeframe) {
        if timeframe == self.timeframe {
            return;
        }
        self.timeframe = timeframe;
        self.data = Arc::new(self.raw_data.aggregated(timeframe));
        // Stale VMA / EMA / RSI caches would be wrong length for the new
        // series — force a recompute.
        self.manager.invalidate_cache();
        // Re-anchor committed drawings to the new (aggregated) index space
        // using each point's stored date.
        self.drawings.remap_to_data(&self.data);
        // Reset zoom/pan to fit the aggregated series — otherwise the prior
        // x_offset/x_scale (sized for daily candles) lands mid-data at a
        // wildly-wrong position after aggregation.
        let mut camera = self.camera.lock();
        camera.x_scale = 6.0;
        camera.fit_to_data(&self.data);
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        self.dispatch_toolbar(ui);

        let (total_rect, chart_rect, pane_slots) = self.layout_rects(ui);
        self.update_camera_viewport(chart_rect);

        let modal_open = self.toolbar.indicator_modal.open
            || self.settings_modal.open
            || self.drawing_settings_modal.open;
        let drawing_tool_armed = self.toolbar.active_drawing.is_some();

        let drawing_drag_active = !matches!(self.drawings.drag, drawings::SelectionDrag::None);

        if !modal_open && !drawing_drag_active {
            self.handle_chart_interaction(ui, total_rect, chart_rect);
        }

        let full_rect = full_chart_rect(chart_rect, &pane_slots);

        if !modal_open && drawing_tool_armed {
            self.handle_drawing_input(ui, chart_rect);
        } else if !drawing_tool_armed {
            self.drawings.cancel_draft();
            if !modal_open {
                self.handle_drawing_selection(ui, chart_rect, full_rect);
            }
        }

        self.manager.ensure_computed(&self.data);
        let cursor_idx = self.compute_cursor_idx(ui, chart_rect, &pane_slots);

        self.paint_wgpu_candles(ui, chart_rect);
        self.paint_grid(ui, chart_rect, modal_open);
        {
            let camera = self.camera.lock();
            gaps::paint(ui, chart_rect, &camera, &self.data);
            range_markers::paint(ui, chart_rect, &camera, &self.data);
        }

        let mut pending = self.paint_main_overlays(ui, chart_rect, cursor_idx);
        pending.extend(self.paint_sub_panes(
            ui,
            total_rect,
            chart_rect,
            &pane_slots,
            cursor_idx,
            modal_open,
        ));
        self.apply_legend_actions(pending);

        self.settings_modal.show(ui.ctx(), &mut self.manager);
        self.paint_drawings(ui, chart_rect, full_rect);
        self.paint_drawing_selection(ui, chart_rect, full_rect);
        self.drawing_settings_modal
            .show(ui.ctx(), &mut self.drawings, &self.data);
        self.paint_crosshair(ui, chart_rect, full_rect);
        
        // Render notification toast
        self.paint_notification(ui, total_rect);
    }

    fn dispatch_toolbar(&mut self, ui: &mut egui::Ui) {
        if let Some(ev) = self.toolbar.show(ui, &mut self.manager) {
            if let IndicatorBarEvent::Remove(id) = ev {
                self.manager.remove(id);
            }
        }
        // Pick up user-driven timeframe changes from the toolbar.
        if self.toolbar.timeframe != self.timeframe {
            self.set_timeframe(self.toolbar.timeframe);
        }
    }

    fn layout_rects(
        &mut self,
        ui: &mut egui::Ui,
    ) -> (egui::Rect, egui::Rect, Vec<(egui::Rect, egui::Rect)>) {
        let available = ui.available_size();
        let (total_rect, _response) = ui.allocate_exact_size(available, egui::Sense::hover());
        let n_panes = self.manager.sub_pane_count();
        self.sub_stack.sync_len(n_panes);
        let (chart_rect, pane_slots) = self.sub_stack.split(total_rect, n_panes);
        (total_rect, chart_rect, pane_slots)
    }

    fn update_camera_viewport(&self, chart_rect: egui::Rect) {
        let mut camera = self.camera.lock();
        camera.viewport = chart_rect.size();
    }

    fn handle_chart_interaction(
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

    fn compute_cursor_idx(
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

    fn paint_wgpu_candles(&self, ui: &egui::Ui, chart_rect: egui::Rect) {
        let target_format = egui_wgpu::preferred_framebuffer_format(&[
            wgpu::TextureFormat::Bgra8Unorm,
            wgpu::TextureFormat::Rgba8Unorm,
        ])
        .unwrap_or(wgpu::TextureFormat::Bgra8Unorm);
        let callback = ChartCallback {
            camera: self.camera.clone(),
            data: self.data.clone(),
            initialized: self.initialized.clone(),
            target_format,
        };
        ui.painter().add(egui_wgpu::Callback::new_paint_callback(
            chart_rect, callback,
        ));
    }

    fn paint_grid(&self, ui: &mut egui::Ui, chart_rect: egui::Rect, modal_open: bool) {
        if !modal_open {
            let mut camera = self.camera.lock();
            grid::handle_price_axis_drag(ui, chart_rect, &mut camera);
        }
        let mut camera = self.camera.lock();
        grid::paint_price_grid(ui, chart_rect, &camera, &self.data);
        grid::paint_time_grid(ui, chart_rect, &camera, &self.data);
        if !modal_open {
            grid::paint_auto_button(ui, chart_rect, &mut camera);
        }
    }

    fn paint_main_overlays(
        &self,
        ui: &mut egui::Ui,
        chart_rect: egui::Rect,
        cursor_idx: Option<usize>,
    ) -> Vec<(u64, LegendAction, ParamValues)> {
        {
            let camera = self.camera.lock();
            let painter = ui.painter_at(chart_rect);
            for (active, computed) in self.manager.main_overlays() {
                active.indicator().draw_main(
                    &painter,
                    chart_rect,
                    &camera,
                    &self.data,
                    computed,
                    &active.params,
                );
            }
        }

        // OHLC header row, painted above the indicator legend.
        paint_ohlc_row(
            ui,
            chart_rect,
            &self.data,
            cursor_idx,
            egui::Pos2::new(chart_rect.left() + 8.0, chart_rect.top() + 8.0),
        );

        // Snapshot legend items grouped by def_id. Release the manager borrow
        // before `draw_legend_row` takes &mut ui.
        let mut groups: Vec<FamilyGroup> = Vec::new();
        for (active, computed) in self.manager.main_overlays() {
            let entries =
                active
                    .indicator()
                    .legend(&self.data, computed, &active.params, cursor_idx);
            let member = FamilyMember {
                instance_id: active.instance_id,
                entries,
                params: active.params.clone(),
            };
            if let Some(g) = groups.iter_mut().find(|g| g.def_id == active.def_id) {
                g.members.push(member);
            } else {
                let family_name = ind::get(active.def_id)
                    .map(|d| d.short_name)
                    .unwrap_or("")
                    .to_string();
                groups.push(FamilyGroup {
                    def_id: active.def_id,
                    family_name,
                    members: vec![member],
                });
            }
        }

        // Sort each family's members by the `period` param (ascending) so
        // `EMA(10)` always renders before `EMA(20)`, regardless of the order
        // the user added them. Instances without a period param keep their
        // insertion order (stable sort).
        for group in &mut groups {
            group.members.sort_by_key(|m| period_sort_key(&m.params));
        }

        let mut actions = Vec::new();
        for (i, group) in groups.into_iter().enumerate() {
            // Flatten every member's entries into a single row. Family name
            // prefix is drawn only when there is more than one member.
            let display_name = if group.members.len() > 1 {
                group.family_name.as_str()
            } else {
                ""
            };
            let combined: Vec<indicators::LegendEntry> = group
                .members
                .iter()
                .flat_map(|m| m.entries.iter().cloned())
                .collect();
            let first_id = group.members[0].instance_id;
            let first_params = group.members[0].params.clone();

            let action = draw_legend_row(
                ui,
                chart_rect,
                egui::Pos2::new(
                    chart_rect.left() + 8.0,
                    chart_rect.top() + 28.0 + (i as f32 * 14.0),
                ),
                display_name,
                &combined,
                &format!("main-{}", group.def_id),
            );
            match action {
                LegendAction::None => {}
                LegendAction::OpenSettings => {
                    // Target the first instance — settings modal edits one
                    // instance at a time.
                    actions.push((first_id, action, first_params));
                }
                LegendAction::Remove => {
                    // Fan out: emit one Remove per member so the caller's
                    // per-id apply loop clears the whole family.
                    for m in &group.members {
                        actions.push((m.instance_id, LegendAction::Remove, m.params.clone()));
                    }
                }
            }
        }
        actions
    }

    fn paint_sub_panes(
        &mut self,
        ui: &mut egui::Ui,
        total_rect: egui::Rect,
        chart_rect: egui::Rect,
        pane_slots: &[(egui::Rect, egui::Rect)],
        cursor_idx: Option<usize>,
        modal_open: bool,
    ) -> Vec<(u64, LegendAction, ParamValues)> {
        let x_mapper = self.build_x_mapper(chart_rect);
        let items = self.draw_sub_pane_indicators(ui, pane_slots, x_mapper.as_ref(), cursor_idx);
        self.handle_and_paint_dividers(ui, total_rect, pane_slots, modal_open);

        let mut actions = Vec::new();
        for (i, item) in items.into_iter().enumerate() {
            let Some((_, pane_rect)) = pane_slots.get(i) else {
                continue;
            };
            let action = draw_legend_row(
                ui,
                *pane_rect,
                egui::Pos2::new(pane_rect.left() + 6.0, pane_rect.top() + 4.0),
                &item.name,
                &item.entries,
                &format!("pane-{}", item.instance_id),
            );
            if action != LegendAction::None {
                actions.push((item.instance_id, action, item.params));
            }
        }
        actions
    }

    fn build_x_mapper(&self, chart_rect: egui::Rect) -> Box<dyn Fn(f32) -> f32> {
        let camera = self.camera.lock();
        let x_offset = camera.x_offset;
        let x_scale = camera.x_scale;
        let left = chart_rect.left();
        Box::new(move |idx: f32| -> f32 { left + ((idx as f64 - x_offset) * x_scale) as f32 })
    }

    fn draw_sub_pane_indicators(
        &self,
        ui: &mut egui::Ui,
        pane_slots: &[(egui::Rect, egui::Rect)],
        x_mapper: &dyn Fn(f32) -> f32,
        cursor_idx: Option<usize>,
    ) -> Vec<LegendItem> {
        let sub_list: Vec<(&ind::ActiveIndicator, &ind::ComputedSeries)> =
            self.manager.sub_panes().collect();
        let mut items: Vec<LegendItem> = Vec::with_capacity(sub_list.len());
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
            items.push(LegendItem {
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

    fn handle_and_paint_dividers(
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

    fn apply_legend_actions(&mut self, actions: Vec<(u64, LegendAction, ParamValues)>) {
        for (id, action, params) in actions {
            match action {
                LegendAction::OpenSettings => self.settings_modal.open_for(id, params),
                LegendAction::Remove => self.manager.remove(id),
                LegendAction::None => {}
            }
        }
    }

    fn handle_drawing_input(&mut self, ui: &egui::Ui, chart_rect: egui::Rect) {
        let Some(def_id) = self.toolbar.active_drawing else {
            return;
        };
        let Some(tool) = self.drawings.tool_for(def_id) else {
            return;
        };
        let camera = self.camera.lock();
        let result = tool.handle_input(ui, chart_rect, &camera, &mut self.drawings.draft);
        drop(camera);
        if let drawings::InputResult::Commit(points) = result {
            let point_dates: Vec<String> = points
                .iter()
                .map(|p| {
                    self.raw_data
                        .dates
                        .get(p.index as usize)
                        .cloned()
                        .unwrap_or_default()
                })
                .collect();
            self.drawings.commit(tool.id(), points, point_dates);
            self.toolbar.active_drawing = None;
        }
    }

    fn paint_drawings(&self, ui: &egui::Ui, chart_rect: egui::Rect, full_rect: egui::Rect) {
        let camera = self.camera.lock();
        // Wide clip so tools that span sub-panes (vertical lines) can reach
        // past chart_rect; other tools narrow the clip back themselves.
        let painter = ui.painter_at(full_rect);
        for drawing in &self.drawings.committed {
            let Some(tool) = self.drawings.tool_for(drawing.def_id) else {
                continue;
            };
            tool.render(
                &painter,
                chart_rect,
                full_rect,
                &camera,
                &drawing.points,
                &drawing.style,
            );
        }
        if let Some(draft) = self.drawings.draft.as_ref() {
            if let Some(tool) = self.drawings.tool_for(draft.def_id) {
                tool.render_preview(&painter, chart_rect, full_rect, &camera, &draft.points);
            }
        }
    }

    fn paint_crosshair(&self, ui: &egui::Ui, chart_rect: egui::Rect, full_rect: egui::Rect) {
        let camera = self.camera.lock();
        crosshair::paint_crosshair(ui, chart_rect, full_rect, &camera, &self.data);
    }

    fn handle_drawing_selection(
        &mut self,
        ui: &egui::Ui,
        chart_rect: egui::Rect,
        full_rect: egui::Rect,
    ) {
        if !chart_rect.intersects(full_rect) {
            return;
        }

        let (pointer_pos, pointer_down, pointer_released, modifiers, key_pressed) = ui.input(|i| {
            (
                i.pointer.latest_pos(),
                i.pointer.primary_pressed(),
                i.pointer.primary_released(),
                i.modifiers,
                first_consumed_key(&i.events),
            )
        });

        let key_pressed = if ui.ctx().wants_keyboard_input() {
            None
        } else {
            key_pressed
        };

        let (dx, dy) = {
            let camera = self.camera.lock();
            let x_range = camera.viewport.x as f64 / camera.x_scale;
            let y_range = camera.viewport.y as f64 / camera.y_scale;
            ((x_range * 0.05) as f32, (y_range * 0.05) as f32)
        };

        let toolbar_rect = self.cached_toolbar_rect(chart_rect, full_rect);

        let input = SelectionInput {
            pointer_pos,
            pointer_down,
            pointer_released,
            modifiers,
            key_pressed,
            chart_rect,
            full_rect,
            toolbar_rect,
            clone_offset: (dx, dy),
        };

        let camera = self.camera.lock();
        selection_step(&mut self.drawings, &camera, &input);
    }

    /// Compute the anchor rect the toolbar will occupy this frame, so the
    /// selection state machine can treat the grip area as a hit target.
    fn cached_toolbar_rect(
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

    fn paint_drawing_selection(
        &mut self,
        ui: &mut egui::Ui,
        chart_rect: egui::Rect,
        full_rect: egui::Rect,
    ) {
        // 1. Paint the blue endpoint handles for the selected drawing.
        let handles_to_paint: Option<Vec<egui::Pos2>> = {
            let camera = self.camera.lock();
            self.drawings
                .selected_drawing()
                .filter(|d| !d.locked)
                .and_then(|d| {
                    self.drawings
                        .tool_for(d.def_id)
                        .map(|t| t.handles(chart_rect, full_rect, &camera, &d.points))
                })
        };
        if let Some(handles) = handles_to_paint {
            let painter = ui.painter_at(full_rect);
            paint_handles(&painter, &handles);
        }

        // 2. Paint the floating toolbar.
        let Some(drawing_id) = self.drawings.selected else {
            return;
        };
        let Some(idx) = self
            .drawings
            .committed
            .iter()
            .position(|d| d.id == drawing_id)
        else {
            return;
        };
        let def_id = self.drawings.committed[idx].def_id;
        let Some(tool) = self.drawings.tool_for(def_id) else {
            return;
        };

        // Split-borrow `self.drawings` so show_toolbar can hold &mut drawing,
        // &mut drag, and &mut toolbar_offset simultaneously. The destructure
        // gives three disjoint &mut references, which is safe.
        let event = {
            let camera = self.camera.lock();
            let DrawingsManager {
                committed,
                drag,
                toolbar_offset,
                ..
            } = &mut self.drawings;
            let drawing = &mut committed[idx];
            let (_rect, event) = show_toolbar(
                ui,
                tool.as_ref(),
                chart_rect,
                full_rect,
                &camera,
                drawing,
                toolbar_offset,
                drag,
            );
            event
        };

        // 3. Map toolbar events to manager mutations / modal opens.
        match event {
            ToolbarEvent::None => {}
            ToolbarEvent::Clone => {
                let (dx, dy) = {
                    let camera = self.camera.lock();
                    let x_range = camera.viewport.x as f64 / camera.x_scale;
                    let y_range = camera.viewport.y as f64 / camera.y_scale;
                    ((x_range * 0.05) as f32, (y_range * 0.05) as f32)
                };
                self.drawings.clone_selected(dx, dy);
            }
            ToolbarEvent::Delete => {
                self.drawings.remove_selected();
            }
            ToolbarEvent::OpenSettings => {
                if let Some(d) = self.drawings.selected_drawing() {
                    self.drawing_settings_modal.open_for(d);
                }
            }
            ToolbarEvent::SetAsDefault => {
                if let Some(d) = self.drawings.selected_drawing() {
                    if let Err(e) = drawings::set_user_default_style(d.style) {
                        eprintln!("Failed to save default style: {}", e);
                        self.notification = Some((
                            format!("✗ Failed to save: {}", e),
                            std::time::Instant::now(),
                        ));
                    } else {
                        self.notification = Some((
                            "✓ Saved as default style".to_string(),
                            std::time::Instant::now(),
                        ));
                    }
                }
            }
        }
    }
    
    fn paint_notification(&mut self, ui: &egui::Ui, rect: egui::Rect) {
        const NOTIFICATION_DURATION_SECS: f32 = 2.0;
        
        if let Some((message, start_time)) = &self.notification {
            let elapsed = start_time.elapsed().as_secs_f32();
            
            if elapsed > NOTIFICATION_DURATION_SECS {
                self.notification = None;
                return;
            }
            
            // Fade out in the last 0.5 seconds
            let alpha = if elapsed > NOTIFICATION_DURATION_SECS - 0.5 {
                ((NOTIFICATION_DURATION_SECS - elapsed) / 0.5).clamp(0.0, 1.0)
            } else {
                1.0
            };
            
            let painter = ui.painter_at(rect);
            let center = rect.center();
            let text_pos = egui::Pos2::new(center.x, rect.top() + 60.0);
            
            // Determine color based on message type
            let (bg_color, text_color) = if message.starts_with('✓') {
                (
                    egui::Color32::from_rgba_unmultiplied(78, 205, 196, (180.0 * alpha) as u8),
                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, (255.0 * alpha) as u8),
                )
            } else {
                (
                    egui::Color32::from_rgba_unmultiplied(255, 107, 107, (180.0 * alpha) as u8),
                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, (255.0 * alpha) as u8),
                )
            };
            
            // Measure text size
            let font_id = egui::FontId::proportional(14.0);
            let galley = painter.layout_no_wrap(message.clone(), font_id.clone(), text_color);
            
            // Draw background
            let padding = egui::Vec2::new(12.0, 8.0);
            let bg_rect = egui::Rect::from_center_size(
                text_pos,
                galley.size() + padding * 2.0,
            );
            painter.rect_filled(bg_rect, egui::Rounding::same(6), bg_color);
            
            // Draw text
            painter.galley(
                egui::Pos2::new(text_pos.x - galley.size().x / 2.0, text_pos.y - galley.size().y / 2.0),
                galley,
                text_color,
            );
            
            // Request repaint for animation
            ui.ctx().request_repaint();
        }
    }
}

fn first_consumed_key(events: &[egui::Event]) -> Option<egui::Key> {
    for e in events {
        if let egui::Event::Key {
            key, pressed: true, ..
        } = e
        {
            return Some(*key);
        }
    }
    None
}

fn full_chart_rect(chart_rect: egui::Rect, pane_slots: &[(egui::Rect, egui::Rect)]) -> egui::Rect {
    let bottom = pane_slots
        .last()
        .map(|(_, p)| p.bottom())
        .unwrap_or(chart_rect.bottom());
    egui::Rect::from_min_max(chart_rect.min, egui::Pos2::new(chart_rect.right(), bottom))
}

struct LegendItem {
    instance_id: u64,
    name: String,
    entries: Vec<indicators::LegendEntry>,
    params: ParamValues,
}

struct FamilyMember {
    instance_id: u64,
    entries: Vec<indicators::LegendEntry>,
    params: ParamValues,
}

struct FamilyGroup {
    def_id: &'static str,
    family_name: String,
    members: Vec<FamilyMember>,
}

/// Paint the OHLC + change + volume row at `anchor`. Uses the candle under
/// `cursor_idx` when hovering, otherwise the last candle.
fn paint_ohlc_row(
    ui: &egui::Ui,
    clip_rect: egui::Rect,
    data: &CandleData,
    cursor_idx: Option<usize>,
    anchor: egui::Pos2,
) {
    use egui::{Color32, FontId, Pos2};

    if data.is_empty() {
        return;
    }
    let idx = cursor_idx.unwrap_or(data.len() - 1).min(data.len() - 1);
    let c = &data.instances[idx];
    let prev_close = if idx > 0 {
        data.instances[idx - 1].close
    } else {
        c.open
    };
    let change = c.close - prev_close;
    let change_pct = if prev_close.abs() > f32::EPSILON {
        (change / prev_close) * 100.0
    } else {
        0.0
    };

    let painter = ui.painter_at(clip_rect);
    let font = FontId::monospace(12.0);
    let label_color = Color32::from_rgb(140, 140, 150);
    let value_color = Color32::from_rgb(225, 225, 230);
    let up_color = Color32::from_rgb(38, 201, 160);
    let down_color = Color32::from_rgb(255, 107, 107);
    let change_color = if change >= 0.0 { up_color } else { down_color };
    let sign = if change >= 0.0 { "+" } else { "" };

    let parts: Vec<(String, Color32)> = vec![
        ("O".into(), label_color),
        (format!("{:.2}", c.open), value_color),
        ("H".into(), label_color),
        (format!("{:.2}", c.high), value_color),
        ("L".into(), label_color),
        (format!("{:.2}", c.low), value_color),
        ("C".into(), label_color),
        (format!("{:.2}", c.close), value_color),
        (
            format!("{}{:.2} ({}{:.2}%)", sign, change, sign, change_pct),
            change_color,
        ),
        ("Vol".into(), label_color),
        (format_with_commas(c.volume as i64), value_color),
    ];

    let mut x = anchor.x;
    let mut first = true;
    for (text, color) in parts {
        if !first {
            x += 6.0;
        }
        first = false;
        let galley = painter.layout_no_wrap(text, font.clone(), color);
        let size = galley.size();
        painter.galley(Pos2::new(x, anchor.y), galley, color);
        x += size.x;
    }
}

/// Sort key used to order family members (e.g. multiple EMAs) by period
/// ascending. Instances without a numeric `period` param sort to the end —
/// keeping their relative insertion order under a stable sort.
fn period_sort_key(params: &ParamValues) -> i32 {
    match params.0.get("period") {
        Some(zaned_chart_core::ParamValue::Int(v)) => *v,
        _ => i32::MAX,
    }
}

fn format_with_commas(n: i64) -> String {
    let neg = n < 0;
    let digits: Vec<char> = n.unsigned_abs().to_string().chars().collect();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    for (i, d) in digits.iter().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(*d);
    }
    if neg {
        out.insert(0, '-');
    }
    out
}

fn draw_legend_row(
    ui: &mut egui::Ui,
    clip_rect: egui::Rect,
    anchor: egui::Pos2,
    display_name: &str,
    entries: &[indicators::LegendEntry],
    id_salt: &str,
) -> LegendAction {
    use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Vec2};

    let painter = ui.painter_at(clip_rect);
    let text_font = FontId::monospace(10.0);
    let icon_font = FontId::proportional(13.0);
    let name_color = Color32::from_rgb(240, 240, 242);
    let icon_color = Color32::from_rgb(160, 160, 170);
    let icon_hover = Color32::from_rgb(240, 240, 242);
    let pill_bg = Color32::from_rgba_premultiplied(30, 30, 34, 220);

    // Measure text upfront so we can allocate the hover rect before painting.
    let name_galley =
        painter.layout_no_wrap(display_name.to_string(), text_font.clone(), name_color);
    let name_size = name_galley.size();

    let mut entry_galleys: Vec<(std::sync::Arc<egui::Galley>, Color32)> =
        Vec::with_capacity(entries.len());
    let mut entries_width: f32 = 0.0;
    for entry in entries {
        let color = util::egui_color(entry.color);
        let g = painter.layout_no_wrap(entry.label.clone(), text_font.clone(), color);
        entries_width += 8.0 + g.size().x;
        entry_galleys.push((g, color));
    }

    let icon_slot: f32 = 20.0;
    let row_height: f32 = 18.0_f32.max(name_size.y);
    let full_width = name_size.x + entries_width;
    let hover_width = name_size.x + 6.0 + icon_slot + 2.0 + icon_slot + 4.0;
    let hit_width = full_width.max(hover_width);

    let hit_rect = Rect::from_min_size(
        anchor - Vec2::new(4.0, 2.0),
        Vec2::new(hit_width + 8.0, row_height + 4.0),
    );
    // Pure geometric pointer test. Don't use ui.interact(...).hovered() here —
    // registering the row as an interactable fights with the icon click rects
    // below (they'd steal focus and toggle .hovered() off every other frame,
    // causing the legend to flicker and swallow clicks).
    let hovered = ui
        .input(|i| i.pointer.latest_pos())
        .map(|p| hit_rect.contains(p))
        .unwrap_or(false);

    if !hovered {
        painter.galley(anchor, name_galley, name_color);
        let mut x = anchor.x + name_size.x + 8.0;
        for (g, fallback) in entry_galleys {
            let size = g.size();
            painter.galley(Pos2::new(x, anchor.y), g, fallback);
            x += size.x + 8.0;
        }
        return LegendAction::None;
    }

    let pill_rect = Rect::from_min_size(
        anchor - Vec2::new(6.0, 3.0),
        Vec2::new(hover_width + 4.0, row_height + 6.0),
    );
    painter.rect_filled(pill_rect, 4.0, pill_bg);
    painter.galley(anchor, name_galley, name_color);

    let mut action = LegendAction::None;

    let gear_x = anchor.x + name_size.x + 6.0;
    let gear_rect = Rect::from_min_size(
        Pos2::new(gear_x, anchor.y - 2.0),
        Vec2::new(icon_slot, icon_slot),
    );
    let gear_resp = ui.interact(
        gear_rect,
        ui.id().with("legend_gear").with(id_salt),
        Sense::click(),
    );
    let gear_color = if gear_resp.hovered() {
        icon_hover
    } else {
        icon_color
    };
    if gear_resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    painter.text(
        gear_rect.center(),
        Align2::CENTER_CENTER,
        egui_phosphor::regular::GEAR,
        icon_font.clone(),
        gear_color,
    );
    if gear_resp.clicked() {
        action = LegendAction::OpenSettings;
    }

    let x_x = gear_x + icon_slot + 2.0;
    let x_rect = Rect::from_min_size(
        Pos2::new(x_x, anchor.y - 2.0),
        Vec2::new(icon_slot, icon_slot),
    );
    let x_resp = ui.interact(
        x_rect,
        ui.id().with("legend_x").with(id_salt),
        Sense::click(),
    );
    let x_color = if x_resp.hovered() {
        icon_hover
    } else {
        icon_color
    };
    if x_resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    painter.text(
        x_rect.center(),
        Align2::CENTER_CENTER,
        egui_phosphor::regular::X,
        icon_font.clone(),
        x_color,
    );
    if x_resp.clicked() {
        action = LegendAction::Remove;
    }

    action
}
