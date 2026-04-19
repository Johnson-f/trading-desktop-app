use eframe::egui::{self, Color32, RichText, Stroke, Vec2};

use super::super::super::indicators::{self, IndicatorManager};

const ACTIVE_BLUE: Color32 = Color32::from_rgb(88, 166, 255);
const TEXT_COLOR: Color32 = Color32::from_rgb(180, 180, 190);
const TEXT_HOVER: Color32 = Color32::from_rgb(240, 240, 242);
const HOVER_BG: Color32 = Color32::from_rgb(28, 28, 32);
const BORDER: Color32 = Color32::from_rgb(30, 30, 33);

pub enum IndicatorBarEvent {
    OpenModal,
    /// Kept for API compatibility but no longer emitted — removal now happens
    /// through the chart legend or via toggling a shortcut off.
    #[allow(dead_code)]
    Remove(u64),
}

#[derive(Default)]
pub struct IndicatorBar;

impl IndicatorBar {
    /// Render the inline shortcut row. Each chip is one def_id from the
    /// manager's recents list; clicking toggles that family on/off.
    pub fn show_inline(
        &self,
        ui: &mut egui::Ui,
        manager: &mut IndicatorManager,
    ) -> Option<IndicatorBarEvent> {
        let mut event: Option<IndicatorBarEvent> = None;

        ui.style_mut().animation_time = 0.0;
        ui.add_space(4.0);
        draw_separator(ui);
        ui.add_space(4.0);

        // Snapshot recents first to release the borrow before mutating.
        let recents: Vec<&'static str> = manager.recents.iter().copied().collect();
        for def_id in recents {
            let Some(def) = indicators::get(def_id) else {
                continue;
            };
            let is_active = manager.has_any_of(def_id);
            if draw_shortcut(ui, def.short_name, is_active) {
                if is_active {
                    manager.remove_all_of(def_id);
                } else {
                    manager.restore_or_add_default(def_id);
                }
            }
        }

        ui.add_space(4.0);
        draw_separator(ui);
        ui.add_space(4.0);

        if draw_text_button(ui, "All Indicators") && event.is_none() {
            event = Some(IndicatorBarEvent::OpenModal);
        }

        ui.add_space(4.0);
        draw_separator(ui);
        ui.add_space(4.0);
        // "Technical Signal ▾" — placeholder; wiring deferred.
        draw_text_button(ui, "Technical Signal ▾");

        event
    }
}

fn draw_shortcut(ui: &mut egui::Ui, label: &str, is_active: bool) -> bool {
    let color = if is_active { ACTIVE_BLUE } else { TEXT_COLOR };
    let btn = ui.add(
        egui::Button::new(RichText::new(label).size(12.0).color(color).strong())
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::NONE)
            .min_size(Vec2::new(0.0, 22.0)),
    );
    if btn.hovered() {
        let rect = btn.rect;
        ui.painter().rect_filled(rect, 3.0, HOVER_BG);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(12.0),
            if is_active { ACTIVE_BLUE } else { TEXT_HOVER },
        );
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    btn.clicked()
}

fn draw_text_button(ui: &mut egui::Ui, label: &str) -> bool {
    let btn = ui.add(
        egui::Button::new(RichText::new(label).size(11.0).color(TEXT_COLOR))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::NONE)
            .min_size(Vec2::new(0.0, 22.0)),
    );
    if btn.hovered() {
        let rect = btn.rect;
        ui.painter().rect_filled(rect, 3.0, HOVER_BG);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(11.0),
            TEXT_HOVER,
        );
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    btn.clicked()
}

fn draw_separator(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, 18.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 0.0, BORDER);
}
