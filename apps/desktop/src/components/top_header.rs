use eframe::egui::{
    self, Align, Color32, CornerRadius, Layout, Pos2, Rect, RichText, Stroke, Vec2,
};
use tokio::sync::oneshot;
use zaned_api_client::Symbol;
use zaned_theme::{
    ACCENT, ACCENT_BG, AVATAR, BORDER, BORDER_HOVER, HEADER_HEIGHT, HOVER_BG, ICON_ACTIVE,
    ICON_HOVER, ICON_INACTIVE, ICON_ROUNDING, NOTIFICATION_DOT, SURFACE, SURFACE_HIGH, TEXT_MUTED,
    TEXT_PRIMARY,
};

// ── Dimensions ─────────────────────────────────────────────────
const AVATAR_SIZE: f32 = 28.0;
const PILL_WIDTH: f32 = 12.0;
const PILL_HEIGHT: f32 = 2.0;
const DOT_RADIUS: f32 = 3.0;

// ── Icon Groups ────────────────────────────────────────────────
const CHARTS_ICONS: [&str; 3] = [
    egui_phosphor::regular::CHART_BAR,
    egui_phosphor::regular::TREND_UP,
    egui_phosphor::regular::CHART_LINE_UP,
];

const BROWSE_ICONS: [&str; 3] = [
    egui_phosphor::regular::LIST_BULLETS,
    egui_phosphor::regular::HOUSE,
    egui_phosphor::regular::NEWSPAPER,
];

pub struct TopHeader {
    pub search_query: String,
    pub active_group: usize, // 0 = Charts, 1 = Browse
    pub active_icon: usize,  // index within the active group
    /// Last query string we actually fired a search for. We only fire a
    /// new search when `search_query` diverges from this — prevents one
    /// keystroke from spawning an in-flight + queued duplicate.
    last_searched_query: String,
    /// In-flight symbol search; polled each frame. `None` between fires.
    pending_search: Option<oneshot::Receiver<Result<Vec<Symbol>, String>>>,
    /// Latest search results to render in the dropdown.
    results: Vec<Symbol>,
    /// Symbol the user clicked on this frame, drained by `show()`.
    picked: Option<String>,
    /// Whether the dropdown should render. Hidden when the user picks a
    /// result, presses Esc, or clicks outside the search bar's panel.
    dropdown_open: bool,
}

impl Default for TopHeader {
    fn default() -> Self {
        Self {
            search_query: String::new(),
            active_group: 0,
            active_icon: 0,
            last_searched_query: String::new(),
            pending_search: None,
            results: Vec::new(),
            picked: None,
            dropdown_open: false,
        }
    }
}

impl TopHeader {
    fn paint_divider(ui: &mut egui::Ui, height: f32) {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, height), egui::Sense::hover());
        ui.painter().rect_filled(rect, 0.0, BORDER);
    }

    fn paint_icon_group(
        &mut self,
        ui: &mut egui::Ui,
        icons: &[&str],
        label: &str,
        group_index: usize,
    ) {
        let is_active_group = self.active_group == group_index;

        for (i, icon) in icons.iter().enumerate() {
            let is_active = is_active_group && self.active_icon == i;

            let icon_color = if is_active {
                ICON_ACTIVE
            } else {
                ICON_INACTIVE
            };

            let bg_fill = if is_active {
                ACCENT_BG
            } else {
                Color32::TRANSPARENT
            };

            let btn = ui.add(
                egui::Button::new(RichText::new(*icon).size(16.0).color(icon_color))
                    .fill(bg_fill)
                    .corner_radius(CornerRadius::same(ICON_ROUNDING as u8))
                    .min_size(Vec2::new(30.0, 30.0)),
            );

            if btn.hovered() && !is_active {
                let rect = btn.rect;
                ui.painter().rect_filled(rect, ICON_ROUNDING, HOVER_BG);
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    *icon,
                    egui::FontId::proportional(16.0),
                    ICON_HOVER,
                );
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }

            if is_active {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                let pill_rect = Rect::from_center_size(
                    Pos2::new(btn.rect.center().x, btn.rect.bottom() - 1.0),
                    Vec2::new(PILL_WIDTH, PILL_HEIGHT),
                );
                ui.painter().rect_filled(pill_rect, 1.0, ACCENT);
            }

            if btn.clicked() {
                self.active_group = group_index;
                self.active_icon = i;
            }
        }

        let _ = label; // group label removed from UI
    }

    fn paint_search(&mut self, ui: &mut egui::Ui) {
        use eframe::egui::{Frame, Margin};

        const SEARCH_PILL_WIDTH: f32 = 280.0;
        const SEARCH_PILL_HEIGHT: f32 = 32.0;

        // Drain any completed search BEFORE rendering — keeps the
        // dropdown current with the latest results.
        self.poll_pending_search();

        let frame = Frame::default()
            .fill(SURFACE_HIGH)
            .stroke(Stroke::new(1.0, BORDER))
            .corner_radius(CornerRadius::same(6))
            .inner_margin(Margin {
                left: 12,
                right: 12,
                top: 0,
                bottom: 0,
            });

        let frame_response = frame.show(ui, |ui| {
            ui.set_min_size(Vec2::new(SEARCH_PILL_WIDTH, SEARCH_PILL_HEIGHT));
            ui.set_max_size(Vec2::new(SEARCH_PILL_WIDTH, SEARCH_PILL_HEIGHT));

            // Force a left-to-right layout for the inner row. The parent
            // `paint` loop runs inside a right-to-left section (avatar +
            // bell on the right side of the header), which would otherwise
            // flip the icon and text inside this pill. Pinning to LTR keeps
            // the icon on the left and the text to its right, matching
            // Webull's pattern.
            ui.with_layout(
                eframe::egui::Layout::left_to_right(eframe::egui::Align::Center),
                |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;

                    // Magnifying glass icon — small, muted.
                    ui.label(
                        RichText::new(egui_phosphor::regular::MAGNIFYING_GLASS)
                            .size(13.0)
                            .color(TEXT_MUTED),
                    );

                    // Make the TextEdit transparent and FRAMELESS so the outer
                    // pill is the only visible chrome — no internal cyan focus
                    // ring around just the text portion.
                    let te_response = ui
                        .scope(|ui| {
                            let style = ui.style_mut();
                            style.visuals.extreme_bg_color = Color32::TRANSPARENT;
                            style.visuals.widgets.inactive.bg_fill = Color32::TRANSPARENT;
                            style.visuals.widgets.inactive.bg_stroke = Stroke::NONE;
                            style.visuals.widgets.hovered.bg_fill = Color32::TRANSPARENT;
                            style.visuals.widgets.hovered.bg_stroke = Stroke::NONE;
                            style.visuals.widgets.active.bg_fill = Color32::TRANSPARENT;
                            style.visuals.widgets.active.bg_stroke = Stroke::NONE;
                            style.visuals.widgets.open.bg_fill = Color32::TRANSPARENT;
                            style.visuals.widgets.open.bg_stroke = Stroke::NONE;
                            style.visuals.selection.bg_fill = ACCENT_BG;
                            style.visuals.selection.stroke = Stroke::NONE;

                            let te = egui::TextEdit::singleline(&mut self.search_query)
                                .hint_text(
                                    RichText::new("Search and view in Stocks").color(TEXT_MUTED),
                                )
                                .text_color(TEXT_PRIMARY)
                                .desired_width(f32::INFINITY)
                                .margin(Margin::ZERO);

                            // Sized to natural text height so cross-axis centering
                            // vertically aligns the text in the 32px pill.
                            ui.add_sized(Vec2::new(ui.available_width(), 18.0), te)
                        })
                        .inner;

                    // Open the dropdown on focus so an empty-but-focused
                    // search bar still hides the panel until the user types.
                    if te_response.has_focus() {
                        self.dropdown_open = true;
                    }
                },
            );
        });

        // Fire a fresh search whenever the query has changed since the
        // last fire AND is non-empty. Replaces any in-flight search —
        // older results would just stomp newer ones.
        let query = self.search_query.trim().to_string();
        if query != self.last_searched_query {
            self.last_searched_query = query.clone();
            if query.is_empty() {
                self.results.clear();
                self.pending_search = None;
            } else {
                self.pending_search = Some(crate::api::symbol_search::search_async(query));
            }
        }

        // Esc clears the dropdown without losing focus.
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.dropdown_open = false;
        }

        // Paint the dropdown as a free-floating Area anchored just below
        // the search pill. Only when there's something to show.
        if self.dropdown_open && !self.search_query.trim().is_empty() {
            self.paint_search_dropdown(ui, frame_response.response.rect);
        }
    }

    /// Drain a completed search-symbols receiver. On success, swap the
    /// new results into the dropdown. On error, clear results and log.
    fn poll_pending_search(&mut self) {
        let Some(rx) = self.pending_search.as_mut() else {
            return;
        };
        match rx.try_recv() {
            Ok(Ok(symbols)) => {
                self.results = symbols;
                self.pending_search = None;
            }
            Ok(Err(msg)) => {
                tracing::warn!(error = %msg, "symbol search failed");
                self.results.clear();
                self.pending_search = None;
            }
            Err(oneshot::error::TryRecvError::Empty) => {
                // Still loading — leave existing `results` visible (if any)
                // so the dropdown doesn't flicker between every keystroke.
            }
            Err(oneshot::error::TryRecvError::Closed) => {
                self.pending_search = None;
            }
        }
    }

    /// Paint the result dropdown anchored to the bottom-left of
    /// `pill_rect` (the search bar's outer frame). Each row is a clickable
    /// button that, on click, populates `self.picked` and clears state so
    /// the parent (`show`) can pick it up and forward to the chart.
    fn paint_search_dropdown(&mut self, ui: &mut egui::Ui, pill_rect: Rect) {
        const ROW_HEIGHT: f32 = 36.0;
        const DROPDOWN_OFFSET: f32 = 4.0;
        const DROPDOWN_WIDTH: f32 = 320.0;

        // Anchor the dropdown using the pill's right edge so it doesn't
        // bleed past the window's right gutter when the header is narrow.
        let anchor = Pos2::new(
            pill_rect.right() - DROPDOWN_WIDTH,
            pill_rect.bottom() + DROPDOWN_OFFSET,
        );

        let area = egui::Area::new(egui::Id::new("top_header_search_dropdown"))
            .order(egui::Order::Foreground)
            .fixed_pos(anchor);

        area.show(ui.ctx(), |ui| {
            egui::Frame::default()
                .fill(SURFACE)
                .stroke(Stroke::new(1.0, BORDER))
                .corner_radius(CornerRadius::same(6))
                .inner_margin(egui::Margin::same(4))
                .shadow(egui::epaint::Shadow {
                    offset: [0, 8],
                    blur: 24,
                    spread: 0,
                    color: Color32::from_black_alpha(60),
                })
                .show(ui, |ui| {
                    ui.set_min_width(DROPDOWN_WIDTH - 8.0);
                    ui.set_max_width(DROPDOWN_WIDTH - 8.0);

                    if self.results.is_empty() {
                        // Two states: searching, or no matches. We treat
                        // the in-flight receiver as "searching" — once the
                        // first response lands `pending_search` clears.
                        let label = if self.pending_search.is_some() {
                            "Searching…"
                        } else {
                            "No matches"
                        };
                        ui.add_space(8.0);
                        ui.vertical_centered(|ui| {
                            ui.label(RichText::new(label).color(TEXT_MUTED).size(13.0));
                        });
                        ui.add_space(8.0);
                        return;
                    }

                    for symbol in self.results.clone() {
                        let row_response = ui.allocate_response(
                            Vec2::new(ui.available_width(), ROW_HEIGHT),
                            egui::Sense::click(),
                        );
                        let rect = row_response.rect;

                        if row_response.hovered() {
                            ui.painter().rect_filled(rect, 4.0, HOVER_BG);
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }

                        // Row layout: [logo 24x24] [TICKER + name stack] ... [exchange]
                        const LOGO_SIZE: f32 = 24.0;
                        const LOGO_LEFT_PAD: f32 = 8.0;
                        const TEXT_LEFT_PAD: f32 = LOGO_LEFT_PAD + LOGO_SIZE + 10.0;
                        let logo_rect = Rect::from_min_size(
                            Pos2::new(
                                rect.left() + LOGO_LEFT_PAD,
                                rect.center().y - LOGO_SIZE / 2.0,
                            ),
                            Vec2::splat(LOGO_SIZE),
                        );
                        ui.put(
                            logo_rect,
                            crate::api::logo::image_for(&symbol.symbol)
                                .fit_to_exact_size(Vec2::splat(LOGO_SIZE))
                                .corner_radius(CornerRadius::same(4)),
                        );

                        let painter = ui.painter_at(rect);
                        let ticker_pos = Pos2::new(
                            rect.left() + TEXT_LEFT_PAD,
                            rect.top() + ROW_HEIGHT / 2.0 - 8.0,
                        );
                        painter.text(
                            ticker_pos,
                            egui::Align2::LEFT_TOP,
                            &symbol.symbol,
                            egui::FontId::monospace(13.0),
                            TEXT_PRIMARY,
                        );

                        if let Some(name) = symbol.long_name.as_deref() {
                            let name_pos = Pos2::new(
                                rect.left() + TEXT_LEFT_PAD,
                                rect.top() + ROW_HEIGHT / 2.0 + 2.0,
                            );
                            // Truncate visually-overflowing names so the
                            // exchange tag on the right still fits.
                            let max_name_chars = 32;
                            let name_display = if name.chars().count() > max_name_chars {
                                let truncated: String = name.chars().take(max_name_chars).collect();
                                format!("{truncated}…")
                            } else {
                                name.to_string()
                            };
                            painter.text(
                                name_pos,
                                egui::Align2::LEFT_TOP,
                                name_display,
                                egui::FontId::proportional(11.0),
                                TEXT_MUTED,
                            );
                        }

                        // Right-aligned exchange tag.
                        painter.text(
                            Pos2::new(rect.right() - 10.0, rect.center().y),
                            egui::Align2::RIGHT_CENTER,
                            &symbol.exchange,
                            egui::FontId::proportional(11.0),
                            TEXT_MUTED,
                        );

                        if row_response.clicked() {
                            self.picked = Some(symbol.symbol.clone());
                            self.search_query.clear();
                            self.last_searched_query.clear();
                            self.results.clear();
                            self.pending_search = None;
                            self.dropdown_open = false;
                        }
                    }
                });
        });

        // Click-outside closes the dropdown — only collapses if the
        // user clicked OUTSIDE both the pill and the panel area.
        if ui.input(|i| i.pointer.any_click()) {
            let dropdown_rect = Rect::from_min_size(
                Pos2::new(
                    pill_rect.right() - DROPDOWN_WIDTH,
                    pill_rect.bottom() + DROPDOWN_OFFSET,
                ),
                Vec2::new(
                    DROPDOWN_WIDTH,
                    (self.results.len() as f32 * ROW_HEIGHT + 16.0).max(60.0),
                ),
            );
            if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                if !pill_rect.contains(pos) && !dropdown_rect.contains(pos) {
                    self.dropdown_open = false;
                }
            }
        }

        // Keep `BORDER_HOVER` referenced — the dropdown's border is
        // currently `BORDER`, but the stylesheet exports `BORDER_HOVER`
        // for hover feedback that may be used in a future polish pass.
        let _ = BORDER_HOVER;
    }

    fn paint_bell(ui: &mut egui::Ui, has_notification: bool) {
        let btn = ui.add(
            egui::Button::new(
                RichText::new(egui_phosphor::regular::BELL)
                    .size(16.0)
                    .color(ICON_INACTIVE),
            )
            .fill(Color32::TRANSPARENT)
            .corner_radius(CornerRadius::same(ICON_ROUNDING as u8))
            .min_size(Vec2::new(30.0, 30.0)),
        );

        if btn.hovered() {
            let rect = btn.rect;
            ui.painter().rect_filled(rect, ICON_ROUNDING, HOVER_BG);
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                egui_phosphor::regular::BELL,
                egui::FontId::proportional(16.0),
                ICON_HOVER,
            );
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        if has_notification {
            let dot_center = Pos2::new(btn.rect.right() - 7.0, btn.rect.top() + 7.0);
            ui.painter()
                .circle_filled(dot_center, DOT_RADIUS + 1.5, SURFACE);
            ui.painter()
                .circle_filled(dot_center, DOT_RADIUS, NOTIFICATION_DOT);
        }
    }

    fn paint_avatar(ui: &mut egui::Ui, initial: char) {
        let size = Vec2::splat(AVATAR_SIZE);
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());

        let color = if response.hovered() {
            Color32::from_rgb(130, 115, 245)
        } else {
            AVATAR
        };

        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        ui.painter()
            .circle_filled(rect.center(), AVATAR_SIZE / 2.0, color);

        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            initial.to_string(),
            egui::FontId::proportional(11.0),
            Color32::WHITE,
        );
    }

    /// Render the header. Returns the symbol the user picked from the
    /// search dropdown this frame (if any) so the caller can forward it
    /// to the chart.
    pub fn show(&mut self, ui: &mut egui::Ui) -> Option<String> {
        egui::Frame::new()
            .fill(SURFACE)
            .inner_margin(egui::Margin {
                left: 76,
                right: 16,
                top: 0,
                bottom: 0,
            })
            .stroke(Stroke::NONE)
            .show(ui, |ui| {
                ui.set_height(HEADER_HEIGHT);

                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 1.0;

                    ui.add_space(14.0);

                    // ── Charts Group ──
                    self.paint_icon_group(ui, &CHARTS_ICONS, "Charts", 0);
                    ui.add_space(14.0);
                    Self::paint_divider(ui, 14.0);
                    ui.add_space(14.0);

                    // ── Browse Group ──
                    self.paint_icon_group(ui, &BROWSE_ICONS, "Browse", 1);

                    // ── Right Section (RTL) ──
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;

                        // Avatar
                        Self::paint_avatar(ui, 'J');

                        ui.add_space(6.0);

                        // Bell
                        Self::paint_bell(ui, true);

                        ui.add_space(6.0);

                        // Search
                        self.paint_search(ui);
                    });
                });
            });

        self.picked.take()
    }
}
