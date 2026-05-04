mod camera;
mod candle;
mod controls;
mod crosshair;
pub mod drawings;
mod gaps;
mod grid;
mod indicators;
mod interaction;
pub mod multi_charts;
mod pane;
mod range_markers;
mod renderer;
mod util;

use egui::mutex::Mutex;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_CHART_ID: AtomicU64 = AtomicU64::new(1);

use camera::Camera;
pub use candle::{CandleData, JsonCandle, Timeframe};
use controls::{ChartToolbar, DrawingSettingsModal, IndicatorBarEvent, SettingsModal};
use drawings::{
    DrawingsManager, HIT_TOLERANCE_PX, SelectionInput, ToolbarEvent, paint_handles, selection_step,
    show_toolbar, toolbar_anchor_rect,
};
use indicators::{self as ind, IndicatorEvent, IndicatorManager, ParamValues};
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
    /// Unique id used to scope this chart's GPU resources in the shared
    /// egui_wgpu callback resource map. Every pane needs its own slot,
    /// otherwise the last-prepared pane's vertex/camera buffers visually
    /// drive every other pane.
    id: u64,
    sub_stack: SubPaneStack,
    toolbar: ChartToolbar,
    manager: IndicatorManager,
    settings_modal: SettingsModal,
    drawings: DrawingsManager,
    drawing_settings_modal: DrawingSettingsModal,
    notification: Option<(String, std::time::Instant)>,
    /// Cached cursor index from the most recent frame. `None` when the pointer
    /// is not over the chart. Updated each `show()` call; read by sync passes.
    last_cursor_idx: Option<usize>,
    /// Set when an external caller invoked `set_data` and we want sync mirroring
    /// to know about it. Drained by `take_change_events`.
    pending_symbol_change: Option<CandleData>,
    /// External date pushed into this pane (e.g. by Crosshair sync from another
    /// pane). Painted as a dashed vertical line, no OHLC tooltip. Cleared each
    /// frame after painting; sync passes set it again every frame.
    ghost_cursor_date: Option<String>,
    /// When true, all pointer-driven handlers (pan/zoom, drawing, price-axis
    /// scaling) skip this frame. Set by `MultiChartWidget` while the user is
    /// dragging a splitter — without it, the splitter hit zone and the chart's
    /// interact rect overlap by 1 px, so any vertical jitter during a resize
    /// hits the pan path and silently flips `auto_scale_y` off.
    input_suppressed: bool,
    /// When true, `show()` skips rendering the chart toolbar — `MultiChartWidget`
    /// hoists the active pane's toolbar above all panes so it's shared across
    /// the multi-chart layout instead of duplicated per pane.
    suppress_toolbar: bool,
    /// Symbol this chart is bound to. Set via `set_symbol()` (called by the
    /// containing widget when the symbol is known). Used as the persistence
    /// key for committed drawings — `None` means "don't persist."
    symbol: Option<String>,
    /// In-flight async load triggered by `set_symbol`. Polled each frame; on
    /// completion the result replaces `drawings.committed`.
    pending_drawings_load: Option<tokio::sync::oneshot::Receiver<Vec<drawings::CommittedDrawing>>>,
    /// Last instant we kicked off an async save. Used to debounce: we wait
    /// `DRAWINGS_SAVE_DEBOUNCE` after the last mutation before saving so a
    /// burst of edits (drag-resizing a trendline) coalesces into one write.
    drawings_dirty_since: Option<std::time::Instant>,
    /// Eye-icon flag: when true, committed drawings are skipped in the paint
    /// loop and selection. Purely transient (does NOT persist).
    drawings_hidden: bool,
}

const DRAWINGS_SAVE_DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(500);

/// Mutations originating from this `ChartWidget` since the last drain.
/// Consumed by `MultiChartWidget` to mirror changes onto sibling panes.
pub struct ChangeEvents {
    pub symbol_changed: Option<CandleData>,
    pub indicator_events: Vec<IndicatorEvent>,
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
            id: NEXT_CHART_ID.fetch_add(1, Ordering::Relaxed),
            sub_stack: SubPaneStack::default(),
            toolbar: ChartToolbar::default(),
            manager,
            settings_modal: SettingsModal::default(),
            drawings: DrawingsManager::default(),
            drawing_settings_modal: DrawingSettingsModal::default(),
            notification: None,
            last_cursor_idx: None,
            pending_symbol_change: None,
            ghost_cursor_date: None,
            input_suppressed: false,
            suppress_toolbar: false,
            symbol: None,
            pending_drawings_load: None,
            drawings_dirty_since: None,
            drawings_hidden: false,
        }
    }

    /// Switch the base timeframe (Daily / Weekly / Monthly). Rebuilds the
    /// active data from `raw_data` and re-fits the camera so the newly-
    /// aggregated range shows cleanly.
    pub fn set_timeframe(&mut self, timeframe: Timeframe) {
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

    /// Replace the underlying raw data (e.g. on a symbol change) and re-aggregate
    /// to the current timeframe. Indicators are invalidated (will recompute against
    /// the new series next frame); committed drawings are remapped by date and
    /// any whose dates don't appear in the new data are dropped by the existing
    /// remap path. Camera is refit so the new data shows cleanly.
    pub fn set_data(&mut self, new_raw: CandleData) {
        self.pending_symbol_change = Some(new_raw.clone());
        self.raw_data = Arc::new(new_raw);
        self.data = Arc::new(self.raw_data.aggregated(self.timeframe));
        self.manager.invalidate_cache();
        self.drawings.remap_to_data(&self.data);
        let mut camera = self.camera.lock();
        camera.fit_to_data(&self.data);
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        // Per-frame lifecycle: pull in any completed async load, then schedule
        // a save if drawings are dirty and the debounce window has elapsed.
        self.poll_pending_drawings_load();
        self.flush_dirty_drawings();

        if !self.suppress_toolbar {
            self.dispatch_toolbar(ui);
        }

        let (total_rect, chart_rect, pane_slots, footer_rect) = self.layout_rects(ui);
        self.update_camera_viewport(chart_rect);

        let modal_open = self.toolbar.indicator_modal.open
            || self.settings_modal.open
            || self.drawing_settings_modal.open;
        let drawing_tool_armed = self.toolbar.active_drawing.is_some();

        let drawing_drag_active = !matches!(self.drawings.drag, drawings::SelectionDrag::None);

        if !modal_open && !drawing_drag_active && !self.input_suppressed {
            self.handle_chart_interaction(ui, total_rect, chart_rect);
        }

        let full_rect = full_chart_rect(chart_rect, &pane_slots);

        if !modal_open && drawing_tool_armed && !self.input_suppressed && !self.drawings_hidden {
            self.handle_drawing_input(ui, chart_rect);
        } else if !drawing_tool_armed {
            self.drawings.cancel_draft();
            // When drawings are hidden, suppress selection too — clicking empty
            // space shouldn't pick up an invisible shape.
            if !modal_open && !self.input_suppressed && !self.drawings_hidden {
                self.handle_drawing_selection(ui, chart_rect, full_rect);
            }
        }

        self.manager.ensure_computed(&self.data);
        let cursor_idx = self.compute_cursor_idx(ui, chart_rect, &pane_slots);
        self.last_cursor_idx = cursor_idx;

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
        if !self.drawings_hidden {
            self.paint_drawings(ui, chart_rect, full_rect);
            self.paint_drawing_hover_tooltip(ui, chart_rect, full_rect);
            self.paint_drawing_selection(ui, chart_rect, full_rect);
        }
        self.drawing_settings_modal
            .show(ui.ctx(), &mut self.drawings, &self.data);
        self.paint_crosshair(ui, chart_rect, full_rect);

        if let Some(ref date) = self.ghost_cursor_date {
            let camera = self.camera.lock();
            crosshair::paint_ghost(ui.painter(), chart_rect, &camera, &self.data, date);
        }
        self.ghost_cursor_date = None;

        // Render notification toast
        self.paint_notification(ui, total_rect);

        self.show_footer(ui, footer_rect);
    }

    pub fn timeframe(&self) -> Timeframe {
        self.timeframe
    }

    /// Clone of the underlying raw data Arc — used when promoting Single → Multi
    /// so the new `MultiChartWidget` shares the same source-of-truth.
    pub fn raw_data_arc(&self) -> Arc<CandleData> {
        self.raw_data.clone()
    }

    /// The symbol this chart is showing. `None` until `set_symbol()` is called
    /// (typically by the containing widget once the data is bound to a symbol).
    pub fn symbol(&self) -> Option<String> {
        self.symbol.clone()
    }

    /// Bind this chart to a symbol. Triggers:
    ///   1. Save of any current dirty drawings (under the previous symbol).
    ///   2. Wipe of in-memory drawings.
    ///   3. Async load of the new symbol's persisted drawings.
    /// No-op when the symbol is unchanged.
    pub fn set_symbol(&mut self, symbol: String) {
        if self.symbol.as_deref() == Some(symbol.as_str()) {
            return;
        }
        // Flush any pending dirty drawings under the OLD symbol before swapping.
        if let Some(prev) = self.symbol.take() {
            if self.drawings.dirty {
                drawings::persistence::save_all(prev, self.drawings.committed.clone());
                self.drawings.dirty = false;
                self.drawings_dirty_since = None;
            }
        }
        self.symbol = Some(symbol.clone());
        self.drawings.replace_committed(Vec::new());
        self.drawings_hidden = false;
        // Kick off async load — `poll_pending_drawings_load` swaps the result
        // into `drawings.committed` once the future resolves.
        self.pending_drawings_load = Some(drawings::persistence::load_async(symbol));
    }

    /// Wipe every committed drawing on this chart (in-memory + DB).
    /// Triggered by the toolbar's trash icon after the user confirms.
    pub fn clear_all_drawings(&mut self) {
        self.drawings.clear_all();
        if let Some(symbol) = &self.symbol {
            drawings::persistence::delete_all(symbol.clone());
        }
        // We've already deleted from the DB; suppress the debounced save that
        // `clear_all` would otherwise schedule (it would be a redundant DELETE +
        // empty-set INSERT).
        self.drawings.dirty = false;
        self.drawings_dirty_since = None;
    }

    /// Toggle the eye-icon visibility flag (UI-only, not persisted).
    pub fn toggle_drawings_visibility(&mut self) {
        self.drawings_hidden = !self.drawings_hidden;
    }

    pub fn drawings_hidden(&self) -> bool {
        self.drawings_hidden
    }

    /// Drain a completed async drawings load, if one finished. Called once per
    /// frame from `show()`.
    fn poll_pending_drawings_load(&mut self) {
        let Some(rx) = self.pending_drawings_load.as_mut() else {
            return;
        };
        match rx.try_recv() {
            Ok(loaded) => {
                self.drawings.replace_committed(loaded);
                self.drawings.remap_to_data(&self.data);
                self.pending_drawings_load = None;
            }
            Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                // Still loading; check again next frame.
            }
            Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
                // Sender dropped without sending — treat as empty.
                self.pending_drawings_load = None;
            }
        }
    }

    /// If the manager is dirty AND it's been at least `DRAWINGS_SAVE_DEBOUNCE`
    /// since the last edit, kick off an async save and clear the flag. Called
    /// once per frame from `show()`.
    fn flush_dirty_drawings(&mut self) {
        if !self.drawings.dirty {
            self.drawings_dirty_since = None;
            return;
        }
        let Some(symbol) = self.symbol.clone() else {
            // No symbol = nothing to persist against. Keep the flag set in case
            // a symbol is bound later, but don't write.
            return;
        };
        let now = std::time::Instant::now();
        let since = self.drawings_dirty_since.get_or_insert(now);
        if now.duration_since(*since) >= DRAWINGS_SAVE_DEBOUNCE {
            drawings::persistence::save_all(symbol, self.drawings.committed.clone());
            self.drawings.dirty = false;
            self.drawings_dirty_since = None;
        }
    }

    pub fn has_user_drawings(&self) -> bool {
        !self.drawings.committed.is_empty()
    }

    pub fn has_non_default_indicators(&self) -> bool {
        // "volume" is the default added in `ChartWidget::new`; anything else
        // counts as user-added.
        self.manager.active.iter().any(|a| a.def_id != "volume")
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

    /// Drain mutations recorded since the last call. Always-safe to call —
    /// returns empty `ChangeEvents` if nothing changed.
    pub fn take_change_events(&mut self) -> ChangeEvents {
        ChangeEvents {
            symbol_changed: self.pending_symbol_change.take(),
            indicator_events: self.manager.take_events(),
        }
    }

    /// Show a "ghost" crosshair anchored at the given date, distinct from the
    /// pane's own pointer-driven crosshair. Pass `None` to clear.
    pub fn set_ghost_cursor(&mut self, date: Option<String>) {
        self.ghost_cursor_date = date;
    }

    /// Drain the toolbar's multi-chart toggle request. Called by `MyApp` each
    /// frame to know when to swap `ChartView` variants.
    pub fn take_multi_chart_toggle_request(&mut self) -> bool {
        let pending = self.toolbar.toggle_multi_chart_pending;
        self.toolbar.toggle_multi_chart_pending = false;
        pending
    }

    /// Drain the toolbar's grid layout request. Called by `MyApp` (single mode)
    /// and `MultiChartWidget` (multi mode) so a click on the inline grid bar
    /// can promote/demote between Single/Multi or just change the layout.
    pub fn take_grid_layout_request(&mut self) -> Option<multi_charts::GridLayout> {
        self.toolbar.pending_grid_layout.take()
    }

    /// Tell the toolbar which grid layout the enclosing `MultiChartWidget` is
    /// currently using, so the inline grid bar can highlight it. `Single` for
    /// non-multi charts.
    pub fn set_active_grid_layout(&mut self, layout: multi_charts::GridLayout) {
        self.toolbar.active_grid_layout = layout;
    }

    /// Tell the toolbar which sync flags the enclosing `MultiChartWidget` is
    /// currently using, so the inline grid bar's checkboxes are checked
    /// correctly. Defaults to all-off for non-multi charts.
    pub fn set_active_sync_flags(&mut self, flags: multi_charts::SyncFlags) {
        self.toolbar.active_sync_flags = flags;
    }

    /// Drain a sync-flags change requested via the inline grid bar.
    pub fn take_sync_flags_request(&mut self) -> Option<multi_charts::SyncFlags> {
        self.toolbar.pending_sync_flags.take()
    }

    /// Suppress pointer-driven input for this frame. Used by `MultiChartWidget`
    /// while a splitter is being dragged so resize gestures don't bleed into
    /// the chart's pan handler (which would otherwise flip `auto_scale_y` off).
    pub fn set_input_suppressed(&mut self, suppressed: bool) {
        self.input_suppressed = suppressed;
    }

    /// When true, `show()` skips its internal toolbar render. Used by
    /// `MultiChartWidget` to hoist the active pane's toolbar above all panes.
    pub fn set_suppress_toolbar(&mut self, suppressed: bool) {
        self.suppress_toolbar = suppressed;
    }

    /// Render this chart's toolbar in an externally-provided `ui`. Used by
    /// `MultiChartWidget` to display the active pane's toolbar above the
    /// pane grid, so all panes share one toolbar instead of duplicating it.
    pub fn show_toolbar(&mut self, ui: &mut egui::Ui) {
        self.dispatch_toolbar(ui);
    }

    /// Snapshot of toolbar UI state (which inline bar is open, current tool).
    /// Used by `MultiChartWidget` to keep all panes in sync so swapping the
    /// active pane (cursor-driven) doesn't drop whichever inline bar the
    /// user just opened.
    pub fn toolbar_ui_state(&self) -> controls::ToolbarUiState {
        self.toolbar.ui_state()
    }

    pub fn set_toolbar_ui_state(&mut self, state: controls::ToolbarUiState) {
        self.toolbar.set_ui_state(state);
    }

    pub fn manager(&self) -> &IndicatorManager {
        &self.manager
    }

    pub fn manager_mut(&mut self) -> &mut IndicatorManager {
        &mut self.manager
    }

    fn dispatch_toolbar(&mut self, ui: &mut egui::Ui) {
        // Mirror the chart's drawings-hidden flag back into the toolbar so the
        // eye icon's active-state reflects reality.
        self.toolbar.drawings_hidden = self.drawings_hidden;

        if let Some(ev) = self.toolbar.show(ui, &mut self.manager) {
            if let IndicatorBarEvent::Remove(id) = ev {
                self.manager.remove(id);
            }
        }

        // Drain the toolbar's pending drawings actions.
        if std::mem::take(&mut self.toolbar.pending_toggle_drawings_visibility) {
            self.toggle_drawings_visibility();
        }
        if std::mem::take(&mut self.toolbar.pending_clear_drawings) {
            self.clear_all_drawings();
        }
    }

    fn layout_rects(
        &mut self,
        ui: &mut egui::Ui,
    ) -> (egui::Rect, egui::Rect, Vec<(egui::Rect, egui::Rect)>, egui::Rect) {
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
            id: self.id,
            camera: self.camera.clone(),
            data: self.data.clone(),
            target_format,
        };
        ui.painter().add(egui_wgpu::Callback::new_paint_callback(
            chart_rect, callback,
        ));
    }

    fn paint_grid(&self, ui: &mut egui::Ui, chart_rect: egui::Rect, modal_open: bool) {
        if !modal_open && !self.input_suppressed {
            let mut camera = self.camera.lock();
            grid::handle_price_axis_drag(ui, chart_rect, &mut camera);
        }
        let camera = self.camera.lock();
        grid::paint_price_grid(ui, chart_rect, &camera, &self.data);
        grid::paint_time_grid(ui, chart_rect, &camera, &self.data);
    }

    /// Render the bottom-of-chart footer: Interval (timeframe) buttons on the
    /// left, Auto (y-axis auto-scale) toggle on the right.
    fn show_footer(&mut self, ui: &mut egui::Ui, footer_rect: egui::Rect) {
        use eframe::egui::{Align, Color32, CornerRadius, Layout, RichText, Vec2};

        const ICON_COLOR: Color32 = Color32::from_rgb(120, 120, 130);
        const ICON_HOVER: Color32 = Color32::from_rgb(200, 200, 210);
        const ACCENT: Color32 = Color32::from_rgb(78, 205, 196);
        const ACTIVE_BG: Color32 = Color32::from_rgb(35, 35, 40);
        const BORDER: Color32 = Color32::from_rgb(50, 50, 55);

        let mut new_timeframe: Option<Timeframe> = None;
        let current_tf = self.timeframe;

        ui.scope_builder(egui::UiBuilder::new().max_rect(footer_rect), |ui| {
            ui.style_mut().interaction.tooltip_delay = 0.0;
            ui.horizontal_centered(|ui| {
                ui.add_space(8.0);
                ui.label(RichText::new("Interval:").size(11.0).color(ICON_COLOR));
                ui.add_space(4.0);
                for tf in Timeframe::ALL {
                    let selected = current_tf == *tf;
                    let color = if selected { ACCENT } else { ICON_COLOR };
                    let fill = if selected { ACTIVE_BG } else { Color32::TRANSPARENT };
                    let btn = ui.add(
                        egui::Button::new(
                            RichText::new(tf.short_label()).size(11.0).color(color),
                        )
                        .fill(fill)
                        .corner_radius(CornerRadius::same(3))
                        .min_size(Vec2::new(28.0, 20.0)),
                    );
                    if btn.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if btn.clicked() && !selected {
                        new_timeframe = Some(*tf);
                    }
                    btn.on_hover_text(tf.long_label());
                }

                // Auto toggle on the far right.
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(8.0);
                    let mut camera = self.camera.lock();
                    let is_auto = camera.auto_scale_y;
                    let color = if is_auto { ACCENT } else { ICON_HOVER };
                    let fill = if is_auto { Color32::TRANSPARENT } else { ACTIVE_BG };
                    let stroke = if is_auto {
                        egui::Stroke::new(1.0, ACCENT)
                    } else {
                        egui::Stroke::new(0.5, BORDER)
                    };
                    let btn = ui.add(
                        egui::Button::new(RichText::new("Auto").size(10.0).color(color))
                            .fill(fill)
                            .stroke(stroke)
                            .corner_radius(CornerRadius::same(3))
                            .min_size(Vec2::new(40.0, 18.0)),
                    );
                    if btn.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if btn.clicked() {
                        camera.auto_scale_y = !camera.auto_scale_y;
                    }
                });
            });
        });

        if let Some(tf) = new_timeframe {
            self.set_timeframe(tf);
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
                &drawing.kind_style,
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

    /// Show an immediate (no-delay) label naming the drawing under the cursor.
    /// Painted directly on the chart painter as a small pill anchored to the
    /// drawing's bounds (above, or below if it wouldn't fit) — never on top
    /// of the drawing itself, and position is stable so it doesn't flicker.
    /// Suppressed while placing / dragging.
    fn paint_drawing_hover_tooltip(
        &self,
        ui: &egui::Ui,
        chart_rect: egui::Rect,
        full_rect: egui::Rect,
    ) {
        if self.drawings.draft.is_some() {
            return;
        }
        if !matches!(self.drawings.drag, drawings::SelectionDrag::None) {
            return;
        }
        let Some(pointer) = ui.input(|i| i.pointer.hover_pos()) else {
            return;
        };
        if !chart_rect.contains(pointer) {
            return;
        }

        let camera = self.camera.lock();
        for drawing in self.drawings.committed.iter().rev() {
            let Some(tool) = self.drawings.tool_for(drawing.def_id) else {
                continue;
            };
            if !tool.hit_test(
                chart_rect,
                full_rect,
                &camera,
                &drawing.points,
                pointer,
                HIT_TOLERANCE_PX,
            ) {
                continue;
            }

            let label = tool.display_name().to_string();
            let bounds = tool.bounds(chart_rect, full_rect, &camera, &drawing.points);

            let painter = ui.painter_at(chart_rect);
            let font = egui::FontId::proportional(11.0);
            let text_color = egui::Color32::from_rgb(225, 225, 230);
            let bg = egui::Color32::from_rgba_premultiplied(30, 30, 36, 230);
            let border = egui::Color32::from_rgb(60, 60, 66);

            let galley = painter.layout_no_wrap(label, font, text_color);
            let pad = egui::vec2(8.0, 4.0);
            let size = galley.size() + pad * 2.0;

            // Anchor horizontally to the bounds' midpoint; vertically above
            // the bounds with a small gap, flipping below if that'd clip the
            // chart top. Always offset so the pill never overlaps the shape.
            let gap = 6.0;
            let mut top = bounds.top() - size.y - gap;
            if top < chart_rect.top() + 2.0 {
                top = bounds.bottom() + gap;
            }
            let mut left = bounds.center().x - size.x * 0.5;
            left = left
                .max(chart_rect.left() + 2.0)
                .min(chart_rect.right() - size.x - 2.0);

            let rect = egui::Rect::from_min_size(egui::pos2(left, top), size);
            painter.rect_filled(rect, egui::CornerRadius::same(3), bg);
            painter.rect_stroke(
                rect,
                egui::CornerRadius::same(3),
                egui::Stroke::new(0.5, border),
                egui::StrokeKind::Inside,
            );
            painter.galley(rect.min + pad, galley, text_color);
            break;
        }
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

        let key_pressed = if ui.ctx().egui_wants_keyboard_input() {
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
        // Snapshot style + kind_style before the popups so we can detect
        // in-place mutations from the color/dash/width popups (which don't
        // signal back through the event channel) and mark the manager dirty.
        let (event, style_changed) = {
            let camera = self.camera.lock();
            let DrawingsManager {
                committed,
                drag,
                toolbar_offset,
                ..
            } = &mut self.drawings;
            let drawing = &mut committed[idx];
            let style_before = drawing.style;
            let kind_style_before = drawing.kind_style;
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
            let changed = drawing.style != style_before || drawing.kind_style != kind_style_before;
            (event, changed)
        };
        if style_changed {
            self.drawings.dirty = true;
        }

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
            let bg_rect = egui::Rect::from_center_size(text_pos, galley.size() + padding * 2.0);
            painter.rect_filled(bg_rect, egui::CornerRadius::same(6), bg_color);

            // Draw text
            painter.galley(
                egui::Pos2::new(
                    text_pos.x - galley.size().x / 2.0,
                    text_pos.y - galley.size().y / 2.0,
                ),
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
