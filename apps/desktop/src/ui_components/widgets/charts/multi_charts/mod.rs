use super::{CandleData, ChartWidget, Timeframe};
use egui::mutex::Mutex;
use std::sync::Arc;

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

/// Synchronization mode between chart views
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncMode {
    /// No synchronization - each chart is independent
    None,
    /// Sync crosshair position only
    Crosshair,
    /// Sync time axis (scrolling/zooming)
    TimeAxis,
    /// Sync both crosshair and time axis
    Both,
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
    /// Synchronization mode
    sync_mode: SyncMode,
    /// Which view is currently active/focused
    active_view: Option<usize>,
    /// Layout selector modal state
    layout_selector_open: bool,
    /// Splitter positions for resizable layouts (0.0 to 1.0)
    splitter_positions: Vec<f32>,
}


impl MultiChartWidget {
    /// Create a new multi-chart widget with a single initial view
    pub fn new(symbol: String, data: CandleData) -> Self {
        let raw_data = Arc::new(data);
        let mut views = Vec::new();

        // Start with a single daily view
        views.push(TimeframeView {
            id: 0,
            timeframe: Timeframe::Daily,
            chart: ChartWidget::new((*raw_data).clone()),
            maximized: false,
        });

        Self {
            symbol,
            raw_data,
            views,
            layout: GridLayout::Single,
            sync_mode: SyncMode::Both,
            active_view: Some(0),
            layout_selector_open: false,
            splitter_positions: vec![0.5],
        }
    }

    /// Get the current symbol
    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    /// Get the current layout
    pub fn layout(&self) -> GridLayout {
        self.layout
    }

    /// Get the current sync mode
    pub fn sync_mode(&self) -> SyncMode {
        self.sync_mode
    }

    /// Set the sync mode
    pub fn set_sync_mode(&mut self, mode: SyncMode) {
        self.sync_mode = mode;
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

    /// Change the timeframe of an existing view
    pub fn change_timeframe(&mut self, view_id: usize, timeframe: Timeframe) {
        if let Some(view) = self.views.iter_mut().find(|v| v.id == view_id) {
            view.timeframe = timeframe;
            let aggregated_data = self.raw_data.aggregated(timeframe);
            view.chart = ChartWidget::new(aggregated_data);
        }
    }

    /// Update the symbol for all views
    pub fn set_symbol(&mut self, symbol: String, data: CandleData) {
        self.symbol = symbol;
        self.raw_data = Arc::new(data);

        // Recreate all views with new data
        for view in &mut self.views {
            let aggregated_data = self.raw_data.aggregated(view.timeframe);
            view.chart = ChartWidget::new(aggregated_data);
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

        // Add views if needed
        while self.views.len() < required_views {
            let next_timeframe = self.suggest_next_timeframe();
            self.add_view(next_timeframe);
        }

        self.layout = layout;
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
            egui::Rect::from_min_max(
                rect.min,
                egui::pos2(split_x - gap / 2.0, rect.max.y),
            ),
            egui::Rect::from_min_max(
                egui::pos2(split_x + gap / 2.0, rect.min.y),
                rect.max,
            ),
        ]
    }

    /// Split rect into 2 vertical panels
    fn split_vertical_2(&self, rect: egui::Rect) -> Vec<egui::Rect> {
        let split_pos = self.splitter_positions.get(0).copied().unwrap_or(0.5);
        let gap = 4.0;
        let split_y = rect.top() + rect.height() * split_pos;

        vec![
            egui::Rect::from_min_max(
                rect.min,
                egui::pos2(rect.max.x, split_y - gap / 2.0),
            ),
            egui::Rect::from_min_max(
                egui::pos2(rect.min.x, split_y + gap / 2.0),
                rect.max,
            ),
        ]
    }

    /// Split rect into 2x2 grid
    fn split_grid_2x2(&self, rect: egui::Rect) -> Vec<egui::Rect> {
        let gap = 4.0;
        let mid_x = rect.left() + rect.width() / 2.0;
        let mid_y = rect.top() + rect.height() / 2.0;

        vec![
            // Top-left
            egui::Rect::from_min_max(
                rect.min,
                egui::pos2(mid_x - gap / 2.0, mid_y - gap / 2.0),
            ),
            // Top-right
            egui::Rect::from_min_max(
                egui::pos2(mid_x + gap / 2.0, rect.min.y),
                egui::pos2(rect.max.x, mid_y - gap / 2.0),
            ),
            // Bottom-left
            egui::Rect::from_min_max(
                egui::pos2(rect.min.x, mid_y + gap / 2.0),
                egui::pos2(mid_x - gap / 2.0, rect.max.y),
            ),
            // Bottom-right
            egui::Rect::from_min_max(
                egui::pos2(mid_x + gap / 2.0, mid_y + gap / 2.0),
                rect.max,
            ),
        ]
    }

    /// Split rect into 3 horizontal panels
    fn split_horizontal_3(&self, rect: egui::Rect) -> Vec<egui::Rect> {
        let gap = 4.0;
        let width = (rect.width() - gap * 2.0) / 3.0;

        vec![
            egui::Rect::from_min_size(rect.min, egui::vec2(width, rect.height())),
            egui::Rect::from_min_size(
                egui::pos2(rect.left() + width + gap, rect.top()),
                egui::vec2(width, rect.height()),
            ),
            egui::Rect::from_min_size(
                egui::pos2(rect.left() + (width + gap) * 2.0, rect.top()),
                egui::vec2(width, rect.height()),
            ),
        ]
    }

    /// Split rect into 3 vertical panels
    fn split_vertical_3(&self, rect: egui::Rect) -> Vec<egui::Rect> {
        let gap = 4.0;
        let height = (rect.height() - gap * 2.0) / 3.0;

        vec![
            egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), height)),
            egui::Rect::from_min_size(
                egui::pos2(rect.left(), rect.top() + height + gap),
                egui::vec2(rect.width(), height),
            ),
            egui::Rect::from_min_size(
                egui::pos2(rect.left(), rect.top() + (height + gap) * 2.0),
                egui::vec2(rect.width(), height),
            ),
        ]
    }

    /// Main rendering function
    pub fn show(&mut self, ui: &mut egui::Ui) {
        // Check if any view is maximized
        if let Some(max_view) = self.views.iter().find(|v| v.maximized) {
            let rect = ui.available_rect_before_wrap();
            self.show_single_view(ui, max_view.id, rect);
            return;
        }

        // Calculate layout
        let available = ui.available_rect_before_wrap();
        let view_rects = self.calculate_view_rects(available);

        // Update active view based on cursor position
        self.update_active_view(ui, &view_rects);

        // Render each view
        for (i, rect) in view_rects.iter().enumerate() {
            if i < self.views.len() {
                self.show_single_view(ui, i, *rect);
            }
        }
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
        );

        // Allocate UI space for this view
        let inner_rect = rect.shrink(2.0);
        
        // Show view controls overlay
        self.show_view_controls(ui, view_id, inner_rect);

        // Show the chart
        ui.allocate_ui_at_rect(inner_rect, |ui| {
            if let Some(view) = self.views.get_mut(view_id) {
                view.chart.show(ui);
            }
        });
    }

    /// Show controls for a specific view (timeframe selector, maximize button)
    fn show_view_controls(&mut self, ui: &mut egui::Ui, view_id: usize, rect: egui::Rect) {
        let Some(view) = self.views.get(view_id) else {
            return;
        };

        // Timeframe selector (top-left corner)
        let tf_rect = egui::Rect::from_min_size(
            rect.min + egui::vec2(8.0, 8.0),
            egui::vec2(90.0, 24.0),
        );

        let mut timeframe_changed = None;
        ui.allocate_ui_at_rect(tf_rect, |ui| {
            egui::ComboBox::from_id_source(format!("tf_{}", view_id))
                .selected_text(format!("{:?}", view.timeframe))
                .show_ui(ui, |ui| {
                    for tf in [Timeframe::Daily, Timeframe::Weekly, Timeframe::Monthly] {
                        if ui
                            .selectable_label(view.timeframe == tf, format!("{:?}", tf))
                            .clicked()
                        {
                            timeframe_changed = Some(tf);
                        }
                    }
                });
        });

        if let Some(new_tf) = timeframe_changed {
            self.change_timeframe(view_id, new_tf);
        }

        // Maximize button (top-right corner)
        let max_rect = egui::Rect::from_min_size(
            egui::pos2(rect.right() - 32.0, rect.top() + 8.0),
            egui::vec2(24.0, 24.0),
        );

        ui.allocate_ui_at_rect(max_rect, |ui| {
            let icon = if view.maximized { "⊟" } else { "⊞" };
            if ui.button(icon).clicked() {
                self.toggle_maximize(view_id);
            }
        });
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
}
