mod camera;
mod candle;
mod controls;
mod crosshair;
mod drawings;
mod grid;
mod indicators;
mod interaction;
mod pane;
mod renderer;
mod util;

use std::sync::Arc;
use egui::mutex::Mutex;

pub use candle::{CandleData, JsonCandle};
use camera::Camera;
use controls::{ChartToolbar, IndicatorBarEvent, SettingsModal};
use drawings::DrawingsManager;
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
    data: Arc<CandleData>,
    camera: Arc<Mutex<Camera>>,
    interaction: InteractionState,
    initialized: Arc<Mutex<bool>>,
    sub_stack: SubPaneStack,
    toolbar: ChartToolbar,
    manager: IndicatorManager,
    settings_modal: SettingsModal,
    drawings: DrawingsManager,
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
            data,
            camera: Arc::new(Mutex::new(camera)),
            interaction: InteractionState::default(),
            initialized: Arc::new(Mutex::new(false)),
            sub_stack: SubPaneStack::default(),
            toolbar: ChartToolbar::default(),
            manager,
            settings_modal: SettingsModal::default(),
            drawings: DrawingsManager::default(),
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        self.dispatch_toolbar(ui);

        let (total_rect, chart_rect, pane_slots) = self.layout_rects(ui);
        self.update_camera_viewport(chart_rect);

        let modal_open = self.toolbar.indicator_modal.open || self.settings_modal.open;
        let drawing_active = self.toolbar.active_drawing.is_some();

        // Pan/zoom runs even while a drawing tool is armed: a drag on the chart
        // pans the view, a pure click places the next drawing point. The two
        // don't collide because the drawing tool only reacts to clicks and the
        // chart interaction only reacts to drags and scrolls.
        if !modal_open {
            self.handle_chart_interaction(ui, total_rect, chart_rect);
        }

        if !modal_open && drawing_active {
            self.handle_drawing_input(ui, chart_rect);
        } else if !drawing_active {
            // Tool just got deactivated (or was never on) — drop any in-progress
            // preview so a future activation starts from a clean slate.
            self.drawings.cancel_draft();
        }

        self.manager.ensure_computed(&self.data);
        let cursor_idx = self.compute_cursor_idx(ui, chart_rect, &pane_slots);

        self.paint_wgpu_candles(ui, chart_rect);
        self.paint_grid(ui, chart_rect, modal_open);

        let mut pending = self.paint_main_overlays(ui, chart_rect, cursor_idx);
        pending.extend(self.paint_sub_panes(
            ui, total_rect, chart_rect, &pane_slots, cursor_idx, modal_open,
        ));
        self.apply_legend_actions(pending);

        self.settings_modal.show(ui.ctx(), &mut self.manager);
        self.paint_drawings(ui, chart_rect);
        self.paint_crosshair(ui, chart_rect, &pane_slots);
    }

    fn dispatch_toolbar(&mut self, ui: &mut egui::Ui) {
        if let Some(ev) = self.toolbar.show(ui, &mut self.manager) {
            if let IndicatorBarEvent::Remove(id) = ev {
                self.manager.remove(id);
            }
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
        if idx >= self.data.len() { None } else { Some(idx) }
    }

    fn paint_wgpu_candles(&self, ui: &egui::Ui, chart_rect: egui::Rect) {
        let target_format = egui_wgpu::preferred_framebuffer_format(
            &[wgpu::TextureFormat::Bgra8Unorm, wgpu::TextureFormat::Rgba8Unorm],
        )
        .unwrap_or(wgpu::TextureFormat::Bgra8Unorm);
        let callback = ChartCallback {
            camera: self.camera.clone(),
            data: self.data.clone(),
            initialized: self.initialized.clone(),
            target_format,
        };
        ui.painter()
            .add(egui_wgpu::Callback::new_paint_callback(chart_rect, callback));
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
                    &painter, chart_rect, &camera,
                    &self.data, computed, &active.params,
                );
            }
        }

        // Snapshot legend items so the immutable borrow on manager is released
        // before draw_legend_row takes &mut ui.
        let items: Vec<LegendItem> = self
            .manager
            .main_overlays()
            .map(|(active, computed)| LegendItem {
                instance_id: active.instance_id,
                name: active.indicator().display_name(&active.params),
                entries: active.indicator().legend(
                    &self.data, computed, &active.params, cursor_idx,
                ),
                params: active.params.clone(),
            })
            .collect();

        let mut actions = Vec::new();
        for (i, item) in items.into_iter().enumerate() {
            let action = draw_legend_row(
                ui,
                chart_rect,
                egui::Pos2::new(
                    chart_rect.left() + 8.0,
                    chart_rect.top() + 26.0 + (i as f32 * 14.0),
                ),
                &item.name,
                &item.entries,
                &format!("main-{}", item.instance_id),
            );
            if action != LegendAction::None {
                actions.push((item.instance_id, action, item.params));
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
            let Some((_, pane_rect)) = pane_slots.get(i) else { continue };
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
        Box::new(move |idx: f32| -> f32 {
            left + ((idx as f64 - x_offset) * x_scale) as f32
        })
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
            let Some((active, computed)) = sub_list.get(i) else { continue };
            let painter = ui.painter_at(*pane_rect);
            active.indicator().draw_pane(
                &painter, *pane_rect, x_mapper,
                &self.data, computed, &active.params,
            );
            items.push(LegendItem {
                instance_id: active.instance_id,
                name: active.indicator().display_name(&active.params),
                entries: active.indicator().legend(
                    &self.data, computed, &active.params, cursor_idx,
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
                        ui, *divider_rect, total_rect, "main_divider",
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
        let Some(def_id) = self.toolbar.active_drawing else { return };
        let Some(tool) = self.drawings.tool_for(def_id) else { return };
        let camera = self.camera.lock();
        let result = tool.handle_input(ui, chart_rect, &camera, &mut self.drawings.draft);
        drop(camera);
        if let drawings::InputResult::Commit(points) = result {
            self.drawings.commit(tool.id(), points);
        }
    }

    fn paint_drawings(&self, ui: &egui::Ui, chart_rect: egui::Rect) {
        let camera = self.camera.lock();
        let painter = ui.painter_at(chart_rect);
        for drawing in &self.drawings.committed {
            let Some(tool) = self.drawings.tool_for(drawing.def_id) else { continue };
            tool.render(&painter, chart_rect, &camera, &drawing.points);
        }
        if let Some(draft) = self.drawings.draft.as_ref() {
            if let Some(tool) = self.drawings.tool_for(draft.def_id) {
                tool.render_preview(&painter, chart_rect, &camera, &draft.points);
            }
        }
    }

    fn paint_crosshair(
        &self,
        ui: &egui::Ui,
        chart_rect: egui::Rect,
        pane_slots: &[(egui::Rect, egui::Rect)],
    ) {
        let camera = self.camera.lock();
        let bottom = pane_slots
            .last()
            .map(|(_, p)| p.bottom())
            .unwrap_or(chart_rect.bottom());
        let full_rect = egui::Rect::from_min_max(
            chart_rect.min,
            egui::Pos2::new(chart_rect.right(), bottom),
        );
        crosshair::paint_crosshair(ui, chart_rect, full_rect, &camera, &self.data);
    }
}

struct LegendItem {
    instance_id: u64,
    name: String,
    entries: Vec<indicators::LegendEntry>,
    params: ParamValues,
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
    let name_galley = painter.layout_no_wrap(display_name.to_string(), text_font.clone(), name_color);
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
    let gear_color = if gear_resp.hovered() { icon_hover } else { icon_color };
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
    let x_color = if x_resp.hovered() { icon_hover } else { icon_color };
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
