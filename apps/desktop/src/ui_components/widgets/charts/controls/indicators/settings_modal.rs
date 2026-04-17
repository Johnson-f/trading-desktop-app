use eframe::egui::{self, Color32, CornerRadius, DragValue, RichText, Stroke, Vec2};

use super::super::super::indicators::{self, IndicatorManager, ParamKind, ParamValue, ParamValues};

const MODAL_BG: Color32 = Color32::from_rgb(30, 30, 34);
const SECTION_BG: Color32 = Color32::from_rgb(36, 36, 40);
const BORDER: Color32 = Color32::from_rgb(50, 50, 55);
const TEXT_WHITE: Color32 = Color32::from_rgb(240, 240, 242);
const TEXT_MUTED: Color32 = Color32::from_rgb(140, 140, 150);
const ACCENT: Color32 = Color32::from_rgb(56, 139, 253);
const ACCENT_UNDERLINE: Color32 = Color32::from_rgb(88, 166, 255);
const INPUT_BG: Color32 = Color32::from_rgb(24, 24, 28);
const OUTLINE_BTN: Color32 = Color32::from_rgb(70, 70, 78);

#[derive(Clone, Copy, PartialEq)]
enum SettingsTab {
    Inputs,
    Style,
    Introduction,
}

pub struct SettingsModal {
    pub open: bool,
    pub instance_id: u64,
    active_tab: SettingsTab,
    draft: ParamValues,
}

impl Default for SettingsModal {
    fn default() -> Self {
        Self {
            open: false,
            instance_id: 0,
            active_tab: SettingsTab::Inputs,
            draft: ParamValues(Default::default()),
        }
    }
}

impl SettingsModal {
    pub fn open_for(&mut self, instance_id: u64, params: ParamValues) {
        self.open = true;
        self.instance_id = instance_id;
        self.active_tab = SettingsTab::Inputs;
        self.draft = params;
    }

    pub fn show(&mut self, ctx: &egui::Context, manager: &mut IndicatorManager) {
        if !self.open {
            return;
        }

        let Some((def_id, display_name)) = manager
            .active
            .iter()
            .find(|a| a.instance_id == self.instance_id)
            .map(|a| (a.def_id, a.indicator().display_name(&a.params)))
        else {
            self.open = false;
            return;
        };
        let Some(def) = indicators::get(def_id) else {
            self.open = false;
            return;
        };

        let modal_size = Vec2::new(560.0, 640.0);
        let center = ctx.content_rect().center() - modal_size / 2.0;

        let mut should_apply = false;
        let mut should_reset = false;
        let mut should_close = false;

        egui::Window::new("Indicator Settings")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .fixed_size(modal_size)
            .default_pos(egui::Pos2::new(center.x, center.y))
            .frame(
                egui::Frame::new()
                    .fill(MODAL_BG)
                    .stroke(Stroke::new(1.0, BORDER))
                    .corner_radius(CornerRadius::same(10)),
            )
            .show(ctx, |ui| {
                ui.style_mut().animation_time = 0.0;

                // Header
                egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(16, 14))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if close_dot(ui).clicked() {
                                should_close = true;
                            }
                            ui.centered_and_justified(|ui| {
                                ui.label(
                                    RichText::new("Indicators Settings")
                                        .color(TEXT_WHITE)
                                        .size(14.0)
                                        .strong(),
                                );
                            });
                        });
                    });

                // Indicator name row
                egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(20, 10))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(format!("{}: {}", def.name, display_name))
                                .color(TEXT_WHITE)
                                .size(18.0),
                        );
                    });

                // Tabs
                egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(20, 10))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            self.draw_tab(ui, SettingsTab::Inputs, "Inputs");
                            ui.add_space(14.0);
                            self.draw_tab(ui, SettingsTab::Style, "Style");
                            ui.add_space(14.0);
                            self.draw_tab(ui, SettingsTab::Introduction, "Introduction");
                        });
                    });

                // Body
                let body_height = ui.available_height() - 70.0;
                egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(20, 18))
                    .show(ui, |ui| {
                        ui.set_min_height(body_height);
                        match self.active_tab {
                            SettingsTab::Inputs => draw_inputs(ui, def.params, &mut self.draft),
                            SettingsTab::Style => draw_style(ui, def.params, &mut self.draft),
                            SettingsTab::Introduction => draw_introduction(ui, def.description),
                        }
                    });

                // Footer (pinned at bottom via fixed-size window)
                ui.allocate_ui_with_layout(
                    Vec2::new(ui.available_width(), 56.0),
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        ui.add_space(16.0);
                        let done = ui.add(
                            egui::Button::new(RichText::new("Done").color(TEXT_WHITE).size(12.0))
                                .fill(ACCENT)
                                .corner_radius(CornerRadius::same(6))
                                .min_size(Vec2::new(82.0, 32.0)),
                        );
                        if done.clicked() {
                            should_apply = true;
                        }

                        ui.add_space(8.0);

                        let reset = ui.add(
                            egui::Button::new(
                                RichText::new("Reset to Default").color(TEXT_WHITE).size(12.0),
                            )
                            .fill(SECTION_BG)
                            .stroke(Stroke::new(1.0, OUTLINE_BTN))
                            .corner_radius(CornerRadius::same(6))
                            .min_size(Vec2::new(130.0, 32.0)),
                        );
                        if reset.clicked() {
                            should_reset = true;
                        }
                    },
                );
            });

        if should_reset {
            self.draft = def.params.defaults();
        }
        if should_apply {
            manager.update_params(self.instance_id, self.draft.clone());
            self.open = false;
        }
        if should_close {
            self.open = false;
        }
    }

    fn draw_tab(&mut self, ui: &mut egui::Ui, tab: SettingsTab, label: &str) {
        let is_active = self.active_tab == tab;
        let color = if is_active { ACCENT_UNDERLINE } else { TEXT_MUTED };
        let btn = ui.add(
            egui::Button::new(RichText::new(label).size(12.0).color(color))
                .fill(Color32::TRANSPARENT)
                .stroke(Stroke::NONE),
        );
        if btn.clicked() {
            self.active_tab = tab;
        }
        if btn.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if is_active {
            let rect = btn.rect;
            let underline = egui::Rect::from_min_size(
                egui::Pos2::new(rect.left(), rect.bottom()),
                Vec2::new(rect.width(), 2.0),
            );
            ui.painter().rect_filled(underline, 0.0, ACCENT_UNDERLINE);
        }
    }
}

fn draw_inputs(ui: &mut egui::Ui, schema: indicators::ParamSchema, draft: &mut ParamValues) {
    let fields: Vec<_> = schema
        .fields
        .iter()
        .filter(|f| matches!(f.kind, ParamKind::Int { .. } | ParamKind::Float { .. }))
        .collect();
    if fields.is_empty() {
        draw_empty(ui, "This indicator has no numeric inputs.");
        return;
    }
    for field in fields {
        ui.horizontal(|ui| {
            ui.label(RichText::new(field.label).color(TEXT_WHITE).size(13.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                match field.kind {
                    ParamKind::Int { min, max, .. } => {
                        let current = match draft.0.get(field.key) {
                            Some(ParamValue::Int(v)) => *v,
                            _ => 0,
                        };
                        let mut v = current;
                        egui::Frame::new()
                            .fill(INPUT_BG)
                            .stroke(Stroke::new(1.0, BORDER))
                            .corner_radius(CornerRadius::same(4))
                            .inner_margin(egui::Margin::symmetric(12, 6))
                            .show(ui, |ui| {
                                ui.set_min_width(160.0);
                                ui.add(DragValue::new(&mut v).range(min..=max).speed(1));
                            });
                        if v != current {
                            draft.0.insert(field.key, ParamValue::Int(v));
                        }
                    }
                    ParamKind::Float { min, max, .. } => {
                        let current = match draft.0.get(field.key) {
                            Some(ParamValue::Float(v)) => *v,
                            _ => 0.0,
                        };
                        let mut v = current;
                        egui::Frame::new()
                            .fill(INPUT_BG)
                            .stroke(Stroke::new(1.0, BORDER))
                            .corner_radius(CornerRadius::same(4))
                            .inner_margin(egui::Margin::symmetric(12, 6))
                            .show(ui, |ui| {
                                ui.set_min_width(160.0);
                                ui.add(DragValue::new(&mut v).range(min..=max).speed(0.01));
                            });
                        if (v - current).abs() > f32::EPSILON {
                            draft.0.insert(field.key, ParamValue::Float(v));
                        }
                    }
                    ParamKind::Color { .. } => {}
                }
            });
        });
        ui.add_space(12.0);
    }
}

fn draw_style(ui: &mut egui::Ui, schema: indicators::ParamSchema, draft: &mut ParamValues) {
    let fields: Vec<_> = schema
        .fields
        .iter()
        .filter(|f| matches!(f.kind, ParamKind::Color { .. }))
        .collect();
    if fields.is_empty() {
        draw_empty(ui, "This indicator has no style options.");
        return;
    }
    use super::super::super::util::{core_color, egui_color};
    for field in fields {
        ui.horizontal(|ui| {
            ui.label(RichText::new(field.label).color(TEXT_WHITE).size(13.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let current = match draft.0.get(field.key) {
                    Some(ParamValue::Color(c)) => egui_color(*c),
                    _ => Color32::WHITE,
                };
                let mut c = current;
                if egui::color_picker::color_edit_button_srgba(
                    ui,
                    &mut c,
                    egui::color_picker::Alpha::OnlyBlend,
                )
                .changed()
                {
                    draft.0.insert(field.key, ParamValue::Color(core_color(c)));
                }
            });
        });
        ui.add_space(12.0);
    }
}

fn draw_introduction(ui: &mut egui::Ui, description: &str) {
    ui.label(
        RichText::new(description)
            .color(TEXT_MUTED)
            .size(13.0),
    );
}

fn draw_empty(ui: &mut egui::Ui, msg: &str) {
    ui.vertical_centered(|ui| {
        ui.add_space(40.0);
        ui.label(RichText::new(msg).color(TEXT_MUTED).size(12.0));
    });
}

/// Paint a macOS-style red close dot. Uses `painter.circle_filled` so it
/// doesn't depend on the current font having a filled bullet glyph.
fn close_dot(ui: &mut egui::Ui) -> egui::Response {
    let diameter = 12.0;
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(diameter, diameter),
        egui::Sense::click(),
    );
    let base = Color32::from_rgb(255, 95, 87);
    let hovered = Color32::from_rgb(225, 80, 72);
    let color = if response.hovered() { hovered } else { base };
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    ui.painter().circle_filled(rect.center(), diameter / 2.0, color);
    response
}
