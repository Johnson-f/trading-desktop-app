mod camera;
mod candle;
mod controls;
mod crosshair;
mod grid;
mod interaction;
mod pane;
mod renderer;

use std::sync::Arc;
use egui::mutex::Mutex;

pub use candle::{CandleData, JsonCandle};
use camera::Camera;
use controls::ChartToolbar;
use interaction::InteractionState;
use pane::SubPane;
use renderer::ChartCallback;

pub struct ChartWidget {
    data: Arc<CandleData>,
    camera: Arc<Mutex<Camera>>,
    interaction: InteractionState,
    initialized: Arc<Mutex<bool>>,
    volume_pane: SubPane,
    toolbar: ChartToolbar,
}

impl ChartWidget {
    pub fn new(data: CandleData) -> Self {
        let mut camera = Camera::default();
        let data = Arc::new(data);
        camera.fit_to_data(&data);

        Self {
            data,
            camera: Arc::new(Mutex::new(camera)),
            interaction: InteractionState::default(),
            initialized: Arc::new(Mutex::new(false)),
            volume_pane: SubPane::new(0.20),
            toolbar: ChartToolbar::default(),
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        // Toolbar at top
        self.toolbar.show(ui);

        let available = ui.available_size();
        let (total_rect, _response) = ui.allocate_exact_size(available, egui::Sense::hover());

        // Split into main chart, divider, and volume pane
        let (chart_rect, divider_rect, volume_rect) = self.volume_pane.split_rect(total_rect);

        {
            let mut camera = self.camera.lock();
            camera.viewport = chart_rect.size();
        }

        // Skip chart interactions when modal is open
        let modal_open = self.toolbar.indicator_modal.open;

        if !modal_open {
            let mut camera = self.camera.lock();
            self.interaction.handle_input(ui, total_rect, chart_rect, &mut camera, &self.data);
            camera.auto_scale_y(&self.data);
        }

        // Detect the actual surface texture format
        let target_format = egui_wgpu::preferred_framebuffer_format(
            &[wgpu::TextureFormat::Bgra8Unorm, wgpu::TextureFormat::Rgba8Unorm]
        ).unwrap_or(wgpu::TextureFormat::Bgra8Unorm);

        // Main chart (wgpu candles)
        let callback = ChartCallback {
            camera: self.camera.clone(),
            data: self.data.clone(),
            initialized: self.initialized.clone(),
            target_format,
        };
        ui.painter().add(egui_wgpu::Callback::new_paint_callback(chart_rect, callback));

        // Price axis drag (manual Y scale control)
        if !modal_open {
            let mut camera = self.camera.lock();
            grid::handle_price_axis_drag(ui, chart_rect, &mut camera);
        }

        // Grid + price scale (painted after candles, before crosshair)
        {
            let mut camera = self.camera.lock();
            grid::paint_price_grid(ui, chart_rect, &camera, &self.data);
            grid::paint_time_grid(ui, chart_rect, &camera, &self.data);
            if !modal_open {
                grid::paint_auto_button(ui, chart_rect, &mut camera);
            }
        }

        // Divider (resizable)
        if !modal_open {
            self.volume_pane.handle_divider_drag(ui, divider_rect, total_rect, "volume_divider");
        }
        self.volume_pane.paint_divider(ui, divider_rect);

        // Volume pane
        {
            let camera = self.camera.lock();
            pane::volume::paint_volume(ui, volume_rect, &camera, &self.data);
        }

        // Crosshair across both panes (painted LAST so it's on top of everything)
        {
            let camera = self.camera.lock();
            crosshair::paint_crosshair(ui, chart_rect, volume_rect, &camera, &self.data);
        }
    }
}
