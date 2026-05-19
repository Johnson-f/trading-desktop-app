mod api;
mod auth;
mod components;

use components::{MainSidebar, MiniSidebar, TopHeader, WidgetsControl};
use eframe::egui;
use zaned_chart_widget::multi_charts::{self, MultiChartWidget};
use zaned_chart_widget::{CandleData, ChartWidget};

use crate::auth::{AuthState, AuthStateHandle};

/// Adapter over `AuthStateHandle` that satisfies `chart_widget::BearerProvider`.
/// The chart-widget crate doesn't know how the desktop app authenticates; we
/// snapshot the auth state on each fetch and forward the access token (or a
/// stringly-typed error if the user isn't signed in).
struct AuthBearerProvider {
    state: AuthStateHandle,
}

impl AuthBearerProvider {
    fn new(state: AuthStateHandle) -> Self {
        Self { state }
    }
}

impl zaned_chart_widget::BearerProvider for AuthBearerProvider {
    async fn bearer(&self) -> Result<String, String> {
        match self.state.snapshot().await {
            AuthState::Authenticated { access_token, .. } => Ok(access_token),
            other => Err(format!("not authenticated: {other:?}")),
        }
    }
}

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

    // If the keychain has a refresh token, start in `Loading` state and
    // kick off the background swap. Otherwise we're cold — start in
    // `Unauthenticated` so the login screen renders immediately.
    //
    // Without this, there's a ~50-500ms window during cold start where
    // the UI shows the login screen, the user clicks "Sign In" before
    // the keychain refresh finishes, and an unwanted OAuth flow opens in
    // the browser. The keychain restore wins the race silently and the
    // abandoned browser auth-URL eventually times out 5 minutes later.
    let stored_refresh = auth::storage::load_refresh_token()?;
    let initial_state = if stored_refresh.is_some() {
        auth::AuthState::Loading
    } else {
        auth::AuthState::Unauthenticated
    };
    let auth_state = auth::AuthStateHandle::new(initial_state);

    if let Some(refresh) = stored_refresh {
        let cfg = clerk_cfg.clone();
        let state = auth_state.clone();
        runtime_handle.spawn(async move {
            match auth::flow::refresh(&cfg, &refresh).await {
                Ok(set) => {
                    tracing::info!("restored session from keychain");
                    state.set(auth::AuthState::from(set)).await;
                }
                Err(e) => {
                    tracing::warn!(error = ?e, "keychain refresh failed; user must re-sign-in");
                    // Don't delete the refresh token on transient errors —
                    // a network blip or temporary Clerk 5xx shouldn't force a
                    // full re-sign-in. The token is only confirmed-bad if the
                    // server returned an explicit auth failure (400 / 401 / 403),
                    // mirroring the policy already in `auth/refresh.rs`.
                    let msg = e.to_string();
                    if msg.contains("400") || msg.contains("401") || msg.contains("403") {
                        let _ = auth::storage::delete_refresh_token();
                    }
                    state.set(auth::AuthState::Unauthenticated).await;
                }
            }
        });
    }

    auth::refresh::spawn(clerk_cfg.clone(), auth_state.clone());

    // Initialize drawing defaults with database
    zaned_chart_widget::drawings::init_with_database(db_pool.clone(), runtime_handle.clone());

    // Wire the candle loader so symbol changes can fetch from the gateway.
    api::symbol_search::init(runtime_handle.clone(), auth_state.clone());

    // Wire the chart-widget crate's loaders. The crate is auth- and
    // server-agnostic; we hand it a runtime handle, the gateway URL, and
    // a `BearerProvider` over the desktop app's `AuthStateHandle`.
    zaned_chart_widget::init(zaned_chart_widget::Config {
        runtime: runtime_handle.clone(),
        server_url: api::client::SERVER_URL.to_string(),
        auth: std::sync::Arc::new(AuthBearerProvider::new(auth_state.clone())),
    });

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

            // JetBrains Mono — used for tabular figures in chart price
            // labels, axis ticks, and OHLC overlays. Registered as the
            // primary `Monospace` family so `FontId::monospace(...)`
            // resolves to it everywhere.
            fonts.font_data.insert(
                "JetBrainsMono".to_owned(),
                egui::FontData::from_static(include_bytes!(
                    "../assets/fonts/JetBrainsMono-Regular.ttf"
                ))
                .into(),
            );
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .insert(0, "JetBrainsMono".to_owned());

            cc.egui_ctx.set_fonts(fonts);

            // Wire image loaders so `egui::Image::from_uri(...)` can fetch
            // remote PNGs (Parqet stock logos, etc.) — see `crate::api::logo`.
            // Without this call, image URIs are inert and render as nothing.
            egui_extras::install_image_loaders(&cc.egui_ctx);

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
    /// Flips on the first frame we see `Authenticated`. Used to defer the
    /// boot symbol's data fetch until the bearer is available — calling
    /// `set_symbol` earlier would dispatch an unauth'd request that the
    /// gateway would reject.
    boot_symbol_loaded: bool,
}

impl MyApp {
    fn new(auth_state: auth::AuthStateHandle) -> Self {
        // Start with an empty chart; the boot symbol's candles are fetched
        // from the gateway on the first authenticated frame (see `ui()`).
        let widget = ChartWidget::new(CandleData::from_json(&[]));
        let chart = Some(ChartView::Single(widget));

        Self {
            top_header: TopHeader::default(),
            widgets_control: WidgetsControl::default(),
            main_sidebar: MainSidebar::default(),
            mini_sidebar: MiniSidebar::default(),
            chart,
            pending_collapse_confirm: false,
            auth_state,
            boot_symbol_loaded: false,
        }
    }
}

impl eframe::App for MyApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Route based on auth state before rendering the main UI.
        match self.auth_state.blocking_snapshot() {
            crate::auth::AuthState::Authenticated { .. } => {
                // First authenticated frame — bind the boot symbol so the
                // chart fetches AAPL's history (and persisted drawings) now
                // that we have a bearer.
                if !self.boot_symbol_loaded {
                    if let Some(ChartView::Single(c)) = self.chart.as_mut() {
                        c.set_symbol("AAPL".to_string());
                    }
                    self.boot_symbol_loaded = true;
                }
                // Fall through to the main UI below.
            }
            crate::auth::AuthState::Loading => {
                // Keychain restore is in-flight. Render a non-interactive
                // placeholder so the user can't click "Sign In" during this
                // window and trigger an unwanted OAuth flow.
                crate::auth::screen::LoadingScreen::default().show(ui.ctx());
                return;
            }
            _ => {
                // Unauthenticated or Failed — render the login screen.
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
        }

        if let Some(picked) = self.top_header.show(ui) {
            // The user selected a symbol from the search dropdown.
            // Forward it to the active single chart, which kicks off the
            // async candle load for the new symbol. The Multi layout's
            // `set_symbol` still uses the synchronous data-injection
            // signature; wiring it through the async loader is its own
            // change so we ignore picks while in Multi mode for now.
            match self.chart.as_mut() {
                Some(ChartView::Single(c)) => c.set_symbol(picked),
                Some(ChartView::Multi(_)) => {
                    tracing::info!(
                        "symbol picked while in multi-chart layout; ignoring \
                         until multi_charts is wired through the async loader",
                    );
                }
                None => {}
            }
        }

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
