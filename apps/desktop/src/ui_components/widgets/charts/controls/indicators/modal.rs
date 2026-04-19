use std::collections::HashSet;

use eframe::egui::{self, Color32, CornerRadius, RichText, Stroke, Vec2};

use super::super::super::indicators::{self, IndicatorManager, ParamValues, RenderTarget};
use super::params_popover::{ParamsResponse, show as show_params_popover};

const MODAL_BG: Color32 = Color32::from_rgb(30, 30, 34);
const BORDER: Color32 = Color32::from_rgb(50, 50, 55);
const TEXT_WHITE: Color32 = Color32::from_rgb(240, 240, 242);
const TEXT_MUTED: Color32 = Color32::from_rgb(120, 120, 130);
const ACCENT: Color32 = Color32::from_rgb(78, 205, 196);
const STAR_ACTIVE: Color32 = Color32::from_rgb(255, 193, 7);
const STAR_INACTIVE: Color32 = Color32::from_rgb(80, 80, 90);
const SIDEBAR_BG: Color32 = Color32::from_rgb(34, 34, 38);

pub enum ModalEditor {
    Add {
        def_id: &'static str,
        draft: ParamValues,
    },
    Edit {
        instance_id: u64,
        def_id: &'static str,
        draft: ParamValues,
    },
}

pub struct IndicatorModal {
    pub open: bool,
    pub search_query: String,
    pub active_tab: usize,
    pub favorited: HashSet<&'static str>,
    pub editor: Option<ModalEditor>,
}

impl Default for IndicatorModal {
    fn default() -> Self {
        let mut favorited = HashSet::new();
        favorited.insert("ema");
        favorited.insert("volume");
        Self {
            open: false,
            search_query: String::new(),
            active_tab: 1,
            favorited,
            editor: None,
        }
    }
}

impl IndicatorModal {
    pub fn show(&mut self, ctx: &egui::Context, manager: &mut IndicatorManager) {
        if !self.open {
            return;
        }

        let screen_rect = ctx.content_rect();
        let modal_size = Vec2::new(720.0, 560.0);
        let center = screen_rect.center() - modal_size / 2.0;

        egui::Window::new("Indicators")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .fixed_size(modal_size)
            .default_pos(egui::Pos2::new(center.x, center.y))
            .frame(
                egui::Frame::new()
                    .fill(MODAL_BG)
                    .stroke(Stroke::new(1.0, BORDER))
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(egui::Margin::symmetric(16, 12)),
            )
            .show(ctx, |ui| {
                ui.set_min_height(500.0);
                ui.style_mut().animation_time = 0.0;

                self.draw_header(ui);
                ui.add_space(4.0);
                self.draw_tabs(ui);
                ui.add_space(8.0);

                let content_height = ui.available_height();
                ui.horizontal(|ui| {
                    ui.set_height(content_height);

                    ui.vertical(|ui| {
                        ui.set_width(ui.available_width() - 220.0);
                        ui.set_height(content_height);
                        self.draw_search(ui);
                        ui.add_space(8.0);
                        self.draw_column_headers(ui);
                        ui.add_space(4.0);
                        self.draw_indicator_list(ui, manager);
                    });

                    ui.vertical(|ui| {
                        ui.set_width(210.0);
                        self.draw_added_sidebar(ui, manager);
                    });
                });
            });
    }

    fn draw_header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let close = ui.add(
                egui::Button::new(
                    RichText::new("●")
                        .color(Color32::from_rgb(255, 95, 87))
                        .size(12.0),
                )
                .fill(Color32::TRANSPARENT)
                .stroke(Stroke::NONE),
            );
            if close.clicked() {
                self.open = false;
                self.editor = None;
            }
            ui.centered_and_justified(|ui| {
                ui.label(
                    RichText::new("Indicators")
                        .color(TEXT_WHITE)
                        .size(14.0)
                        .strong(),
                );
            });
        });
    }

    fn draw_tabs(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            let tabs = ["Favorites", "All Indicators", "My Indicators"];
            for (i, tab) in tabs.iter().enumerate() {
                let is_active = self.active_tab == i;
                let color = if is_active { ACCENT } else { TEXT_MUTED };
                let btn = ui.add(
                    egui::Button::new(RichText::new(*tab).size(11.0).color(color))
                        .fill(Color32::TRANSPARENT)
                        .stroke(Stroke::NONE),
                );
                if btn.clicked() {
                    self.active_tab = i;
                }
                if btn.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if is_active {
                    let rect = btn.rect;
                    let underline = egui::Rect::from_min_size(
                        egui::Pos2::new(rect.left(), rect.bottom() - 2.0),
                        Vec2::new(rect.width(), 2.0),
                    );
                    ui.painter().rect_filled(underline, 0.0, ACCENT);
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new("＋ New Indicator").size(11.0).color(ACCENT));
            });
        });
    }

    fn draw_search(&mut self, ui: &mut egui::Ui) {
        ui.scope(|ui| {
            let style = ui.style_mut();
            style.visuals.extreme_bg_color = Color32::from_rgb(20, 20, 24);
            style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(20, 20, 24);
            style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
            style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(20, 20, 24);
            style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT);
            style.visuals.widgets.active.bg_fill = Color32::from_rgb(20, 20, 24);
            style.visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
            style.visuals.selection.bg_fill = Color32::from_rgb(33, 33, 54);

            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(egui_phosphor::regular::MAGNIFYING_GLASS)
                        .size(14.0)
                        .color(TEXT_MUTED),
                );
                let te = egui::TextEdit::singleline(&mut self.search_query)
                    .hint_text(RichText::new("Search").color(TEXT_MUTED))
                    .desired_width(ui.available_width() - 30.0)
                    .text_color(TEXT_WHITE);
                ui.add(te);
            });
        });
    }

    fn draw_column_headers(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Indicator Name").size(10.0).color(TEXT_MUTED));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(60.0);
                ui.label(RichText::new("Likes ▾").size(10.0).color(TEXT_MUTED));
            });
        });
    }

    fn draw_indicator_list(&mut self, ui: &mut egui::Ui, manager: &mut IndicatorManager) {
        let query = self.search_query.to_lowercase();
        egui::ScrollArea::vertical().show(ui, |ui| {
            for def in indicators::all() {
                if !query.is_empty() && !def.name.to_lowercase().contains(&query) {
                    continue;
                }
                if self.active_tab == 0 && !self.favorited.contains(def.id) {
                    continue;
                }

                let row_frame = egui::Frame::new()
                    .fill(Color32::TRANSPARENT)
                    .inner_margin(egui::Margin::symmetric(0, 4))
                    .stroke(Stroke::NONE);

                let row_resp = row_frame.show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.set_min_height(28.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(def.name).size(12.0).color(TEXT_WHITE));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let fav = self.favorited.contains(def.id);
                            let star_color = if fav { STAR_ACTIVE } else { STAR_INACTIVE };
                            let star_btn = ui.add(
                                egui::Button::new(
                                    RichText::new(egui_phosphor::regular::STAR)
                                        .size(14.0)
                                        .color(star_color),
                                )
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::NONE),
                            );
                            if star_btn.clicked() {
                                if fav {
                                    self.favorited.remove(def.id);
                                } else {
                                    self.favorited.insert(def.id);
                                }
                            }

                            let add_btn = ui.add(
                                egui::Button::new(
                                    RichText::new(egui_phosphor::regular::PLUS_CIRCLE)
                                        .size(14.0)
                                        .color(TEXT_MUTED),
                                )
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::NONE),
                            );
                            if add_btn.clicked() {
                                self.editor = Some(ModalEditor::Add {
                                    def_id: def.id,
                                    draft: def.params.defaults(),
                                });
                            }

                            ui.label(
                                RichText::new(format!("{}", def.likes))
                                    .size(11.0)
                                    .color(TEXT_MUTED),
                            );
                        });
                    });
                });

                let row_rect = row_resp.response.rect;
                let sep_rect = egui::Rect::from_min_size(
                    egui::Pos2::new(row_rect.left(), row_rect.bottom()),
                    Vec2::new(row_rect.width(), 1.0),
                );
                ui.painter()
                    .rect_filled(sep_rect, 0.0, Color32::from_rgb(40, 40, 44));
            }
        });

        self.maybe_draw_editor(ui, manager);
    }

    fn maybe_draw_editor(&mut self, ui: &mut egui::Ui, manager: &mut IndicatorManager) {
        let Some(editor) = self.editor.as_mut() else {
            return;
        };
        ui.add_space(8.0);
        let (schema, confirm_label) = match editor {
            ModalEditor::Add { def_id, .. } => {
                let def = indicators::get(def_id).expect("editor def_id must exist");
                (def.params, "Add to chart")
            }
            ModalEditor::Edit { def_id, .. } => {
                let def = indicators::get(def_id).expect("editor def_id must exist");
                (def.params, "Save")
            }
        };

        let draft: &mut ParamValues = match editor {
            ModalEditor::Add { draft, .. } => draft,
            ModalEditor::Edit { draft, .. } => draft,
        };

        match show_params_popover(ui, &schema, draft, confirm_label) {
            ParamsResponse::Confirm => {
                let editor_taken = self.editor.take().unwrap();
                match editor_taken {
                    ModalEditor::Add { def_id, draft } => {
                        manager.add(def_id, draft);
                    }
                    ModalEditor::Edit {
                        instance_id, draft, ..
                    } => {
                        manager.update_params(instance_id, draft);
                    }
                }
            }
            ParamsResponse::Cancel => {
                self.editor = None;
            }
            ParamsResponse::Open => {}
        }
    }

    fn draw_added_sidebar(&mut self, ui: &mut egui::Ui, manager: &mut IndicatorManager) {
        egui::Frame::new()
            .fill(SIDEBAR_BG)
            .corner_radius(CornerRadius::same(4))
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                let count = manager.active.len();
                ui.label(
                    RichText::new(format!("Added Indicators ({})", count))
                        .size(11.0)
                        .color(TEXT_WHITE)
                        .strong(),
                );
                ui.add_space(8.0);

                let main_ids: Vec<(u64, &'static str, String)> = manager
                    .active
                    .iter()
                    .filter(|a| a.indicator().target() == RenderTarget::MainOverlay)
                    .map(|a| {
                        (
                            a.instance_id,
                            a.def_id,
                            a.indicator().display_name(&a.params),
                        )
                    })
                    .collect();
                let sub_ids: Vec<(u64, &'static str, String)> = manager
                    .active
                    .iter()
                    .filter(|a| a.indicator().target() == RenderTarget::SubPane)
                    .map(|a| {
                        (
                            a.instance_id,
                            a.def_id,
                            a.indicator().display_name(&a.params),
                        )
                    })
                    .collect();

                let mut to_edit: Option<(u64, &'static str, ParamValues)> = None;
                let mut to_remove: Option<u64> = None;

                if !main_ids.is_empty() {
                    ui.label(RichText::new("Main Chart").size(10.0).color(TEXT_MUTED));
                    ui.add_space(4.0);
                    for (id, def_id, label) in &main_ids {
                        draw_sidebar_row(
                            ui,
                            label,
                            &mut to_edit,
                            &mut to_remove,
                            *id,
                            def_id,
                            manager,
                        );
                    }
                    ui.add_space(8.0);
                }

                if !sub_ids.is_empty() {
                    ui.label(RichText::new("Sub Chart").size(10.0).color(TEXT_MUTED));
                    ui.add_space(4.0);
                    for (id, def_id, label) in &sub_ids {
                        draw_sidebar_row(
                            ui,
                            label,
                            &mut to_edit,
                            &mut to_remove,
                            *id,
                            def_id,
                            manager,
                        );
                    }
                }

                if let Some(id) = to_remove {
                    manager.remove(id);
                }
                if let Some((id, def_id, draft)) = to_edit {
                    self.editor = Some(ModalEditor::Edit {
                        instance_id: id,
                        def_id,
                        draft,
                    });
                }
            });
    }
}

fn draw_sidebar_row(
    ui: &mut egui::Ui,
    label: &str,
    to_edit: &mut Option<(u64, &'static str, ParamValues)>,
    to_remove: &mut Option<u64>,
    instance_id: u64,
    def_id: &'static str,
    manager: &IndicatorManager,
) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(11.0).color(TEXT_WHITE));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let del = ui.add(
                egui::Button::new(
                    RichText::new(egui_phosphor::regular::X)
                        .size(12.0)
                        .color(TEXT_MUTED),
                )
                .fill(Color32::TRANSPARENT)
                .stroke(Stroke::NONE),
            );
            if del.clicked() {
                *to_remove = Some(instance_id);
            }
            let gear = ui.add(
                egui::Button::new(
                    RichText::new(egui_phosphor::regular::GEAR)
                        .size(12.0)
                        .color(TEXT_MUTED),
                )
                .fill(Color32::TRANSPARENT)
                .stroke(Stroke::NONE),
            );
            if gear.clicked() {
                if let Some(a) = manager.active.iter().find(|a| a.instance_id == instance_id) {
                    *to_edit = Some((instance_id, def_id, a.params.clone()));
                }
            }
        });
    });
    ui.add_space(2.0);
}
