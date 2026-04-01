use eframe::egui::{self, Color32, CornerRadius, RichText, Stroke, Vec2};

// ── Colors ─────────────────────────────────────────────────────
const ROW_HOVER: Color32 = Color32::from_rgb(22, 22, 26);
const BORDER: Color32 = Color32::from_rgb(30, 30, 33);
const TEXT_WHITE: Color32 = Color32::from_rgb(240, 240, 242);
const TEXT_MUTED: Color32 = Color32::from_rgb(85, 85, 90);
const TEXT_LABEL: Color32 = Color32::from_rgb(120, 120, 130);
const GREEN: Color32 = Color32::from_rgb(78, 205, 196);
const RED: Color32 = Color32::from_rgb(255, 107, 107);
const ORANGE: Color32 = Color32::from_rgb(255, 165, 0);
const BADGE_BG: Color32 = Color32::from_rgb(30, 30, 34);

#[derive(Clone)]
pub struct StockRow {
    pub symbol: String,
    pub name: String,
    pub price: f64,
    pub change: f64,
    pub has_earnings: bool,
    pub has_news: bool,
}

pub struct MainSidebar {
    pub stocks: Vec<StockRow>,
}

impl Default for MainSidebar {
    fn default() -> Self {
        Self {
            stocks: Vec::new(),
        }
    }
}

impl MainSidebar {
    fn paint_header(ui: &mut egui::Ui) {
        let dropdown_frame = egui::Frame::new()
            .fill(Color32::from_rgb(22, 22, 26))
            .corner_radius(CornerRadius::same(4))
            .stroke(Stroke::new(1.0, Color32::from_rgb(40, 40, 44)))
            .inner_margin(egui::Margin::symmetric(10, 6));

        ui.horizontal(|ui| {
            ui.add_space(20.0);
            let available = ui.available_width() - 50.0; // reserve space for settings icon + gap
            dropdown_frame.show(ui, |ui| {
                ui.set_width(available.max(80.0));
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Recently Viewed").color(TEXT_WHITE).size(11.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(egui_phosphor::regular::CARET_DOWN)
                                .size(12.0)
                                .color(TEXT_LABEL),
                        );
                    });
                });
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let settings = ui.add(
                    egui::Button::new(
                        RichText::new(egui_phosphor::regular::SLIDERS_HORIZONTAL)
                            .size(14.0)
                            .color(TEXT_LABEL),
                    )
                    .fill(Color32::TRANSPARENT)
                    .min_size(Vec2::new(24.0, 24.0)),
                );
                if settings.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
            });
        });
    }

    fn paint_column_headers(ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.label(
                RichText::new(egui_phosphor::regular::LIST)
                    .size(12.0)
                    .color(TEXT_MUTED),
            );
            ui.label(RichText::new("Symbol").color(TEXT_MUTED).size(10.0));

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.label(
                    RichText::new(egui_phosphor::regular::ARROW_DOWN)
                        .size(10.0)
                        .color(TEXT_MUTED),
                );
                ui.label(RichText::new("Price/Change").color(TEXT_MUTED).size(10.0));
            });
        });
    }

    fn paint_stock_row(ui: &mut egui::Ui, stock: &StockRow) {
        let change_color = if stock.change > 0.0 {
            GREEN
        } else if stock.change < 0.0 {
            RED
        } else {
            TEXT_MUTED
        };

        let price_color = if stock.change > 0.0 {
            GREEN
        } else if stock.change < 0.0 {
            RED
        } else {
            TEXT_WHITE
        };

        let row_frame = egui::Frame::new()
            .fill(Color32::TRANSPARENT)
            .inner_margin(egui::Margin::symmetric(0, 6))
            .stroke(Stroke::NONE);

        let resp = row_frame.show(ui, |ui| {
            ui.set_width(ui.available_width());

            ui.horizontal(|ui| {
                ui.label(RichText::new(&stock.symbol).color(TEXT_WHITE).strong().size(13.0));

                if stock.has_news {
                    let badge = egui::Frame::new()
                        .fill(BADGE_BG)
                        .corner_radius(CornerRadius::same(2))
                        .inner_margin(egui::Margin::symmetric(3, 1));
                    badge.show(ui, |ui| {
                        ui.label(
                            RichText::new(egui_phosphor::regular::NEWSPAPER)
                                .size(9.0)
                                .color(ORANGE),
                        );
                    });
                }

                if stock.has_earnings {
                    let badge = egui::Frame::new()
                        .fill(ORANGE)
                        .corner_radius(CornerRadius::same(2))
                        .inner_margin(egui::Margin::symmetric(3, 1));
                    badge.show(ui, |ui| {
                        ui.label(
                            RichText::new("E")
                                .size(8.0)
                                .color(Color32::BLACK)
                                .strong(),
                        );
                    });
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("{:.2}", stock.price))
                            .color(price_color)
                            .size(13.0)
                            .strong(),
                    );
                });
            });

            ui.horizontal(|ui| {
                ui.label(RichText::new(&stock.name).color(TEXT_MUTED).size(10.0));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let sign = if stock.change > 0.0 { "+" } else { "" };
                    ui.label(
                        RichText::new(format!("After: {sign}{:.2}%", stock.change))
                            .color(change_color)
                            .size(10.0),
                    );
                });
            });
        });

        let row_rect = resp.response.rect;
        if resp.response.hovered() {
            ui.painter().rect_filled(row_rect, 0.0, ROW_HOVER);
        }

        let border_rect = egui::Rect::from_min_size(
            egui::Pos2::new(row_rect.left(), row_rect.bottom()),
            Vec2::new(row_rect.width(), 1.0),
        );
        ui.painter().rect_filled(border_rect, 0.0, BORDER);
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;

            ui.add_space(8.0);
            ui.scope(|ui| {
                ui.spacing_mut().item_spacing.y = 8.0;
                Self::paint_header(ui);
                Self::paint_column_headers(ui);
            });

            ui.add_space(4.0);

            let sep_rect = egui::Rect::from_min_size(
                ui.cursor().min,
                Vec2::new(ui.available_width(), 1.0),
            );
            ui.painter().rect_filled(sep_rect, 0.0, BORDER);
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), egui::Sense::hover());

            if self.stocks.is_empty() {
                ui.add_space(40.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("No stocks yet").color(TEXT_MUTED).size(12.0));
                    ui.add_space(4.0);
                    ui.label(RichText::new("Search and add symbols to your watchlist").color(TEXT_MUTED).size(10.0));
                });
            } else {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let stocks = self.stocks.clone();
                    for stock in &stocks {
                        Self::paint_stock_row(ui, stock);
                    }
                });
            }
        });
    }
}
