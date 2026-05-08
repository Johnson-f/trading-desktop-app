use super::{CandleData, ChartWidget, Timeframe};

fn apply_indicator_event(chart: &mut ChartWidget, ev: &super::indicators::IndicatorEvent) {
    use super::indicators::IndicatorEvent;
    match ev {
        IndicatorEvent::Added { def_id, params } => {
            chart.manager_mut().add(def_id, params.clone());
        }
        IndicatorEvent::Removed { def_id } => {
            chart.manager_mut().remove_all_of(def_id);
        }
        IndicatorEvent::ParamsUpdated { def_id, params } => {
            // Find the first matching def_id in the sibling pane and update.
            // Positional match per the design doc — Indicators-sync assumes
            // the panes have structurally identical indicator lists.
            if let Some(instance_id) = chart
                .manager()
                .active
                .iter()
                .find(|a| a.def_id == *def_id)
                .map(|a| a.instance_id)
            {
                chart
                    .manager_mut()
                    .update_params(instance_id, params.clone());
            }
        }
    }
}
use std::sync::Arc;

/// Axis along which a splitter divides the layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SplitterAxis {
    Vertical,   // a vertical line; drag varies x
    Horizontal, // a horizontal line; drag varies y
}

/// Grid layout configurations for multi-chart display
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridLayout {
    /// Single chart (1x1)
    Single,
    /// Two charts side by side (1x2)
    Horizontal2,
    /// Two charts stacked vertically (2x1)
    Vertical2,
    /// Four charts in a 2x2 grid
    Grid2x2,
    /// Three charts horizontally (1x3)
    Grid1x3,
    /// Three charts vertically (3x1)
    Grid3x1,
}

/// Independent toggles for cross-pane synchronization. Each axis is
/// independently controllable in the toolbar `Sync:` row.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SyncFlags {
    pub symbol: bool,
    pub time: bool,
    pub indicators: bool,
    pub crosshair: bool,
}

/// A single timeframe view of the chart data
pub struct TimeframeView {
    /// Unique identifier for this view
    pub id: usize,
    /// The timeframe this view displays
    pub timeframe: Timeframe,
    /// The chart widget for this view
    pub chart: ChartWidget,
    /// Whether this view is maximized
    pub maximized: bool,
}

/// Multi-chart widget displaying the same symbol across multiple timeframes
pub struct MultiChartWidget {
    /// The symbol being displayed across all views
    symbol: String,
    /// Raw daily candle data (source of truth)
    raw_data: Arc<CandleData>,
    /// Multiple timeframe views of the same data
    views: Vec<TimeframeView>,
    /// Current grid layout
    layout: GridLayout,
    /// Synchronization flags
    sync_flags: SyncFlags,
    /// Which view is currently active/focused
    active_view: Option<usize>,
    /// Splitter positions for resizable layouts (0.0 to 1.0)
    splitter_positions: Vec<f32>,
    /// While the user is dragging a splitter: `Some((axis, splitter_index))`.
    /// Set when primary mouse button goes down on a hit zone; cleared on release.
    /// Allows continued position updates even when the cursor moves outside the
    /// narrow hit strip.
    dragging_splitter: Option<(SplitterAxis, usize)>,
    /// Set when a pane's inline grid bar requested `Single`. Drained by
    /// `take_grid_layout_request` so `MyApp` can collapse Multi → Single.
    pending_grid_layout: Option<GridLayout>,
}

impl MultiChartWidget {
    /// Create a new multi-chart widget with a single initial view
    pub fn new(symbol: String, data: CandleData) -> Self {
        let raw_data = Arc::new(data);
        let mut views = Vec::new();

        // Start with a single daily view, bound to `symbol` so the chart's
        // drawing-persistence layer can key off it.
        let mut chart = ChartWidget::new((*raw_data).clone());
        chart.set_symbol(symbol.clone());
        views.push(TimeframeView {
            id: 0,
            timeframe: Timeframe::Daily,
            chart,
            maximized: false,
        });

        Self {
            symbol,
            raw_data,
            views,
            layout: GridLayout::Single,
            sync_flags: SyncFlags::default(),
            active_view: Some(0),
            splitter_positions: vec![0.5],
            dragging_splitter: None,
            pending_grid_layout: None,
        }
    }

    /// Wrap an existing `ChartWidget` as the first (and only) pane of a new
    /// `MultiChartWidget`. State (camera, indicators, drawings) is preserved
    /// because the chart is moved in, not rebuilt.
    pub fn from_chart(symbol: String, raw_data: Arc<CandleData>, mut chart: ChartWidget) -> Self {
        let timeframe = chart.timeframe();
        chart.set_symbol(symbol.clone());
        let views = vec![TimeframeView {
            id: 0,
            timeframe,
            chart,
            maximized: false,
        }];
        Self {
            symbol,
            raw_data,
            views,
            layout: GridLayout::Single,
            sync_flags: SyncFlags::default(),
            active_view: Some(0),
            splitter_positions: vec![],
            dragging_splitter: None,
            pending_grid_layout: None,
        }
    }

    /// Consume self and return the active pane's `ChartWidget`, dropping every
    /// other pane. If no view is marked active, falls back to view 0.
    pub fn into_active_chart(self) -> ChartWidget {
        let active_id = self.active_view.unwrap_or(0);
        let mut views = self.views;
        let pos = views.iter().position(|v| v.id == active_id).unwrap_or(0);
        let mut chart = views.swap_remove(pos).chart;
        // Multi mode suppresses per-pane toolbars; restore default so the
        // chart renders its own toolbar once it's standalone again.
        chart.set_suppress_toolbar(false);
        chart
    }

    /// Returns true if any pane besides the active one has user-added drawings
    /// or non-default indicators. Used to decide whether to show a confirm prompt
    /// before collapsing Multi → Single.
    pub fn inactive_panes_have_user_content(&self) -> bool {
        let active_id = self.active_view.unwrap_or(0);
        self.views
            .iter()
            .filter(|v| v.id != active_id)
            .any(|v| v.chart.has_user_drawings() || v.chart.has_non_default_indicators())
    }

    /// Get the current symbol
    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    /// Get the current layout
    pub fn layout(&self) -> GridLayout {
        self.layout
    }

    /// Get the current sync flags
    pub fn sync_flags(&self) -> SyncFlags {
        self.sync_flags
    }

    /// Set the sync flags
    pub fn set_sync_flags(&mut self, flags: SyncFlags) {
        self.sync_flags = flags;
    }

    /// Get a mutable reference to sync flags
    pub fn sync_flags_mut(&mut self) -> &mut SyncFlags {
        &mut self.sync_flags
    }

    /// Add a new timeframe view
    pub fn add_view(&mut self, timeframe: Timeframe) -> usize {
        let id = self.views.len();
        let aggregated_data = self.raw_data.aggregated(timeframe);

        self.views.push(TimeframeView {
            id,
            timeframe,
            chart: ChartWidget::new(aggregated_data),
            maximized: false,
        });

        id
    }

    /// Remove a timeframe view (must keep at least one view)
    pub fn remove_view(&mut self, id: usize) {
        if self.views.len() > 1 {
            self.views.retain(|v| v.id != id);
            // Reassign IDs to maintain sequential order
            for (i, view) in self.views.iter_mut().enumerate() {
                view.id = i;
            }
            // Update active view if needed
            if self.active_view == Some(id) {
                self.active_view = Some(0);
            }
        }
    }

    /// Switch a single pane's timeframe. Indicators and drawings are preserved
    /// (delegated to `ChartWidget::set_timeframe`, which invalidates indicator
    /// caches and remaps drawings by date).
    pub fn change_timeframe(&mut self, view_id: usize, timeframe: Timeframe) {
        if let Some(view) = self.views.iter_mut().find(|v| v.id == view_id) {
            view.timeframe = timeframe;
            view.chart.set_timeframe(timeframe);
        }
    }

    /// Update every pane's data to a new symbol. Each pane's existing indicators
    /// recompute against the new series; drawings remap by date. Used when
    /// Symbol-sync is on (or as a one-shot when the symbol changes outside of
    /// sync mode and the user has only the active pane in mind — call
    /// `set_symbol_for(view_id, ...)` for that case).
    pub fn set_symbol(&mut self, symbol: String, data: CandleData) {
        self.symbol = symbol.clone();
        self.raw_data = Arc::new(data);
        let raw = self.raw_data.as_ref().clone();
        for view in &mut self.views {
            view.chart.set_data(raw.clone());
            view.chart.set_symbol(symbol.clone());
        }
    }

    /// Update only the given pane's data — used when Symbol-sync is off and the
    /// user wants per-pane symbols.
    pub fn set_symbol_for(&mut self, view_id: usize, data: CandleData) {
        let is_active = self.active_view == Some(view_id);
        if let Some(view) = self.views.iter_mut().find(|v| v.id == view_id) {
            view.chart.set_data(data.clone());
        }
        if is_active {
            self.raw_data = Arc::new(data);
        }
    }

    /// Toggle maximize state for a view
    pub fn toggle_maximize(&mut self, view_id: usize) {
        if let Some(view) = self.views.iter_mut().find(|v| v.id == view_id) {
            view.maximized = !view.maximized;
        }
    }

    /// Suggest a logical next timeframe based on existing views
    fn suggest_next_timeframe(&self) -> Timeframe {
        let existing: Vec<Timeframe> = self.views.iter().map(|v| v.timeframe).collect();

        // Suggest in order: Daily, Weekly, Monthly
        for tf in [Timeframe::Daily, Timeframe::Weekly, Timeframe::Monthly] {
            if !existing.contains(&tf) {
                return tf;
            }
        }

        // Default to Daily if all are taken
        Timeframe::Daily
    }

    /// Set the grid layout and ensure we have enough views
    pub fn set_layout(&mut self, layout: GridLayout) {
        let required_views = match layout {
            GridLayout::Single => 1,
            GridLayout::Horizontal2 | GridLayout::Vertical2 => 2,
            GridLayout::Grid1x3 | GridLayout::Grid3x1 => 3,
            GridLayout::Grid2x2 => 4,
        };

        while self.views.len() < required_views {
            let next_timeframe = self.suggest_next_timeframe();
            self.add_view(next_timeframe);
        }

        let splitter_count = match layout {
            GridLayout::Single => 0,
            GridLayout::Horizontal2 | GridLayout::Vertical2 => 1,
            GridLayout::Grid1x3 | GridLayout::Grid3x1 => 2,
            GridLayout::Grid2x2 => 2,
        };
        self.splitter_positions.resize(splitter_count, 0.5);
        if splitter_count == 2 && matches!(layout, GridLayout::Grid1x3 | GridLayout::Grid3x1) {
            self.splitter_positions = vec![1.0 / 3.0, 2.0 / 3.0];
        }

        self.layout = layout;
    }

    /// Detect hover/drag on splitter hit zones and update `splitter_positions`.
    /// Called from `show()` BEFORE pane rendering so updated positions are picked
    /// up this frame.
    fn handle_splitter_drag(&mut self, ui: &egui::Ui, rect: egui::Rect) {
        let hit_w = 6.0; // 4 px gap + 1 px each side
        let pointer = ui.input(|i| i.pointer.interact_pos());
        let primary_down = ui.input(|i| i.pointer.primary_down());

        match self.layout {
            GridLayout::Single => {}
            GridLayout::Horizontal2 => {
                self.drag_v_splitter(ui, rect, 0, pointer, primary_down, hit_w);
            }
            GridLayout::Vertical2 => {
                self.drag_h_splitter(ui, rect, 0, pointer, primary_down, hit_w);
            }
            GridLayout::Grid1x3 => {
                self.drag_v_splitter(ui, rect, 0, pointer, primary_down, hit_w);
                self.drag_v_splitter(ui, rect, 1, pointer, primary_down, hit_w);
            }
            GridLayout::Grid3x1 => {
                self.drag_h_splitter(ui, rect, 0, pointer, primary_down, hit_w);
                self.drag_h_splitter(ui, rect, 1, pointer, primary_down, hit_w);
            }
            GridLayout::Grid2x2 => {
                self.drag_v_splitter(ui, rect, 0, pointer, primary_down, hit_w);
                self.drag_h_splitter(ui, rect, 1, pointer, primary_down, hit_w);
            }
        }
    }

    fn drag_v_splitter(
        &mut self,
        ui: &egui::Ui,
        rect: egui::Rect,
        idx: usize,
        pointer: Option<egui::Pos2>,
        primary_down: bool,
        hit_w: f32,
    ) {
        let Some(pos) = self.splitter_positions.get(idx).copied() else {
            return;
        };
        let Some(p) = pointer else {
            if !primary_down {
                if self.dragging_splitter == Some((SplitterAxis::Vertical, idx)) {
                    self.dragging_splitter = None;
                }
            }
            return;
        };
        let split_x = rect.left() + rect.width() * pos;
        let hit = egui::Rect::from_min_max(
            egui::pos2(split_x - hit_w / 2.0, rect.top()),
            egui::pos2(split_x + hit_w / 2.0, rect.bottom()),
        );

        // End drag on button release.
        if !primary_down {
            if self.dragging_splitter == Some((SplitterAxis::Vertical, idx)) {
                self.dragging_splitter = None;
            }
            if hit.contains(p) {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
            }
            return;
        }

        // Mouse is down. Either we're already dragging this splitter, or we just started.
        if self.dragging_splitter.is_none() && hit.contains(p) {
            self.dragging_splitter = Some((SplitterAxis::Vertical, idx));
        }
        if self.dragging_splitter == Some((SplitterAxis::Vertical, idx)) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
            let new_pos = ((p.x - rect.left()) / rect.width()).clamp(0.1, 0.9);
            self.splitter_positions[idx] = new_pos;
        } else if hit.contains(p) {
            // Hovering with mouse already down (started elsewhere) — show cursor but don't drag.
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }
    }

    fn drag_h_splitter(
        &mut self,
        ui: &egui::Ui,
        rect: egui::Rect,
        idx: usize,
        pointer: Option<egui::Pos2>,
        primary_down: bool,
        hit_w: f32,
    ) {
        let Some(pos) = self.splitter_positions.get(idx).copied() else {
            return;
        };
        let Some(p) = pointer else {
            if !primary_down {
                if self.dragging_splitter == Some((SplitterAxis::Horizontal, idx)) {
                    self.dragging_splitter = None;
                }
            }
            return;
        };
        let split_y = rect.top() + rect.height() * pos;
        let hit = egui::Rect::from_min_max(
            egui::pos2(rect.left(), split_y - hit_w / 2.0),
            egui::pos2(rect.right(), split_y + hit_w / 2.0),
        );

        if !primary_down {
            if self.dragging_splitter == Some((SplitterAxis::Horizontal, idx)) {
                self.dragging_splitter = None;
            }
            if hit.contains(p) {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
            }
            return;
        }

        if self.dragging_splitter.is_none() && hit.contains(p) {
            self.dragging_splitter = Some((SplitterAxis::Horizontal, idx));
        }
        if self.dragging_splitter == Some((SplitterAxis::Horizontal, idx)) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
            let new_pos = ((p.y - rect.top()) / rect.height()).clamp(0.1, 0.9);
            self.splitter_positions[idx] = new_pos;
        } else if hit.contains(p) {
            // Hovering with mouse already down (started elsewhere) — show cursor but don't drag.
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        }
    }

    /// Calculate rect positions for each view based on current layout
    fn calculate_view_rects(&self, available_rect: egui::Rect) -> Vec<egui::Rect> {
        match self.layout {
            GridLayout::Single => vec![available_rect],
            GridLayout::Horizontal2 => self.split_horizontal_2(available_rect),
            GridLayout::Vertical2 => self.split_vertical_2(available_rect),
            GridLayout::Grid2x2 => self.split_grid_2x2(available_rect),
            GridLayout::Grid1x3 => self.split_horizontal_3(available_rect),
            GridLayout::Grid3x1 => self.split_vertical_3(available_rect),
        }
    }

    /// Split rect into 2 horizontal panels
    fn split_horizontal_2(&self, rect: egui::Rect) -> Vec<egui::Rect> {
        let split_pos = self.splitter_positions.get(0).copied().unwrap_or(0.5);
        let gap = 4.0;
        let split_x = rect.left() + rect.width() * split_pos;

        vec![
            egui::Rect::from_min_max(rect.min, egui::pos2(split_x - gap / 2.0, rect.max.y)),
            egui::Rect::from_min_max(egui::pos2(split_x + gap / 2.0, rect.min.y), rect.max),
        ]
    }

    /// Split rect into 2 vertical panels
    fn split_vertical_2(&self, rect: egui::Rect) -> Vec<egui::Rect> {
        let split_pos = self.splitter_positions.get(0).copied().unwrap_or(0.5);
        let gap = 4.0;
        let split_y = rect.top() + rect.height() * split_pos;

        vec![
            egui::Rect::from_min_max(rect.min, egui::pos2(rect.max.x, split_y - gap / 2.0)),
            egui::Rect::from_min_max(egui::pos2(rect.min.x, split_y + gap / 2.0), rect.max),
        ]
    }

    /// Split rect into 2x2 grid
    fn split_grid_2x2(&self, rect: egui::Rect) -> Vec<egui::Rect> {
        let gap = 4.0;
        let x_split_pos = self.splitter_positions.get(0).copied().unwrap_or(0.5);
        let y_split_pos = self.splitter_positions.get(1).copied().unwrap_or(0.5);
        let mid_x = rect.left() + rect.width() * x_split_pos;
        let mid_y = rect.top() + rect.height() * y_split_pos;

        vec![
            egui::Rect::from_min_max(rect.min, egui::pos2(mid_x - gap / 2.0, mid_y - gap / 2.0)),
            egui::Rect::from_min_max(
                egui::pos2(mid_x + gap / 2.0, rect.min.y),
                egui::pos2(rect.max.x, mid_y - gap / 2.0),
            ),
            egui::Rect::from_min_max(
                egui::pos2(rect.min.x, mid_y + gap / 2.0),
                egui::pos2(mid_x - gap / 2.0, rect.max.y),
            ),
            egui::Rect::from_min_max(egui::pos2(mid_x + gap / 2.0, mid_y + gap / 2.0), rect.max),
        ]
    }

    /// Split rect into 3 horizontal panels
    fn split_horizontal_3(&self, rect: egui::Rect) -> Vec<egui::Rect> {
        let gap = 4.0;
        let p1 = self.splitter_positions.get(0).copied().unwrap_or(1.0 / 3.0);
        let p2 = self.splitter_positions.get(1).copied().unwrap_or(2.0 / 3.0);
        let x1 = rect.left() + rect.width() * p1;
        let x2 = rect.left() + rect.width() * p2;
        vec![
            egui::Rect::from_min_max(rect.min, egui::pos2(x1 - gap / 2.0, rect.max.y)),
            egui::Rect::from_min_max(
                egui::pos2(x1 + gap / 2.0, rect.min.y),
                egui::pos2(x2 - gap / 2.0, rect.max.y),
            ),
            egui::Rect::from_min_max(egui::pos2(x2 + gap / 2.0, rect.min.y), rect.max),
        ]
    }

    /// Split rect into 3 vertical panels
    fn split_vertical_3(&self, rect: egui::Rect) -> Vec<egui::Rect> {
        let gap = 4.0;
        let p1 = self.splitter_positions.get(0).copied().unwrap_or(1.0 / 3.0);
        let p2 = self.splitter_positions.get(1).copied().unwrap_or(2.0 / 3.0);
        let y1 = rect.top() + rect.height() * p1;
        let y2 = rect.top() + rect.height() * p2;
        vec![
            egui::Rect::from_min_max(rect.min, egui::pos2(rect.max.x, y1 - gap / 2.0)),
            egui::Rect::from_min_max(
                egui::pos2(rect.min.x, y1 + gap / 2.0),
                egui::pos2(rect.max.x, y2 - gap / 2.0),
            ),
            egui::Rect::from_min_max(egui::pos2(rect.min.x, y2 + gap / 2.0), rect.max),
        ]
    }

    /// Render the inline `Grids:` + `Sync:` toolbar row at the top of the multi-
    /// chart area. Lets the user pick a grid layout and toggle the four sync
    /// flags. Called from `show()` before the panes are laid out.
    /// Apply a sync-flag change pushed up by a pane's inline grid bar.
    /// Off → On for Indicators rehydrates siblings to match the active pane.
    fn apply_sync_flags(&mut self, new_flags: SyncFlags) {
        let prev = self.sync_flags;
        self.sync_flags = new_flags;
        if !prev.indicators && new_flags.indicators {
            self.rehydrate_indicators();
        }
    }

    /// Main rendering function
    pub fn show(&mut self, ui: &mut egui::Ui) {
        // Check if any view is maximized
        if let Some(max_view) = self.views.iter().find(|v| v.maximized) {
            let rect = ui.available_rect_before_wrap();
            self.show_single_view(ui, max_view.id, rect);
            return;
        }

        // Tell every pane which layout / sync flags are active so the shared
        // grid bar reflects the current state. Also suppress per-pane input
        // while a splitter is being dragged — the splitter hit zone overlaps
        // the chart's interact rect by 1 px, so without this any vertical
        // jitter during a resize gets routed into the chart's pan handler and
        // silently flips `auto_scale_y` off. Per-pane toolbars are suppressed
        // because the active pane's toolbar is hoisted above the layout.
        let current_layout = self.layout;
        let current_sync = self.sync_flags;
        let resizing = self.dragging_splitter.is_some();
        for view in &mut self.views {
            view.chart.set_active_grid_layout(current_layout);
            view.chart.set_active_sync_flags(current_sync);
            view.chart.set_input_suppressed(resizing);
            view.chart.set_suppress_toolbar(true);
        }

        // Hoist the active pane's toolbar above the layout. After it renders,
        // copy the toolbar's UI state (open inline bar, current tool) onto all
        // other panes so when the cursor moves and `active_view` switches mid-
        // frame, the next frame's active pane starts from the same UI state
        // instead of dropping whichever inline bar the user just opened.
        let active_id = self.active_view.unwrap_or(0);
        if let Some(active_idx) = self.views.iter().position(|v| v.id == active_id) {
            self.views[active_idx].chart.show_toolbar(ui);
            let shared = self.views[active_idx].chart.toolbar_ui_state();
            for (i, view) in self.views.iter_mut().enumerate() {
                if i != active_idx {
                    view.chart.set_toolbar_ui_state(shared);
                }
            }
        }

        // Calculate layout from whatever space is left after the toolbar.
        let available = ui.available_rect_before_wrap();
        self.handle_splitter_drag(ui, available);
        let view_rects = self.calculate_view_rects(available);

        // Update active view based on cursor position
        self.update_active_view(ui, &view_rects);

        // Render each view
        for (i, rect) in view_rects.iter().enumerate() {
            if i < self.views.len() {
                self.show_single_view(ui, i, *rect);
            }
        }

        // Drain grid layout / sync requests from any pane the user clicked
        // through. First non-Single layout request wins; Single is treated as
        // a demote and bubbles up to `MyApp` via `take_grid_layout_request`.
        let mut pending_layout: Option<GridLayout> = None;
        let mut pending_sync: Option<SyncFlags> = None;
        for view in &mut self.views {
            if let Some(req) = view.chart.take_grid_layout_request() {
                pending_layout.get_or_insert(req);
            }
            if let Some(req) = view.chart.take_sync_flags_request() {
                pending_sync = Some(req);
            }
        }
        if let Some(flags) = pending_sync {
            self.apply_sync_flags(flags);
        }
        if let Some(layout) = pending_layout {
            if !matches!(layout, GridLayout::Single) {
                self.set_layout(layout);
            } else {
                // Stash so MyApp can collapse Multi → Single.
                self.pending_grid_layout = Some(layout);
            }
        }

        self.run_sync();
    }

    /// Render a single view
    fn show_single_view(&mut self, ui: &mut egui::Ui, view_id: usize, rect: egui::Rect) {
        let is_active = self.active_view == Some(view_id);

        // Draw border (highlight if active)
        let border_color = if is_active {
            egui::Color32::from_rgb(78, 205, 196) // ACCENT
        } else {
            egui::Color32::from_rgb(30, 30, 33) // BORDER
        };

        ui.painter().rect_stroke(
            rect,
            0.0,
            egui::Stroke::new(1.0, border_color),
            egui::StrokeKind::Inside,
        );

        // Allocate UI space for this view
        let inner_rect = rect.shrink(2.0);

        // Show view controls overlay
        self.show_view_controls(ui, view_id, inner_rect);

        // Show the chart. Wrap in push_id so multiple panes don't collide on
        // internal widget IDs (egui paints red overlays on duplicate IDs).
        ui.scope_builder(egui::UiBuilder::new().max_rect(inner_rect), |ui| {
            ui.push_id(("multi_chart_pane", view_id), |ui| {
                if let Some(view) = self.views.get_mut(view_id) {
                    view.chart.show(ui);
                }
            });
        });
    }

    /// Per-pane corner controls. Currently just a maximize toggle in the
    /// top-right; the pane's own inner `ChartWidget` toolbar handles timeframe
    /// selection, so no duplicate combo here.
    fn show_view_controls(&mut self, ui: &mut egui::Ui, view_id: usize, rect: egui::Rect) {
        let Some(view) = self.views.get(view_id) else {
            return;
        };
        let max_rect = egui::Rect::from_min_size(
            egui::pos2(rect.right() - 32.0, rect.top() + 8.0),
            egui::vec2(24.0, 24.0),
        );
        let icon = if view.maximized { "⊟" } else { "⊞" };
        let mut maximize_clicked = false;
        ui.scope_builder(egui::UiBuilder::new().max_rect(max_rect), |ui| {
            if ui.button(icon).clicked() {
                maximize_clicked = true;
            }
        });
        if maximize_clicked {
            self.toggle_maximize(view_id);
        }
    }

    /// Update which view is currently active based on cursor position
    fn update_active_view(&mut self, ui: &egui::Ui, view_rects: &[egui::Rect]) {
        if let Some(pos) = ui.input(|i| i.pointer.latest_pos()) {
            for (i, rect) in view_rects.iter().enumerate() {
                if rect.contains(pos) {
                    self.active_view = Some(i);
                    return;
                }
            }
        }
    }

    /// Run all enabled sync passes. Called from `show()` after every pane has
    /// rendered so each pane has up-to-date camera state. Change events are
    /// drained once per frame so multiple sync passes can read them without
    /// one consuming what the other needs.
    fn run_sync(&mut self) {
        let Some(active_id) = self.active_view else {
            return;
        };
        let Some(active_idx) = self.views.iter().position(|v| v.id == active_id) else {
            return;
        };

        // Drain change events once per frame so multiple sync passes can read them.
        let active_events = self.views[active_idx].chart.take_change_events();

        if self.sync_flags.time {
            self.sync_time(active_idx);
        }
        if self.sync_flags.crosshair {
            self.sync_crosshair(active_idx);
        } else {
            for view in &mut self.views {
                view.chart.set_ghost_cursor(None);
            }
        }
        if self.sync_flags.indicators {
            self.sync_indicators_from(active_idx, &active_events.indicator_events);
        }
        if self.sync_flags.symbol {
            self.sync_symbol_from(active_idx, &active_events.symbol_changed);
        }
    }

    fn sync_crosshair(&mut self, active_idx: usize) {
        let cursor_date = self.views[active_idx].chart.cursor_date();
        for (i, view) in self.views.iter_mut().enumerate() {
            if i == active_idx {
                // Active pane shows its own real crosshair.
                view.chart.set_ghost_cursor(None);
            } else {
                view.chart.set_ghost_cursor(cursor_date.clone());
            }
        }
    }

    fn sync_time(&mut self, active_idx: usize) {
        let Some((start, end)) = self.views[active_idx].chart.visible_date_range() else {
            return;
        };
        for (i, view) in self.views.iter_mut().enumerate() {
            if i == active_idx {
                continue;
            }
            view.chart.apply_camera_for_date_range(&start, &end);
        }
    }

    fn sync_indicators_from(
        &mut self,
        active_idx: usize,
        events: &[super::indicators::IndicatorEvent],
    ) {
        if events.is_empty() {
            return;
        }
        for (i, view) in self.views.iter_mut().enumerate() {
            if i == active_idx {
                continue;
            }
            for ev in events {
                apply_indicator_event(&mut view.chart, ev);
            }
            let _ = view.chart.take_change_events();
        }
    }

    fn sync_symbol_from(&mut self, active_idx: usize, pending: &Option<CandleData>) {
        let Some(new_data) = pending.as_ref() else {
            return;
        };
        self.raw_data = Arc::new(new_data.clone());
        for (i, view) in self.views.iter_mut().enumerate() {
            if i == active_idx {
                continue;
            }
            view.chart.set_data(new_data.clone());
            let _ = view.chart.take_change_events();
        }
    }

    /// Drain the toolbar's multi-chart toggle request from the active pane.
    /// Returns true if the user clicked the multi-chart toggle this frame.
    pub fn take_multi_chart_toggle_request(&mut self) -> bool {
        let Some(active_id) = self.active_view else {
            return false;
        };
        let Some(view) = self.views.iter_mut().find(|v| v.id == active_id) else {
            return false;
        };
        view.chart.take_multi_chart_toggle_request()
    }

    /// Drain a pending demote-to-single request. Set when any pane's inline
    /// grid bar picked `Single`; consumed by `MyApp` to collapse the
    /// multi-chart back to a single-chart view.
    pub fn take_grid_layout_request(&mut self) -> Option<GridLayout> {
        self.pending_grid_layout.take()
    }

    /// Force-overwrite every non-active pane's indicators to match the active
    /// pane. Call this when Indicators-sync transitions off → on.
    pub fn rehydrate_indicators(&mut self) {
        let Some(active_id) = self.active_view else {
            return;
        };
        let Some(active_idx) = self.views.iter().position(|v| v.id == active_id) else {
            return;
        };

        let snapshot: Vec<(&'static str, super::indicators::ParamValues)> = self.views[active_idx]
            .chart
            .manager()
            .active
            .iter()
            .map(|a| (a.def_id, a.params.clone()))
            .collect();

        for (i, view) in self.views.iter_mut().enumerate() {
            if i == active_idx {
                continue;
            }
            let to_remove: Vec<u64> = view
                .chart
                .manager()
                .active
                .iter()
                .map(|a| a.instance_id)
                .collect();
            for id in to_remove {
                view.chart.manager_mut().remove(id);
            }
            for (def_id, params) in &snapshot {
                view.chart.manager_mut().add(def_id, params.clone());
            }
            let _ = view.chart.take_change_events();
        }
    }
}
