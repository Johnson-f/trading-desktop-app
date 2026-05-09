use eframe::egui::{self, Color32, CornerRadius, DragValue, RichText, Stroke, Vec2};

use super::super::super::indicators::{self, IndicatorManager, ParamKind, ParamValue, ParamValues};

const BORDER: Color32 = Color32::from_rgb(50, 50, 55);
const TEXT_WHITE: Color32 = Color32::from_rgb(240, 240, 242);
const TEXT_MUTED: Color32 = Color32::from_rgb(140, 140, 150);
const ACCENT_UNDERLINE: Color32 = Color32::from_rgb(88, 166, 255);
const INPUT_BG: Color32 = Color32::from_rgb(24, 24, 28);

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

    pub fn show(&mut self, ui: &mut egui::Ui, manager: &mut IndicatorManager) {
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

        let theme = crate::shadcn_theme::theme();

        let mut should_reset = false;
        let mut should_close = false;
        let mut changed = false;
        // Read self.open into a local so we can pass `&mut local` to
        // DialogProps without conflicting with the body's mutable borrow
        // of `self`.
        let mut open = self.open;

        egui_shadcn::dialog(
            ui,
            theme,
            egui_shadcn::DialogProps::new(egui::Id::new("indicator_settings_modal"), &mut open)
                .title("Indicator Settings")
                .width(560.0)
                .height(640.0)
                .scrollable(false),
            |ui| {
                // Indicator name row
                egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(4, 6))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(format!("{}: {}", def.name, display_name))
                                .color(TEXT_WHITE)
                                .size(18.0),
                        );
                    });

                // Tabs
                egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(4, 8))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            self.draw_tab(ui, SettingsTab::Inputs, "Inputs");
                            ui.add_space(14.0);
                            self.draw_tab(ui, SettingsTab::Style, "Style");
                            ui.add_space(14.0);
                            self.draw_tab(ui, SettingsTab::Introduction, "Introduction");
                        });
                    });

                // Body — scroll_area wraps the active tab content so long
                // forms (many params, long Introduction text) scroll inside
                // the modal instead of pushing the footer out of view.
                let body_height = (ui.available_height() - 70.0).max(0.0);
                egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(4, 12))
                    .show(ui, |ui| {
                        ui.set_min_height(body_height);
                        let scroll_props = egui_shadcn::ScrollAreaProps::default()
                            .id(ui.make_persistent_id("indicator_settings_scroll"))
                            .auto_shrink([false; 2]);
                        egui_shadcn::scroll_area(ui, theme, scroll_props, |ui| {
                            match self.active_tab {
                                SettingsTab::Inputs => {
                                    changed |= draw_inputs(ui, def.params, &mut self.draft);
                                }
                                SettingsTab::Style => {
                                    changed |= draw_style(ui, def.params, &mut self.draft);
                                }
                                SettingsTab::Introduction => {
                                    draw_introduction(ui, def.description)
                                }
                            }
                        });
                    });

                // Footer
                ui.allocate_ui_with_layout(
                    Vec2::new(ui.available_width(), 48.0),
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        let done = egui_shadcn::Button::new(
                            RichText::new("Done").color(TEXT_WHITE).size(12.0),
                        )
                        .variant(egui_shadcn::ButtonVariant::Default)
                        .show(ui, crate::shadcn_theme::theme());
                        if done.clicked() {
                            should_close = true;
                        }

                        ui.add_space(8.0);

                        let reset = egui_shadcn::Button::new(
                            RichText::new("Reset to Default")
                                .color(TEXT_WHITE)
                                .size(12.0),
                        )
                        .variant(egui_shadcn::ButtonVariant::Outline)
                        .show(ui, crate::shadcn_theme::theme());
                        if reset.clicked() {
                            should_reset = true;
                        }
                    },
                );
            },
        );

        // Sync `open` back: dialog may have toggled it via the close
        // button / scrim / Esc. Combine with the explicit Done click.
        self.open = open && !should_close;

        if should_reset {
            self.draft = def.params.defaults();
            changed = true;
        }
        if changed {
            manager.update_params(self.instance_id, self.draft.clone());
        }
    }

    fn draw_tab(&mut self, ui: &mut egui::Ui, tab: SettingsTab, label: &str) {
        let is_active = self.active_tab == tab;
        let color = if is_active {
            ACCENT_UNDERLINE
        } else {
            TEXT_MUTED
        };
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

fn draw_inputs(
    ui: &mut egui::Ui,
    schema: indicators::ParamSchema,
    draft: &mut ParamValues,
) -> bool {
    let fields: Vec<_> = schema
        .fields
        .iter()
        .filter(|f| matches!(f.kind, ParamKind::Int { .. } | ParamKind::Float { .. }))
        .collect();
    if fields.is_empty() {
        draw_empty(ui, "This indicator has no numeric inputs.");
        return false;
    }
    let mut changed = false;
    for field in fields {
        ui.horizontal(|ui| {
            ui.label(RichText::new(field.label).color(TEXT_WHITE).size(13.0));
            ui.with_layout(
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| match field.kind {
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
                            changed = true;
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
                            changed = true;
                        }
                    }
                    ParamKind::Color { .. } => {}
                },
            );
        });
        ui.add_space(12.0);
    }
    changed
}

fn draw_style(ui: &mut egui::Ui, schema: indicators::ParamSchema, draft: &mut ParamValues) -> bool {
    let fields: Vec<_> = schema
        .fields
        .iter()
        .filter(|f| matches!(f.kind, ParamKind::Color { .. }))
        .collect();
    if fields.is_empty() {
        draw_empty(ui, "This indicator has no style options.");
        return false;
    }
    use super::super::super::util::{core_color, egui_color};
    let mut changed = false;
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
                    changed = true;
                }
            });
        });
        ui.add_space(12.0);
    }
    changed
}

fn draw_introduction(ui: &mut egui::Ui, description: &str) {
    ui.label(RichText::new(description).color(TEXT_MUTED).size(13.0));
}

fn draw_empty(ui: &mut egui::Ui, msg: &str) {
    ui.vertical_centered(|ui| {
        ui.add_space(40.0);
        ui.label(RichText::new(msg).color(TEXT_MUTED).size(12.0));
    });
}

