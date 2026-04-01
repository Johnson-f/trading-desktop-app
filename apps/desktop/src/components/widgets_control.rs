use eframe::egui::{self, Color32, CornerRadius, RichText, Stroke, Vec2};

// ── Colors ─────────────────────────────────────────────────────
const BG: Color32 = Color32::from_rgb(18, 18, 22);
const TAB_ACTIVE_BG: Color32 = Color32::from_rgb(30, 30, 34);
const TAB_ACTIVE_TEXT: Color32 = Color32::from_rgb(240, 240, 242);
const TAB_INACTIVE_TEXT: Color32 = Color32::from_rgb(100, 100, 110);
const BORDER: Color32 = Color32::from_rgb(30, 30, 33);
const ICON_COLOR: Color32 = Color32::from_rgb(120, 120, 130);
const ICON_HOVER: Color32 = Color32::from_rgb(200, 200, 210);
const HOVER_BG: Color32 = Color32::from_rgb(28, 28, 32);
const ACCENT: Color32 = Color32::from_rgb(78, 205, 196);

const TABS: &[&str] = &[
    "Chart",
    "Corp Actions",
    "Institutions",
    "Messages",
    "News",
    "Options",
    "Profile",
    "Financials",
    "Quotes",
    "Comments",
    "Analysis",
    "Order Flow",
    "Releases",
];

const TOOLBAR_ICONS: &[&str] = &[
    egui_phosphor::regular::CURSOR,
    egui_phosphor::regular::PENCIL_SIMPLE,
    egui_phosphor::regular::PAINT_BUCKET,
    egui_phosphor::regular::CHART_BAR,
    egui_phosphor::regular::CHART_LINE,
    egui_phosphor::regular::TEXT_T,
    egui_phosphor::regular::CLOUD,
    egui_phosphor::regular::CURRENCY_DOLLAR,
    egui_phosphor::regular::SMILEY,
    egui_phosphor::regular::CROSSHAIR,
    egui_phosphor::regular::RULER,
    egui_phosphor::regular::LINK,
    egui_phosphor::regular::ARROWS_OUT_SIMPLE,
    egui_phosphor::regular::FUNNEL,
    egui_phosphor::regular::MAGNET,
    egui_phosphor::regular::LOCK_SIMPLE,
    egui_phosphor::regular::EYE,
    egui_phosphor::regular::TRASH,
];

pub struct WidgetsControl {
    pub active_tab: usize,
}

impl Default for WidgetsControl {
    fn default() -> Self {
        Self { active_tab: 0 }
    }
}

impl WidgetsControl {
    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;

            // ── Tab Bar ──
            self.paint_tab_bar(ui);

            // ── Toolbar ──
            self.paint_toolbar(ui);
        });
    }

    fn paint_tab_bar(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new()
            .fill(BG)
            .inner_margin(egui::Margin::symmetric(8, 2))
            .stroke(Stroke::new(0.5, BORDER))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                egui::ScrollArea::horizontal()
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 2.0;

                            for (i, tab) in TABS.iter().enumerate() {
                                let is_active = self.active_tab == i;

                                let text_color = if is_active { TAB_ACTIVE_TEXT } else { TAB_INACTIVE_TEXT };
                                let fill = if is_active { TAB_ACTIVE_BG } else { Color32::TRANSPARENT };

                                let btn = ui.add(
                                    egui::Button::new(
                                        RichText::new(*tab).size(11.0).color(text_color),
                                    )
                                    .fill(fill)
                                    .corner_radius(CornerRadius::same(4))
                                    .stroke(Stroke::NONE)
                                    .min_size(Vec2::new(0.0, 24.0)),
                                );

                                if btn.hovered() && !is_active {
                                    let rect = btn.rect;
                                    ui.painter().rect_filled(rect, 4.0, HOVER_BG);
                                    ui.painter().text(
                                        rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        *tab,
                                        egui::FontId::proportional(11.0),
                                        ICON_HOVER,
                                    );
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                }

                                if is_active {
                                    // Underline accent
                                    let rect = btn.rect;
                                    let underline = egui::Rect::from_min_size(
                                        egui::Pos2::new(rect.left() + 4.0, rect.bottom() - 2.0),
                                        Vec2::new(rect.width() - 8.0, 2.0),
                                    );
                                    ui.painter().rect_filled(underline, 1.0, ACCENT);
                                }

                                if btn.clicked() {
                                    self.active_tab = i;
                                }
                            }
                        });
                    });
            });
    }

    fn paint_toolbar(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new()
            .fill(BG)
            .inner_margin(egui::Margin::symmetric(8, 4))
            .stroke(Stroke::new(0.5, BORDER))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;

                    for (i, icon) in TOOLBAR_ICONS.iter().enumerate() {
                        let btn = ui.add(
                            egui::Button::new(
                                RichText::new(*icon).size(15.0).color(ICON_COLOR),
                            )
                            .fill(Color32::TRANSPARENT)
                            .corner_radius(CornerRadius::same(4))
                            .min_size(Vec2::new(28.0, 28.0)),
                        );

                        if btn.hovered() {
                            let rect = btn.rect;
                            ui.painter().rect_filled(rect, 4.0, HOVER_BG);
                            ui.painter().text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                *icon,
                                egui::FontId::proportional(15.0),
                                ICON_HOVER,
                            );
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }

                        // Add separators after certain groups
                        if i == 1 || i == 5 || i == 9 || i == 13 {
                            ui.add_space(2.0);
                            let (rect, _) = ui.allocate_exact_size(
                                Vec2::new(1.0, 18.0),
                                egui::Sense::hover(),
                            );
                            ui.painter().rect_filled(rect, 0.0, BORDER);
                            ui.add_space(2.0);
                        }
                    }
                });
            });
    }
}
