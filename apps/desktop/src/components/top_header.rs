use eframe::egui::{self, Align, Color32, CornerRadius, Layout, RichText, Stroke, Vec2};

#[derive(Default)]
pub struct TopHeader {
    pub search_query: String,
}

impl TopHeader {
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let header_height = 36.0;
        let bg_color = Color32::from_rgb(24, 24, 28);
        let icon_color = Color32::from_rgb(160, 160, 170);
        let brand_color = Color32::from_rgb(255, 255, 255);
        let border_color = Color32::from_rgb(50, 50, 55);

        egui::Frame::new()
            .fill(bg_color)
            .inner_margin(egui::Margin { left: 76, right: 12, top: 4, bottom: 4 })
            .stroke(Stroke::new(1.0, border_color))
            .show(ui, |ui| {
                ui.set_height(header_height);

                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;

                    // Brand
                    ui.label(RichText::new("Zaned").color(brand_color).strong().size(14.0));

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);

                    // Nav icons
                    let nav_icons = [
                        egui_phosphor::regular::CHART_BAR,
                        egui_phosphor::regular::TREND_UP,
                        egui_phosphor::regular::CHART_LINE_UP,
                        egui_phosphor::regular::LIST_BULLETS,
                        egui_phosphor::regular::HOUSE,
                        egui_phosphor::regular::NEWSPAPER,
                        egui_phosphor::regular::BELL,
                        egui_phosphor::regular::DOTS_THREE_VERTICAL,
                    ];
                    for icon in &nav_icons {
                        let btn = ui.add(
                            egui::Button::new(RichText::new(*icon).size(16.0).color(icon_color))
                                .fill(Color32::TRANSPARENT)
                                .corner_radius(CornerRadius::same(4))
                                .min_size(Vec2::new(28.0, 28.0)),
                        );
                        if btn.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                    }

                    // Push search + account to the right
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;

                        // Account
                        ui.label(
                            RichText::new(format!("{} Account", egui_phosphor::regular::USER_CIRCLE))
                                .color(icon_color)
                                .size(12.0),
                        );

                        ui.separator();

                        // Search bar
                        let search_frame = egui::Frame::new()
                            .fill(Color32::from_rgb(36, 36, 40))
                            .corner_radius(CornerRadius::same(6))
                            .stroke(Stroke::new(1.0, border_color))
                            .inner_margin(egui::Margin::symmetric(8, 4));

                        search_frame.show(ui, |ui| {
                            ui.set_width(220.0);
                            ui.horizontal_centered(|ui| {
                                ui.label(RichText::new(egui_phosphor::regular::MAGNIFYING_GLASS).size(14.0).color(icon_color));
                                let te = egui::TextEdit::singleline(&mut self.search_query)
                                    .hint_text("Search symbols...")
                                    .desired_width(180.0)
                                    .text_color(brand_color);
                                ui.add(te);
                            });
                        });
                    });
                });
            });
    }
}
