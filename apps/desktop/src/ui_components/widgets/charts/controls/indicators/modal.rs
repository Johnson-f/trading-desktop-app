use eframe::egui::{self, Color32, CornerRadius, RichText, Stroke, Vec2};

const MODAL_BG: Color32 = Color32::from_rgb(30, 30, 34);
const BORDER: Color32 = Color32::from_rgb(50, 50, 55);
const TEXT_WHITE: Color32 = Color32::from_rgb(240, 240, 242);
const TEXT_MUTED: Color32 = Color32::from_rgb(120, 120, 130);
const ACCENT: Color32 = Color32::from_rgb(78, 205, 196);
const STAR_ACTIVE: Color32 = Color32::from_rgb(255, 193, 7);
const STAR_INACTIVE: Color32 = Color32::from_rgb(80, 80, 90);
const SIDEBAR_BG: Color32 = Color32::from_rgb(34, 34, 38);


const INDICATORS: &[(&str, u32, bool)] = &[
    ("EMA", 36759, false),
    ("VWAP", 30320, false),
    ("RSI", 26296, true),
    ("MACD", 23205, true),
    ("MA", 15008, false),
    ("VOL", 12293, true),
    ("Bollinger Bands", 8784, false),
    ("MACD with crossing signal", 8650, true),
    ("SuperTrend", 7594, false),
    ("VP", 7209, true),
    ("ATR", 6800, true),
    ("Stochastic", 5400, true),
    ("OBV", 4200, true),
    ("CCI", 3800, true),
    ("Williams %R", 3200, true),
    ("Ichimoku Cloud", 2900, false),
    ("Parabolic SAR", 2500, false),
    ("ADX", 2100, true),
    ("MFI", 1800, true),
    ("CMF", 1500, true),
];

pub struct IndicatorModal {
    pub open: bool,
    pub search_query: String,
    pub active_tab: usize, // 0 = Favorites, 1 = All Indicators, 2 = My Indicators
    pub added: Vec<bool>,
    pub favorited: Vec<bool>,
}

impl Default for IndicatorModal {
    fn default() -> Self {
        let count = INDICATORS.len();
        let mut added = vec![false; count];
        let mut favorited = vec![false; count];

        // EMA and VOL added + favorited by default
        if let Some(ema_idx) = INDICATORS.iter().position(|(n, _, _)| *n == "EMA") {
            added[ema_idx] = true;
            favorited[ema_idx] = true;
        }
        if let Some(vol_idx) = INDICATORS.iter().position(|(n, _, _)| *n == "VOL") {
            added[vol_idx] = true;
            favorited[vol_idx] = true;
        }

        Self {
            open: false,
            search_query: String::new(),
            active_tab: 1, // All Indicators by default
            added,
            favorited,
        }
    }
}

impl IndicatorModal {
    pub fn show(&mut self, ctx: &egui::Context) {
        if !self.open {
            return;
        }

        let screen_rect = ctx.content_rect();
        let modal_size = Vec2::new(700.0, 550.0);
        let center = screen_rect.center() - modal_size / 2.0;

        // Consume all input events so nothing reaches the chart
        ctx.input_mut(|input| {
            input.events.clear();
        });

        egui::Window::new("Indicators")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .fixed_size(modal_size)
            .default_pos(egui::Pos2::new(center.x, center.y))
            .frame(egui::Frame::new().fill(MODAL_BG).stroke(Stroke::new(1.0, BORDER)).corner_radius(CornerRadius::same(8)).inner_margin(egui::Margin::symmetric(16, 12)))
            .show(ctx, |ui| {
                ui.set_min_height(480.0);
                ui.style_mut().animation_time = 0.0;

                // Close button
                ui.horizontal(|ui| {
                    let close = ui.add(
                        egui::Button::new(RichText::new("●").color(Color32::from_rgb(255, 95, 87)).size(12.0))
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::NONE),
                    );
                    if close.clicked() {
                        self.open = false;
                    }

                    ui.centered_and_justified(|ui| {
                        ui.label(RichText::new("Indicators").color(TEXT_WHITE).size(14.0).strong());
                    });
                });

                ui.add_space(4.0);

                // Tab bar: Favorites | All Indicators | My Indicators | + New Indicator
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    let tabs = ["Favorites", "All Indicators", "My Indicators"];
                    for (i, tab) in tabs.iter().enumerate() {
                        let is_active = self.active_tab == i;
                        let color = if is_active { ACCENT } else { TEXT_MUTED };

                        let btn = ui.add(
                            egui::Button::new(RichText::new(*tab).size(11.0).color(color))
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::NONE),
                        );

                        if btn.clicked() {
                            self.active_tab = i;
                        }
                        if btn.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }

                        // Underline for active tab
                        if is_active {
                            let rect = btn.rect;
                            let underline = egui::Rect::from_min_size(
                                egui::Pos2::new(rect.left(), rect.bottom() - 2.0),
                                Vec2::new(rect.width(), 2.0),
                            );
                            ui.painter().rect_filled(underline, 0.0, ACCENT);
                        }
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new("＋ New Indicator").size(11.0).color(ACCENT));
                    });
                });

                ui.add_space(8.0);

                // Main content: left list + right sidebar
                let content_height = ui.available_height();
                ui.horizontal(|ui| {
                    ui.set_height(content_height);

                    // ── Left panel: search + list ──
                    ui.vertical(|ui| {
                        ui.set_width(ui.available_width() - 200.0);
                        ui.set_height(content_height);

                        // Search bar
                        ui.scope(|ui| {
                            let style = ui.style_mut();
                            style.visuals.extreme_bg_color = Color32::from_rgb(20, 20, 24);
                            style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(20, 20, 24);
                            style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
                            style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(20, 20, 24);
                            style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT);
                            style.visuals.widgets.active.bg_fill = Color32::from_rgb(20, 20, 24);
                            style.visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
                            style.visuals.selection.bg_fill = Color32::from_rgb(33, 33, 54);

                            ui.horizontal(|ui| {
                                ui.label(RichText::new(egui_phosphor::regular::MAGNIFYING_GLASS).size(14.0).color(TEXT_MUTED));
                                let te = egui::TextEdit::singleline(&mut self.search_query)
                                    .hint_text(RichText::new("Search").color(TEXT_MUTED))
                                    .desired_width(ui.available_width() - 30.0)
                                    .text_color(TEXT_WHITE);
                                ui.add(te);
                            });
                        });

                        ui.add_space(8.0);

                        // Column headers
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Indicator Name").size(10.0).color(TEXT_MUTED));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.add_space(60.0);
                                ui.label(RichText::new("Likes ▾").size(10.0).color(TEXT_MUTED));
                            });
                        });

                        ui.add_space(4.0);

                        // Indicator list
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            let query = self.search_query.to_lowercase();

                            for (idx, (name, likes, _is_sub)) in INDICATORS.iter().enumerate() {
                                if !query.is_empty() && !name.to_lowercase().contains(&query) {
                                    continue;
                                }

                                // Filter by tab
                                if self.active_tab == 0 && !self.favorited[idx] {
                                    continue;
                                }

                                let row_frame = egui::Frame::new()
                                    .fill(Color32::TRANSPARENT)
                                    .inner_margin(egui::Margin::symmetric(0, 4))
                                    .stroke(Stroke::NONE);

                                let row_resp = row_frame.show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.set_min_height(28.0);

                                    ui.horizontal(|ui| {
                                        // Name
                                        ui.label(RichText::new(*name).size(12.0).color(TEXT_WHITE));

                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            // Star
                                            let star_color = if self.favorited[idx] { STAR_ACTIVE } else { STAR_INACTIVE };
                                            let star_btn = ui.add(
                                                egui::Button::new(RichText::new(egui_phosphor::regular::STAR).size(14.0).color(star_color))
                                                    .fill(Color32::TRANSPARENT)
                                                    .stroke(Stroke::NONE),
                                            );
                                            if star_btn.clicked() {
                                                self.favorited[idx] = !self.favorited[idx];
                                            }

                                            // Add/remove button
                                            let (add_icon, add_color) = if self.added[idx] {
                                                (egui_phosphor::regular::CHECK_CIRCLE, ACCENT)
                                            } else {
                                                (egui_phosphor::regular::PLUS_CIRCLE, TEXT_MUTED)
                                            };
                                            let add_btn = ui.add(
                                                egui::Button::new(RichText::new(add_icon).size(14.0).color(add_color))
                                                    .fill(Color32::TRANSPARENT)
                                                    .stroke(Stroke::NONE),
                                            );
                                            if add_btn.clicked() {
                                                self.added[idx] = !self.added[idx];
                                            }

                                            // Likes count
                                            ui.label(RichText::new(format!("{}", likes)).size(11.0).color(TEXT_MUTED));
                                        });
                                    });
                                });

                                // Separator line below row
                                let row_rect = row_resp.response.rect;
                                let sep_rect = egui::Rect::from_min_size(
                                    egui::Pos2::new(row_rect.left(), row_rect.bottom()),
                                    Vec2::new(row_rect.width(), 1.0),
                                );
                                ui.painter().rect_filled(sep_rect, 0.0, Color32::from_rgb(40, 40, 44));
                            }
                        });
                    });

                    // ── Right sidebar: Added Indicators ──
                    ui.vertical(|ui| {
                        ui.set_width(190.0);

                        egui::Frame::new()
                            .fill(SIDEBAR_BG)
                            .corner_radius(CornerRadius::same(4))
                            .inner_margin(egui::Margin::same(10))
                            .show(ui, |ui| {
                                let added_count = self.added.iter().filter(|a| **a).count();
                                ui.label(
                                    RichText::new(format!("Added Indicators ({})", added_count))
                                        .size(11.0)
                                        .color(TEXT_WHITE)
                                        .strong(),
                                );

                                ui.add_space(8.0);

                                // Main Chart section
                                let main_indicators: Vec<_> = INDICATORS.iter().enumerate()
                                    .filter(|(idx, (_, _, is_sub))| self.added[*idx] && !is_sub)
                                    .collect();

                                if !main_indicators.is_empty() {
                                    ui.label(RichText::new("Main Chart").size(10.0).color(TEXT_MUTED));
                                    ui.add_space(4.0);
                                    for (_, (name, _, _)) in &main_indicators {
                                        ui.label(RichText::new(*name).size(11.0).color(TEXT_WHITE));
                                        ui.add_space(2.0);
                                    }
                                    ui.add_space(8.0);
                                }

                                // Sub Chart section
                                let sub_indicators: Vec<_> = INDICATORS.iter().enumerate()
                                    .filter(|(idx, (_, _, is_sub))| self.added[*idx] && *is_sub)
                                    .collect();

                                if !sub_indicators.is_empty() {
                                    ui.label(RichText::new("Sub Chart").size(10.0).color(TEXT_MUTED));
                                    ui.add_space(4.0);
                                    for (_, (name, _, _)) in &sub_indicators {
                                        ui.label(RichText::new(*name).size(11.0).color(TEXT_WHITE));
                                        ui.add_space(2.0);
                                    }
                                }
                            });
                    });
                });
            });
    }
}
