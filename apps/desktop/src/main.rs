mod components;
mod ui_components;

use components::{MainSidebar, MiniSidebar, TopHeader, WidgetsControl};
use eframe::egui;
use ui_components::widgets::charts::{CandleData, ChartWidget, JsonCandle};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize database
    println!("Initializing database...");
    let db = zaned_database::Database::init().await?;
    println!("✓ Database initialized at: {}", db.path().display());

    if let Some(version) = db.get_schema_version().await? {
        println!("✓ Schema version: {}", version);
    }

    // Get database pool for the app
    let db_pool = db.pool().clone();

    // Get tokio runtime handle for async operations
    let runtime_handle = tokio::runtime::Handle::current();

    // Initialize drawing defaults with database
    ui_components::widgets::charts::drawings::init_with_database(
        db_pool.clone(),
        runtime_handle.clone(),
    );

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_titlebar_shown(false)
            .with_title_shown(false)
            .with_fullsize_content_view(true),
        ..Default::default()
    };

    eframe::run_native(
        "Zaned",
        options,
        Box::new(move |cc| {
            let mut fonts = egui::FontDefinitions::default();
            egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
            cc.egui_ctx.set_fonts(fonts);
            let mut visuals = egui::Visuals::dark();
            let bg = egui::Color32::from_rgb(0, 0, 0);
            visuals.panel_fill = bg;
            visuals.window_fill = bg;
            visuals.faint_bg_color = bg;
            cc.egui_ctx.set_visuals(visuals);

            // Store database pool and runtime handle in the app
            Ok(Box::new(MyApp::new(db_pool, runtime_handle)))
        }),
    )?;

    Ok(())
}

struct MyApp {
    top_header: TopHeader,
    widgets_control: WidgetsControl,
    main_sidebar: MainSidebar,
    mini_sidebar: MiniSidebar,
    chart: Option<ChartWidget>,
    db_pool: sqlx::SqlitePool,
    runtime_handle: tokio::runtime::Handle,
}

impl MyApp {
    fn new(db_pool: sqlx::SqlitePool, runtime_handle: tokio::runtime::Handle) -> Self {
        let mut chart = None;
        let json_str = std::fs::read_to_string("AAPL.json").unwrap_or_default();
        if !json_str.is_empty() {
            if let Ok(candles) = serde_json::from_str::<Vec<JsonCandle>>(&json_str) {
                if !candles.is_empty() {
                    let data = CandleData::from_json(&candles);
                    chart = Some(ChartWidget::new(data));
                }
            }
        }

        Self {
            top_header: TopHeader::default(),
            widgets_control: WidgetsControl::default(),
            main_sidebar: MainSidebar::default(),
            mini_sidebar: MiniSidebar::default(),
            chart,
            db_pool,
            runtime_handle,
        }
    }
}

impl eframe::App for MyApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.top_header.show(ui);

        let bg = egui::Color32::from_rgb(0, 0, 0);

        // Kill the panel separator line globally before creating panels
        ui.style_mut().visuals.widgets.noninteractive.bg_stroke = egui::Stroke::NONE;

        // Watchlist sidebar on the left
        let sidebar_bg = egui::Color32::from_rgb(0, 0, 0);
        egui::Panel::left("main_sidebar")
            .default_size(220.0)
            .size_range(220.0..=400.0)
            .resizable(true)
            .frame(
                egui::Frame::new()
                    .fill(sidebar_bg)
                    .stroke(egui::Stroke::NONE),
            )
            .show_inside(ui, |ui| {
                self.main_sidebar.show(ui);
            });

        // Mini sidebar on the right
        egui::Panel::right("mini_sidebar")
            .exact_size(58.0)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(sidebar_bg)
                    .stroke(egui::Stroke::NONE),
            )
            .show_inside(ui, |ui| {
                self.mini_sidebar.show(ui);
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(bg))
            .show_inside(ui, |ui| {
                self.widgets_control.show(ui, self.chart.as_mut());
            });
    }
}
