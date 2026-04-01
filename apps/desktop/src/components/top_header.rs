use eframe::egui::{self, Align, Color32, CornerRadius, Layout, Pos2, Rect, RichText, Stroke, Vec2};

// ── Color Palette ──────────────────────────────────────────────
const HEADER_BG: Color32 = Color32::from_rgb(24, 24, 28);
const BORDER: Color32 = Color32::from_rgb(30, 30, 33);
const ACCENT: Color32 = Color32::from_rgb(99, 102, 241);
const ACCENT_BG: Color32 = Color32::from_rgb(33, 33, 54);
const TEXT_PRIMARY: Color32 = Color32::from_rgb(240, 240, 242);
const TEXT_MUTED: Color32 = Color32::from_rgb(63, 63, 63);
const ICON_INACTIVE: Color32 = Color32::from_rgba_premultiplied(240, 240, 242, 89); // 35%
const ICON_HOVER: Color32 = Color32::from_rgba_premultiplied(240, 240, 242, 153); // 60%
const ICON_ACTIVE: Color32 = Color32::from_rgb(240, 240, 242); // full
const HOVER_BG: Color32 = Color32::from_rgb(30, 30, 33);
const NOTIFICATION_DOT: Color32 = Color32::from_rgb(239, 68, 68);
const AVATAR_COLOR: Color32 = Color32::from_rgb(119, 98, 243); // midpoint of indigo→violet

// ── Dimensions ─────────────────────────────────────────────────
const HEADER_HEIGHT: f32 = 44.0;
const ICON_ROUNDING: f32 = 6.0;
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
    pub active_group: usize,  // 0 = Charts, 1 = Browse
    pub active_icon: usize,   // index within the active group
}

impl Default for TopHeader {
    fn default() -> Self {
        Self {
            search_query: String::new(),
            active_group: 0,
            active_icon: 0,
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

            let bg_fill = if is_active { ACCENT_BG } else { Color32::TRANSPARENT };

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
        ui.scope(|ui| {
            // Force all TextEdit widget visuals to use the header background
            let style = ui.style_mut();
            style.visuals.extreme_bg_color = Color32::from_rgb(34, 34, 38);
            style.visuals.widgets.inactive.bg_fill = Color32::TRANSPARENT;
            style.visuals.widgets.inactive.bg_stroke = Stroke::NONE;
            style.visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT_MUTED);
            style.visuals.widgets.hovered.bg_fill = Color32::TRANSPARENT;
            style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, BORDER);
            style.visuals.widgets.active.bg_fill = Color32::TRANSPARENT;
            style.visuals.widgets.active.bg_stroke = Stroke::new(1.0, BORDER);
            style.visuals.selection.bg_fill = ACCENT_BG;

            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                ui.label(
                    RichText::new(egui_phosphor::regular::MAGNIFYING_GLASS)
                        .size(14.0)
                        .color(ICON_INACTIVE),
                );
                let te = egui::TextEdit::singleline(&mut self.search_query)
                    .hint_text(RichText::new("Search and view stocks").color(TEXT_MUTED))
                    .text_color(TEXT_PRIMARY)
                    .margin(egui::Margin::symmetric(4, 6));
                ui.add_sized(Vec2::new(240.0, HEADER_HEIGHT - 12.0), te);
            });
        });
    }

    fn paint_bell(ui: &mut egui::Ui, has_notification: bool) {
        let btn = ui.add(
            egui::Button::new(
                RichText::new(egui_phosphor::regular::BELL).size(16.0).color(ICON_INACTIVE),
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
            ui.painter().circle_filled(dot_center, DOT_RADIUS + 1.5, HEADER_BG);
            ui.painter().circle_filled(dot_center, DOT_RADIUS, NOTIFICATION_DOT);
        }
    }

    fn paint_avatar(ui: &mut egui::Ui, initial: char) {
        let size = Vec2::splat(AVATAR_SIZE);
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());

        let color = if response.hovered() {
            Color32::from_rgb(130, 115, 245)
        } else {
            AVATAR_COLOR
        };

        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        ui.painter().circle_filled(rect.center(), AVATAR_SIZE / 2.0, color);

        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            initial.to_string(),
            egui::FontId::proportional(11.0),
            Color32::WHITE,
        );
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new()
            .fill(HEADER_BG)
            .inner_margin(egui::Margin { left: 76, right: 16, top: 0, bottom: 0 })
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
    }
}
