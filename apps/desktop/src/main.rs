mod auth;
mod components;
mod ui_components;

use components::{MainSidebar, MiniSidebar, TopHeader, WidgetsControl};
use eframe::egui;
use ui_components::widgets::charts::{CandleData, ChartWidget, JsonCandle};
use ui_components::widgets::charts::multi_charts::{self, MultiChartWidget};

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

    // Auth bootstrap: load Clerk env, build the shared state, attempt to
    // restore a session from the keychain, and spawn the refresh worker.
    // The OAuth flow itself runs lazily when the user clicks Sign in.
    // Try the crate's own .env first (apps/desktop/.env), then fall back to
    // any ambient .env in the current working directory.
    let crate_env = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".env");
    if dotenvy::from_path(&crate_env).is_err() {
        let _ = dotenvy::dotenv();
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "zaned=info,Zaned=info".into()),
        )
        .init();

    let clerk_cfg = auth::config::ClerkConfig::from_env()?;
    let auth_state = auth::AuthStateHandle::new(auth::AuthState::Unauthenticated);

    if let Some(refresh) = auth::storage::load_refresh_token()? {
        let cfg = clerk_cfg.clone();
        let state = auth_state.clone();
        runtime_handle.spawn(async move {
            match auth::flow::refresh(&cfg, &refresh).await {
                Ok(set) => {
                    tracing::info!("restored session from keychain");
                    state.set(auth::AuthState::from(set)).await;
                }
                Err(e) => {
                    tracing::warn!(error = %e, "keychain refresh failed; user must re-sign-in");
                    let _ = auth::storage::delete_refresh_token();
                    state.set(auth::AuthState::Unauthenticated).await;
                }
            }
        });
    }

    auth::refresh::spawn(clerk_cfg.clone(), auth_state.clone());

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

            Ok(Box::new(MyApp::new(auth_state)))
        }),
    )?;

    Ok(())
}

/// Either the single chart (default) or the multi-chart wrapper. Toggled via
/// the chart-toolbar multi-chart button. State migrates between variants:
/// Single → Multi takes the existing chart as pane 0; Multi → Single extracts
/// the active pane.
pub enum ChartView {
    Single(ChartWidget),
    Multi(MultiChartWidget),
}

impl ChartView {
    pub fn show(&mut self, ui: &mut egui::Ui) {
        match self {
            ChartView::Single(c) => c.show(ui),
            ChartView::Multi(m) => m.show(ui),
        }
    }

    pub fn is_multi(&self) -> bool {
        matches!(self, ChartView::Multi(_))
    }
}

struct MyApp {
    top_header: TopHeader,
    widgets_control: WidgetsControl,
    main_sidebar: MainSidebar,
    mini_sidebar: MiniSidebar,
    chart: Option<ChartView>,
    /// When set, user clicked the multi-chart toggle while in Multi mode AND
    /// other panes have user content. We show a confirmation modal before
    /// dropping their work.
    pending_collapse_confirm: bool,
    auth_state: auth::AuthStateHandle,
}

impl MyApp {
    fn new(auth_state: auth::AuthStateHandle) -> Self {
        let mut chart = None;
        let json_str = std::fs::read_to_string("AAPL.json").unwrap_or_default();
        if !json_str.is_empty() {
            if let Ok(candles) = serde_json::from_str::<Vec<JsonCandle>>(&json_str) {
                if !candles.is_empty() {
                    let data = CandleData::from_json(&candles);
                    let mut widget = ChartWidget::new(data);
                    // Bind the boot symbol so persisted drawings load on launch.
                    widget.set_symbol("AAPL".to_string());
                    chart = Some(ChartView::Single(widget));
                }
            }
        }

        Self {
            top_header: TopHeader::default(),
            widgets_control: WidgetsControl::default(),
            main_sidebar: MainSidebar::default(),
            mini_sidebar: MiniSidebar::default(),
            chart,
            pending_collapse_confirm: false,
            auth_state,
        }
    }
}

impl eframe::App for MyApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // If unauthenticated, render the login screen and skip the rest.
        if !matches!(
            self.auth_state.blocking_snapshot(),
            crate::auth::AuthState::Authenticated { .. }
        ) {
            let cfg = std::env::var("CLERK_ISSUER")
                .and_then(|_| std::env::var("CLERK_CLIENT_ID"))
                .ok()
                .and_then(|_| crate::auth::config::ClerkConfig::from_env().ok());
            if let Some(cfg) = cfg {
                let runtime = tokio::runtime::Handle::current();
                let screen = crate::auth::screen::LoginScreen::new(
                    cfg,
                    self.auth_state.clone(),
                    runtime,
                );
                screen.show(ui.ctx());
            } else {
                #[allow(deprecated)]
                egui::CentralPanel::default().show(ui.ctx(), |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(120.0);
                        ui.heading("Auth misconfigured");
                        ui.label("Set CLERK_ISSUER and CLERK_CLIENT_ID in apps/desktop/.env and restart.");
                    });
                });
            }
            return;
        }

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

        if self.pending_collapse_confirm {
            egui::Window::new("Collapse multi-chart?")
                .collapsible(false)
                .resizable(false)
                .show(ui.ctx(), |ui| {
                    ui.label(
                        "Collapsing back to a single chart will discard drawings and \
                         indicators on inactive panes. Continue?",
                    );
                    ui.horizontal(|ui| {
                        if ui.button("Discard and collapse").clicked() {
                            if let Some(ChartView::Multi(multi)) = self.chart.take() {
                                self.chart = Some(ChartView::Single(multi.into_active_chart()));
                            }
                            self.pending_collapse_confirm = false;
                        }
                        if ui.button("Cancel").clicked() {
                            self.pending_collapse_confirm = false;
                        }
                    });
                });
        }

        if !self.pending_collapse_confirm {
            if let Some(view) = self.chart.as_mut() {
                let layout_req = match view {
                    ChartView::Single(c) => c.take_grid_layout_request(),
                    ChartView::Multi(m) => m.take_grid_layout_request(),
                };
                if let Some(layout) = layout_req {
                    let owned = self.chart.take().expect("chart is Some by outer condition");
                    let next = match (owned, layout) {
                        // Single → non-Single: promote and apply the picked layout.
                        (ChartView::Single(chart), layout)
                            if !matches!(layout, multi_charts::GridLayout::Single) =>
                        {
                            let raw_data = chart.raw_data_arc();
                            let symbol = chart.symbol().unwrap_or_else(|| "AAPL".to_string());
                            let mut multi = MultiChartWidget::from_chart(symbol, raw_data, chart);
                            multi.set_layout(layout);
                            ChartView::Multi(multi)
                        }
                        // Single → Single: no-op.
                        (ChartView::Single(chart), _) => ChartView::Single(chart),
                        // Multi → Single: collapse, with confirm prompt if other panes have content.
                        (ChartView::Multi(multi), multi_charts::GridLayout::Single) => {
                            if multi.inactive_panes_have_user_content() {
                                self.pending_collapse_confirm = true;
                                ChartView::Multi(multi)
                            } else {
                                ChartView::Single(multi.into_active_chart())
                            }
                        }
                        // Multi → other layouts are applied internally by MultiChartWidget.
                        (ChartView::Multi(multi), _) => ChartView::Multi(multi),
                    };
                    self.chart = Some(next);
                }
            }
        }
    }
}
