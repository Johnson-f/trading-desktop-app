//! Chart widget — egui-based candlestick chart with indicators,
//! drawings, multi-pane layout, and async data loading.
//!
//! Use [`init`] at application boot to wire up the runtime handle and
//! bearer-token provider before constructing any [`ChartWidget`].

mod camera;
mod candle;
mod config;
mod controls;
mod crosshair;
mod data_loading;
mod drawing_input;
pub mod drawings;
mod gaps;
mod grid;
mod indicators;
mod interaction;
mod layout;
mod legend;
pub mod loader;
pub mod multi_charts;
mod overlays;
mod pane;
mod range;
mod range_markers;
mod renderer;
mod shadcn_theme;
mod util;

use egui::mutex::Mutex;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_CHART_ID: AtomicU64 = AtomicU64::new(1);

use camera::Camera;
pub use candle::{CandleData, JsonCandle, Timeframe};
pub use config::{BearerProvider, Config, init};
use controls::{ChartToolbar, DrawingSettingsModal, IndicatorBarEvent, SettingsModal};
use data_loading::parse_date_to_utc_midnight;
use drawings::DrawingsManager;
use indicators::{self as ind, IndicatorEvent, IndicatorManager};
use interaction::InteractionState;
use pane::SubPaneStack;
pub use range::Range;

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
    /// In-flight historical-bars fetch triggered by `set_symbol`. Polled
    /// each frame; on success the result feeds `set_data`.
    pending_candles_load: Option<tokio::sync::oneshot::Receiver<Result<CandleData, String>>>,
    /// In-flight backfill fetch for older bars (triggered when the user
    /// scrolls past the leftmost loaded candle). Polled each frame; on
    /// success the result is prepended to `raw_data` and the camera
    /// shifts so the user's view doesn't jump. None when no fetch is in
    /// flight, which is also the gate against firing duplicate requests.
    pending_older_candles_load: Option<tokio::sync::oneshot::Receiver<Result<CandleData, String>>>,
    /// Latched true when a backfill fetch returns an empty `CandleData`,
    /// signalling Yahoo has no older history for this symbol. Stops us
    /// from firing the same exhausting request every frame. Reset on
    /// `set_symbol` (the next symbol may have deeper history).
    older_history_exhausted: bool,
    /// UTC midnight of the earliest bar currently loaded — anchors the
    /// next backfill request. Updated on every `set_data` and merge.
    /// `None` until the first data lands.
    earliest_loaded_ts: Option<chrono::DateTime<chrono::Utc>>,
    /// Live-tick subscription receiver for the active symbol. Created by
    /// `set_symbol`, drained each frame in `poll_pending_ticks`. Drop
    /// closes the WS — `set_symbol` overwrites with `None` first to
    /// release the old subscription before opening the new one.
    pending_ticks: Option<tokio::sync::mpsc::UnboundedReceiver<api_client::TickEvent>>,
    /// Last instant we kicked off an async save. Used to debounce: we wait
    /// `DRAWINGS_SAVE_DEBOUNCE` after the last mutation before saving so a
    /// burst of edits (drag-resizing a trendline) coalesces into one write.
    drawings_dirty_since: Option<std::time::Instant>,
    /// Eye-icon flag: when true, committed drawings are skipped in the paint
    /// loop and selection. Purely transient (does NOT persist).
    drawings_hidden: bool,
    /// When true the indicator-legend rows below the OHLC row are hidden.
    /// Toggled by the caret button rendered between the OHLC row and the
    /// legend. Purely UI state (not persisted).
    indicator_legend_hidden: bool,
    /// Currently selected data window (Range bar). Purely UI state; drives
    /// camera positioning when clicked.
    selected_range: Range,
}

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

        let earliest_loaded_ts = parse_date_to_utc_midnight(data.dates.first().map(|s| s.as_str()));

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
            pending_candles_load: None,
            pending_older_candles_load: None,
            older_history_exhausted: false,
            earliest_loaded_ts,
            pending_ticks: None,
            drawings_dirty_since: None,
            drawings_hidden: false,
            indicator_legend_hidden: false,
            selected_range: Range::Max,
        }
    }

    /// Switch the base timeframe (Daily / Weekly / Monthly). Rebuilds the
    /// active data from `raw_data` and re-fits the camera so the newly-
    /// aggregated range shows cleanly.
    pub fn set_timeframe(&mut self, timeframe: Timeframe) {
        if timeframe == self.timeframe {
            return;
        }
        let prev_scale = self.timeframe.base_scale();
        let new_scale = timeframe.base_scale();
        self.timeframe = timeframe;

        if prev_scale != new_scale {
            // Base-scale change (e.g. Daily ↔ Minute1) requires a fresh
            // fetch from the gateway because the underlying series is
            // different — daily bars vs 1-minute bars. Clear the chart
            // immediately, fire a new historical load, and let the
            // existing per-frame poll wire up the result via
            // `set_data`. Tick subscription stays open; deltas keep
            // landing into the (soon-rebuilt) `raw_data`.
            self.raw_data = Arc::new(CandleData {
                instances: Vec::new(),
                dates: Vec::new(),
            });
            self.data = Arc::new(self.raw_data.aggregated(timeframe));
            self.manager.invalidate_cache();
            self.drawings.remap_to_data(&self.data);
            self.older_history_exhausted = false;
            self.earliest_loaded_ts = None;
            if let Some(symbol) = self.symbol.clone() {
                self.pending_candles_load =
                    Some(crate::loader::candle_loader::load_async(symbol, new_scale));
            }
            // Camera re-fits when the new data lands (`set_data` does
            // it). Don't fit to empty data here — that just centers on
            // index 0 with no real bounds.
            return;
        }

        // Same-base-scale switch (Daily ↔ Weekly ↔ Monthly): just re-
        // aggregate in memory.
        self.data = Arc::new(self.raw_data.aggregated(timeframe));
        self.manager.invalidate_cache();
        self.drawings.remap_to_data(&self.data);
        let mut camera = self.camera.lock();
        camera.x_scale = 16.0;
        camera.fit_to_data(&self.data);
    }

    /// Replace the underlying raw data (e.g. on a symbol change) and re-aggregate
    /// to the current timeframe. Indicators are invalidated (will recompute against
    /// the new series next frame); committed drawings are remapped by date and
    /// any whose dates don't appear in the new data are dropped by the existing
    /// remap path. Camera is refit so the new data shows cleanly.
    pub fn set_data(&mut self, new_raw: CandleData) {
        self.pending_symbol_change = Some(new_raw.clone());
        self.earliest_loaded_ts =
            parse_date_to_utc_midnight(new_raw.dates.first().map(|s| s.as_str()));
        // New base data may have older history available — reset the
        // exhausted flag so backfill can re-engage.
        self.older_history_exhausted = false;
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
        self.poll_pending_candles_load();
        self.poll_pending_older_candles_load();
        self.maybe_load_older_candles();
        self.poll_pending_ticks();
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

        let full_rect = layout::full_chart_rect(chart_rect, &pane_slots);

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

        // Faded chart watermark: "{SYMBOL}, {TF}" big and gray, behind candles.
        // Must be painted BEFORE paint_wgpu_candles so egui submits this draw
        // call first; WGPU then composites its candle layer on top.
        if let Some(sym) = self.symbol.as_deref() {
            let watermark = format!("{}, {}", sym, self.timeframe.short_label());
            let painter = ui.painter_at(chart_rect);
            painter.text(
                chart_rect.center(),
                egui::Align2::CENTER_CENTER,
                watermark,
                egui::FontId::proportional(72.0),
                egui::Color32::from_rgba_premultiplied(80, 80, 80, 24),
            );
        }

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

        self.settings_modal.show(ui, &mut self.manager);
        if !self.drawings_hidden {
            self.paint_drawings(ui, chart_rect, full_rect);
            self.paint_drawing_hover_tooltip(ui, chart_rect, full_rect);
            self.paint_drawing_selection(ui, chart_rect, full_rect);
        }
        self.drawing_settings_modal
            .show(ui, &mut self.drawings, &self.data);

        if let Some(ref date) = self.ghost_cursor_date {
            let camera = self.camera.lock();
            crosshair::paint_ghost(ui.painter(), chart_rect, &camera, &self.data, date);
        }
        self.ghost_cursor_date = None;

        // Paint the price-scale gutter chrome BEFORE the crosshair so the
        // gutter mask occludes drawings/indicators bleed-through, but the
        // crosshair (which logically sits on top of the entire chart) paints
        // OVER the gutter — its horizontal line and price label both remain
        // visible in the gutter region.
        {
            let camera = self.camera.lock();
            grid::paint_price_axis(ui, chart_rect, &camera, &self.data);
        }
        self.paint_crosshair(ui, chart_rect, full_rect);

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
        // Kick off async loads — the per-frame poll helpers swap each
        // result in once its future resolves.
        self.pending_drawings_load = Some(drawings::persistence::load_async(symbol.clone()));
        self.pending_candles_load = Some(crate::loader::candle_loader::load_async(
            symbol.clone(),
            self.timeframe.base_scale(),
        ));
        // New symbol → reset backfill state. The previous symbol's
        // in-flight request would land into the wrong data; the
        // exhausted flag is also per-symbol.
        self.pending_older_candles_load = None;
        self.older_history_exhausted = false;
        // Live ticks: drop the old receiver first (closes the previous
        // WebSocket on the spawned task's next send) before opening a
        // new subscription for `symbol`.
        self.pending_ticks = None;
        self.pending_ticks = Some(crate::loader::tick_stream::subscribe(symbol));
    }

    /// Toggle whether the indicator-legend rows below the OHLC row are shown.
    pub fn toggle_indicator_legend(&mut self) {
        self.indicator_legend_hidden = !self.indicator_legend_hidden;
    }

    pub fn indicator_legend_hidden(&self) -> bool {
        self.indicator_legend_hidden
    }

    pub fn has_non_default_indicators(&self) -> bool {
        // "volume" is the default added in `ChartWidget::new`; anything else
        // counts as user-added.
        self.manager.active.iter().any(|a| a.def_id != "volume")
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
}
