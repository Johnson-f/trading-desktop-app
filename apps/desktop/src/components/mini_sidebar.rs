use eframe::egui::{self, Color32, CornerRadius, Pos2, Rect, RichText, Vec2};

// ── Color Palette (matches top_header) ─────────────────────────
const ACCENT: Color32 = Color32::from_rgb(99, 102, 241);
const ACCENT_BG: Color32 = Color32::from_rgb(33, 33, 54);
const ICON_INACTIVE: Color32 = Color32::from_rgba_premultiplied(240, 240, 242, 89);
const ICON_HOVER: Color32 = Color32::from_rgba_premultiplied(240, 240, 242, 153);
const ICON_ACTIVE: Color32 = Color32::from_rgb(240, 240, 242);
const HOVER_BG: Color32 = Color32::from_rgb(30, 30, 33);

// ── Dimensions ─────────────────────────────────────────────────
const ICON_ROUNDING: f32 = 6.0;
const PILL_WIDTH: f32 = 2.0;
const PILL_HEIGHT: f32 = 12.0;

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

pub struct MiniSidebar {
    pub active_group: usize,
    pub active_icon: usize,
}

impl Default for MiniSidebar {
    fn default() -> Self {
        Self {
            active_group: 0,
            active_icon: 0,
        }
    }
}

impl MiniSidebar {
    fn paint_divider(ui: &mut egui::Ui) {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(14.0, 1.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 0.0, Color32::from_rgb(30, 30, 33));
    }

    fn paint_icon(
        &mut self,
        ui: &mut egui::Ui,
        icon: &str,
        group_index: usize,
        icon_index: usize,
    ) {
        let is_active = self.active_group == group_index && self.active_icon == icon_index;

        let icon_color = if is_active { ICON_ACTIVE } else { ICON_INACTIVE };
        let bg_fill = if is_active { ACCENT_BG } else { Color32::TRANSPARENT };

        let btn = ui.add(
            egui::Button::new(RichText::new(icon).size(16.0).color(icon_color))
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
                icon,
                egui::FontId::proportional(16.0),
                ICON_HOVER,
            );
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        if is_active {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            // Paint active indicator pill to the right of the icon
            let pill_rect = Rect::from_center_size(
                Pos2::new(btn.rect.right() - 1.0, btn.rect.center().y),
                Vec2::new(PILL_WIDTH, PILL_HEIGHT),
            );
            ui.painter().rect_filled(pill_rect, 1.0, ACCENT);
        }

        if btn.clicked() {
            self.active_group = group_index;
            self.active_icon = icon_index;
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.spacing_mut().item_spacing.y = 8.0;
            ui.add_space(8.0);

            // ── Charts Group ──
            for i in 0..CHARTS_ICONS.len() {
                self.paint_icon(ui, CHARTS_ICONS[i], 0, i);
            }

            ui.add_space(4.0);
            Self::paint_divider(ui);
            ui.add_space(4.0);

            // ── Browse Group ──
            for i in 0..BROWSE_ICONS.len() {
                self.paint_icon(ui, BROWSE_ICONS[i], 1, i);
            }
        });
    }
}
