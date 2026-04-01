use eframe::egui::{self, Color32, CornerRadius, RichText, Stroke, Vec2};

const ACCENT: Color32 = Color32::from_rgb(78, 205, 196);
const TEXT_COLOR: Color32 = Color32::from_rgb(180, 180, 190);
const TEXT_HOVER: Color32 = Color32::from_rgb(240, 240, 242);
const HOVER_BG: Color32 = Color32::from_rgb(28, 28, 32);
const BORDER: Color32 = Color32::from_rgb(30, 30, 33);
const ACTIVE_BG: Color32 = Color32::from_rgb(35, 35, 40);

pub struct IndicatorLabel {
    pub name: &'static str,
    pub active: bool,
}

pub struct IndicatorBar {
    pub indicators: Vec<IndicatorLabel>,
}

impl Default for IndicatorBar {
    fn default() -> Self {
        Self {
            indicators: vec![
                IndicatorLabel { name: "EMA", active: false },
                IndicatorLabel { name: "MA", active: false },
                IndicatorLabel { name: "BOLL", active: false },
                IndicatorLabel { name: "VWAP", active: false },
                IndicatorLabel { name: "MACD", active: false },
                IndicatorLabel { name: "RSI", active: false },
                IndicatorLabel { name: "VOL", active: true }, // volume on by default
            ],
        }
    }
}

impl IndicatorBar {
    /// Render the indicator quick-access labels inline.
    /// Returns (toggled_indicator, open_all_indicators_modal)
    pub fn show_inline(&mut self, ui: &mut egui::Ui) -> (bool, bool) {
        let mut toggled = false;
        let mut open_modal = false;

        // Disable click animation
        ui.style_mut().animation_time = 0.0;

        // Separator before indicators
        ui.add_space(4.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, 18.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 0.0, BORDER);
        ui.add_space(4.0);

        for indicator in &mut self.indicators {
            let text_color = if indicator.active { ACCENT } else { TEXT_COLOR };

            let btn = ui.add(
                egui::Button::new(
                    RichText::new(indicator.name).size(11.0).color(text_color).strong(),
                )
                .fill(if indicator.active { ACTIVE_BG } else { Color32::TRANSPARENT })
                .corner_radius(CornerRadius::same(3))
                .stroke(Stroke::NONE)
                .min_size(Vec2::new(0.0, 22.0)),
            );

            if btn.hovered() {
                let rect = btn.rect;
                if !indicator.active {
                    ui.painter().rect_filled(rect, 3.0, HOVER_BG);
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        indicator.name,
                        egui::FontId::proportional(11.0),
                        TEXT_HOVER,
                    );
                }
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }

            if btn.clicked() {
                indicator.active = !indicator.active;
                toggled = true;
            }
        }

        // "All Indicators" button
        ui.add_space(4.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, 18.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 0.0, BORDER);
        ui.add_space(4.0);

        let all_btn = ui.add(
            egui::Button::new(
                RichText::new("All Indicators").size(11.0).color(TEXT_COLOR),
            )
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::NONE)
            .min_size(Vec2::new(0.0, 22.0)),
        );
        if all_btn.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if all_btn.clicked() {
            open_modal = true;
        }

        // "Technical Signal" dropdown
        ui.add_space(4.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, 18.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 0.0, BORDER);
        ui.add_space(4.0);

        let sig_btn = ui.add(
            egui::Button::new(
                RichText::new("Technical Signal ▾").size(11.0).color(TEXT_COLOR),
            )
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::NONE)
            .min_size(Vec2::new(0.0, 22.0)),
        );
        if sig_btn.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        (toggled, open_modal)
    }
}
