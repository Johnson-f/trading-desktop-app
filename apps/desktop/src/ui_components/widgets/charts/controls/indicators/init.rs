use eframe::egui::{self, Color32, CornerRadius, RichText, Stroke, Vec2};

use super::super::super::indicators::IndicatorManager;

const ACCENT: Color32 = Color32::from_rgb(78, 205, 196);
const TEXT_COLOR: Color32 = Color32::from_rgb(180, 180, 190);
const TEXT_HOVER: Color32 = Color32::from_rgb(240, 240, 242);
const HOVER_BG: Color32 = Color32::from_rgb(28, 28, 32);
const BORDER: Color32 = Color32::from_rgb(30, 30, 33);
const CHIP_BG: Color32 = Color32::from_rgb(35, 35, 40);

pub enum IndicatorBarEvent {
    OpenModal,
    Remove(u64),
}

#[derive(Default)]
pub struct IndicatorBar;

impl IndicatorBar {
    pub fn show_inline(
        &self,
        ui: &mut egui::Ui,
        manager: &IndicatorManager,
    ) -> Option<IndicatorBarEvent> {
        let mut event: Option<IndicatorBarEvent> = None;

        ui.style_mut().animation_time = 0.0;
        ui.add_space(4.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, 18.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 0.0, BORDER);
        ui.add_space(4.0);

        for a in &manager.active {
            let label = a.indicator().display_name(&a.params);
            let chip = ui.add(
                egui::Button::new(
                    RichText::new(&label).size(11.0).color(ACCENT).strong(),
                )
                .fill(CHIP_BG)
                .corner_radius(CornerRadius::same(3))
                .stroke(Stroke::NONE)
                .min_size(Vec2::new(0.0, 22.0)),
            );
            if chip.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }

            let close = ui.add(
                egui::Button::new(
                    RichText::new(egui_phosphor::regular::X).size(11.0).color(TEXT_COLOR),
                )
                .fill(Color32::TRANSPARENT)
                .stroke(Stroke::NONE)
                .min_size(Vec2::new(0.0, 22.0)),
            );
            if close.clicked() && event.is_none() {
                event = Some(IndicatorBarEvent::Remove(a.instance_id));
            }
            ui.add_space(2.0);
        }

        ui.add_space(4.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, 18.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 0.0, BORDER);
        ui.add_space(4.0);

        let all_btn = ui.add(
            egui::Button::new(RichText::new("All Indicators").size(11.0).color(TEXT_COLOR))
                .fill(Color32::TRANSPARENT)
                .stroke(Stroke::NONE)
                .min_size(Vec2::new(0.0, 22.0)),
        );
        if all_btn.hovered() {
            let rect = all_btn.rect;
            ui.painter().rect_filled(rect, 3.0, HOVER_BG);
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "All Indicators",
                egui::FontId::proportional(11.0),
                TEXT_HOVER,
            );
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if all_btn.clicked() && event.is_none() {
            event = Some(IndicatorBarEvent::OpenModal);
        }

        ui.add_space(4.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, 18.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 0.0, BORDER);
        ui.add_space(4.0);
        let sig_btn = ui.add(
            egui::Button::new(RichText::new("Technical Signal ▾").size(11.0).color(TEXT_COLOR))
                .fill(Color32::TRANSPARENT)
                .stroke(Stroke::NONE)
                .min_size(Vec2::new(0.0, 22.0)),
        );
        if sig_btn.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        event
    }
}
