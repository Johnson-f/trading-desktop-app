use eframe::egui::{self, Color32, CornerRadius, DragValue, Pos2, Sense, Stroke, Vec2};

use super::super::super::CandleData;
use super::super::super::drawings::{
    COLOR_PALETTE, CommittedDrawing, DashStyle, DrawingStyle, DrawingsManager, ExtendCapabilities,
    FIB_RATIO_COUNT, KindStyle, WorldPoint,
};
use super::super::super::util::{core_color, egui_color};

const VERTICAL_LINE_ID: &str = "vertical_line";

const MODAL_BG: Color32 = Color32::from_rgb(30, 30, 34);
const SECTION_BG: Color32 = Color32::from_rgb(36, 36, 40);
const BORDER: Color32 = Color32::from_rgb(50, 50, 55);
const TEXT_WHITE: Color32 = Color32::from_rgb(240, 240, 242);
const TEXT_MUTED: Color32 = Color32::from_rgb(140, 140, 150);
const ACCENT: Color32 = Color32::from_rgb(56, 139, 253);
const ACCENT_HOVER: Color32 = Color32::from_rgb(88, 166, 255);
const ACCENT_UNDERLINE: Color32 = Color32::from_rgb(88, 166, 255);
const BTN_OUTLINE: Color32 = Color32::from_rgb(70, 70, 78);
const CLOSE_DOT: Color32 = Color32::from_rgb(255, 95, 86);
const INPUT_BG: Color32 = Color32::from_rgb(24, 24, 28);
const INPUT_HOVER: Color32 = Color32::from_rgb(32, 32, 38);

/// Fixed modal dimensions — stays constant across Style/Position tabs so the
/// window doesn't resize when the user switches tabs.
const MODAL_WIDTH: f32 = 460.0;
const MODAL_HEIGHT: f32 = 420.0;

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Style,
    Position,
}

pub struct DrawingSettingsModal {
    pub open: bool,
    pub drawing_id: Option<u64>,
    draft_style: DrawingStyle,
    draft_kind_style: KindStyle,
    draft_points: Vec<WorldPointEdit>,
    active_tab: Tab,
}

#[derive(Clone, Copy)]
struct WorldPointEdit {
    index: f32,
    price: f32,
}

impl Default for DrawingSettingsModal {
    fn default() -> Self {
        Self {
            open: false,
            drawing_id: None,
            draft_style: DrawingStyle::default(),
            draft_kind_style: KindStyle::None,
            draft_points: Vec::new(),
            active_tab: Tab::Style,
        }
    }
}

impl DrawingSettingsModal {
    pub fn open_for(&mut self, drawing: &CommittedDrawing) {
        self.open = true;
        self.drawing_id = Some(drawing.id);
        self.draft_style = drawing.style;
        self.draft_kind_style = drawing.kind_style;
        self.draft_points = drawing
            .points
            .iter()
            .map(|p| WorldPointEdit {
                index: p.index,
                price: p.price,
            })
            .collect();
        self.active_tab = Tab::Style;
    }

    pub fn show(&mut self, ctx: &egui::Context, manager: &mut DrawingsManager, data: &CandleData) {
        if !self.open {
            return;
        }
        let Some(drawing_id) = self.drawing_id else {
            self.open = false;
            return;
        };
        // Close if the target drawing disappeared (e.g., deleted via Delete key).
        // Closing here is cleaner than letting Apply silently no-op when the
        // user clicks it later.
        let Some(def_id) = manager
            .committed
            .iter()
            .find(|d| d.id == drawing_id)
            .map(|d| d.def_id)
        else {
            self.open = false;
            return;
        };

        let tool = manager.tool_for(def_id);
        let display_name = tool
            .as_ref()
            .map(|t| t.display_name().to_string())
            .unwrap_or_else(|| "Drawing".to_string());
        let extend_caps = tool
            .as_ref()
            .map(|t| t.extend_capabilities())
            .unwrap_or_default();
        let default_kind_style = tool
            .map(|t| t.default_kind_style())
            .unwrap_or(KindStyle::None);

        let modal_size = Vec2::new(MODAL_WIDTH, MODAL_HEIGHT);
        let mut should_close = false;
        let mut should_reset = false;
        let mut changed = false;

        egui::Window::new("Drawing Settings")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .fixed_size(modal_size)
            .frame(
                egui::Frame::new()
                    .fill(MODAL_BG)
                    .stroke(Stroke::new(0.5, BORDER))
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(egui::Margin::same(20)),
            )
            .show(ctx, |ui| {
                // Title row: close-dot on the left, drawing name centered.
                ui.horizontal(|ui| {
                    let (dot_rect, dot_resp) =
                        ui.allocate_exact_size(Vec2::splat(12.0), Sense::click());
                    ui.painter()
                        .circle_filled(dot_rect.center(), 6.0, CLOSE_DOT);
                    if dot_resp.clicked() {
                        should_close = true;
                    }
                    ui.with_layout(
                        egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                        |ui| {
                            ui.label(
                                egui::RichText::new(&display_name)
                                    .color(TEXT_WHITE)
                                    .size(15.0),
                            );
                        },
                    );
                });

                let position_tab_label = if def_id == VERTICAL_LINE_ID {
                    "Date"
                } else {
                    "Price"
                };
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    for (tab, label) in [(Tab::Style, "Style"), (Tab::Position, position_tab_label)]
                    {
                        let resp = ui.selectable_label(self.active_tab == tab, label);
                        if resp.clicked() {
                            self.active_tab = tab;
                        }
                        if self.active_tab == tab {
                            let rect = resp.rect;
                            ui.painter().line_segment(
                                [
                                    Pos2::new(rect.left(), rect.bottom()),
                                    Pos2::new(rect.right(), rect.bottom()),
                                ],
                                Stroke::new(2.0, ACCENT_UNDERLINE),
                            );
                        }
                    }
                });
                ui.add_space(12.0);

                // Content area fills the space between tabs and the action row.
                // Reserve ~56px at the bottom for the action row + its top margin
                // so the frame doesn't overlap the buttons when the modal is
                // tight on height.
                let avail = ui.available_size();
                let content_height = (avail.y - 56.0).max(0.0);
                ui.allocate_ui(Vec2::new(avail.x, content_height), |ui| {
                    egui::Frame::new()
                        .fill(SECTION_BG)
                        .corner_radius(CornerRadius::same(6))
                        .inner_margin(egui::Margin::same(16))
                        .show(ui, |ui| {
                            ui.set_min_size(Vec2::new(ui.available_width(), content_height - 4.0));
                            match self.active_tab {
                                Tab::Style => {
                                    changed |= self.draw_style_tab(ui, extend_caps);
                                }
                                Tab::Position => {
                                    changed |= self.draw_position_tab(ui, def_id, data);
                                }
                            }
                        });
                });

                // Action row, pinned to the bottom: Reset to Defaults (ghost) +
                // Done (accent).
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if outlined_button(ui, "Reset to Defaults").clicked() {
                        should_reset = true;
                    }
                    ui.add_space(8.0);
                    if accent_button(ui, "Done").clicked() {
                        should_close = true;
                    }
                });
            });

        if should_reset {
            self.draft_style = DrawingStyle::default();
            self.draft_kind_style = default_kind_style;
            if let Some(d) = manager.committed.iter().find(|d| d.id == drawing_id) {
                self.draft_points = d
                    .points
                    .iter()
                    .map(|p| WorldPointEdit {
                        index: p.index,
                        price: p.price,
                    })
                    .collect();
            }
            changed = true;
        }
        if changed {
            if let Some(d) = manager.committed.iter_mut().find(|d| d.id == drawing_id) {
                d.style = self.draft_style;
                d.kind_style = self.draft_kind_style;
                d.points = self
                    .draft_points
                    .iter()
                    .map(|p| WorldPoint {
                        index: p.index,
                        price: p.price,
                    })
                    .collect();
            }
            manager.dirty = true;
        }
        if should_close {
            self.open = false;
        }
    }

    fn draw_style_tab(&mut self, ui: &mut egui::Ui, extend_caps: ExtendCapabilities) -> bool {
        let mut changed = false;
        ui.label(egui::RichText::new("Color").color(TEXT_MUTED).size(11.0));
        ui.horizontal_wrapped(|ui| {
            for c in COLOR_PALETTE {
                let size = Vec2::new(24.0, 24.0);
                let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
                ui.painter()
                    .rect_filled(rect, CornerRadius::same(4), egui_color(*c));
                if self.draft_style.color == *c {
                    ui.painter().rect_stroke(
                        rect,
                        CornerRadius::same(4),
                        Stroke::new(2.0, ACCENT_UNDERLINE),
                        egui::StrokeKind::Outside,
                    );
                }
                if resp.clicked() {
                    self.draft_style.color = *c;
                    changed = true;
                }
            }
        });

        ui.add_space(12.0);
        ui.label(egui::RichText::new("Width").color(TEXT_MUTED).size(11.0));
        if ui
            .add(egui::Slider::new(&mut self.draft_style.width, 0.5..=5.0).text("px"))
            .changed()
        {
            changed = true;
        }

        ui.add_space(12.0);
        ui.label(egui::RichText::new("Style").color(TEXT_MUTED).size(11.0));
        ui.horizontal(|ui| {
            for (d, label) in [
                (DashStyle::Solid, "Solid"),
                (DashStyle::Dashed, "Dashed"),
                (DashStyle::Dotted, "Dotted"),
            ] {
                if ui
                    .selectable_label(self.draft_style.dash == d, label)
                    .clicked()
                {
                    self.draft_style.dash = d;
                    changed = true;
                }
            }
        });

        ui.add_space(12.0);
        ui.label(egui::RichText::new("Opacity").color(TEXT_MUTED).size(11.0));
        if ui
            .add(egui::Slider::new(&mut self.draft_style.opacity, 0.0..=1.0))
            .changed()
        {
            changed = true;
        }

        if extend_caps.left || extend_caps.right {
            ui.add_space(12.0);
            ui.label(egui::RichText::new("Extend").color(TEXT_MUTED).size(11.0));
            if extend_caps.left
                && ui
                    .checkbox(&mut self.draft_style.extend_left, "Extend left")
                    .changed()
            {
                changed = true;
            }
            if extend_caps.right
                && ui
                    .checkbox(&mut self.draft_style.extend_right, "Extend right")
                    .changed()
            {
                changed = true;
            }
        }

        changed |= self.draw_kind_style(ui);
        changed
    }

    /// Draw the appearance widgets specific to the current drawing's
    /// `KindStyle` variant. Each branch owns its own layout so adding new
    /// variants doesn't ripple into the base style code above.
    fn draw_kind_style(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;
        match &mut self.draft_kind_style {
            KindStyle::None => {}
            KindStyle::FilledRect {
                fill_enabled,
                fill_alpha,
            } => {
                ui.add_space(12.0);
                ui.label(egui::RichText::new("Fill").color(TEXT_MUTED).size(11.0));
                if ui.checkbox(fill_enabled, "Enable fill").changed() {
                    changed = true;
                }
                if *fill_enabled {
                    let mut a = *fill_alpha as f32;
                    if ui
                        .add(egui::Slider::new(&mut a, 0.0..=255.0).text("alpha"))
                        .changed()
                    {
                        *fill_alpha = a as u8;
                        changed = true;
                    }
                }
            }
            KindStyle::Pitchfork {
                fill_enabled,
                fill_alpha,
            } => {
                ui.add_space(12.0);
                ui.label(
                    egui::RichText::new("Tine fill")
                        .color(TEXT_MUTED)
                        .size(11.0),
                );
                if ui.checkbox(fill_enabled, "Shade between outer tines").changed() {
                    changed = true;
                }
                if *fill_enabled {
                    let mut a = *fill_alpha as f32;
                    if ui
                        .add(egui::Slider::new(&mut a, 0.0..=255.0).text("alpha"))
                        .changed()
                    {
                        *fill_alpha = a as u8;
                        changed = true;
                    }
                }
            }
            KindStyle::Fib {
                ratios_mask,
                show_labels,
            } => {
                ui.add_space(12.0);
                ui.label(egui::RichText::new("Levels").color(TEXT_MUTED).size(11.0));
                ui.horizontal_wrapped(|ui| {
                    const RATIOS: [f32; FIB_RATIO_COUNT] =
                        [0.0, 0.236, 0.382, 0.5, 0.618, 0.786, 1.0];
                    for (i, r) in RATIOS.iter().enumerate() {
                        let mut on = (*ratios_mask >> i) & 1 == 1;
                        if ui
                            .checkbox(&mut on, format!("{:.3}", r))
                            .changed()
                        {
                            let bit = 1u8 << i;
                            if on {
                                *ratios_mask |= bit;
                            } else {
                                *ratios_mask &= !bit;
                            }
                            changed = true;
                        }
                    }
                });
                ui.add_space(8.0);
                if ui.checkbox(show_labels, "Show ratio labels").changed() {
                    changed = true;
                }
            }
            KindStyle::Position {
                profit_color,
                loss_color,
                entry_color,
                show_label,
            } => {
                ui.add_space(12.0);
                ui.label(
                    egui::RichText::new("Zone colors")
                        .color(TEXT_MUTED)
                        .size(11.0),
                );
                let row = |ui: &mut egui::Ui,
                               label: &str,
                               color: &mut zaned_chart_core::Rgba|
                 -> bool {
                    let mut inner_changed = false;
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(label)
                                .color(TEXT_WHITE)
                                .size(12.0),
                        );
                        let mut c = egui_color(*color);
                        if egui::color_picker::color_edit_button_srgba(
                            ui,
                            &mut c,
                            egui::color_picker::Alpha::Opaque,
                        )
                        .changed()
                        {
                            *color = core_color(c);
                            inner_changed = true;
                        }
                    });
                    inner_changed
                };
                changed |= row(ui, "Profit", profit_color);
                changed |= row(ui, "Loss", loss_color);
                changed |= row(ui, "Entry", entry_color);
                ui.add_space(8.0);
                if ui.checkbox(show_label, "Show R:R label").changed() {
                    changed = true;
                }
            }
            KindStyle::LabeledRect { show_label } => {
                ui.add_space(12.0);
                ui.label(egui::RichText::new("Label").color(TEXT_MUTED).size(11.0));
                if ui.checkbox(show_label, "Show measurement label").changed() {
                    changed = true;
                }
            }
        }
        changed
    }

    fn draw_position_tab(&mut self, ui: &mut egui::Ui, def_id: &str, data: &CandleData) -> bool {
        let is_vertical = def_id == VERTICAL_LINE_ID;
        let multi = self.draft_points.len() > 1;
        let label_col_width = 80.0;
        let field_width = 140.0;
        let mut changed = false;

        for (i, p) in self.draft_points.iter_mut().enumerate() {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                let label = if multi {
                    format!("Point {}", i + 1)
                } else {
                    "Position".to_string()
                };
                let (label_rect, _) =
                    ui.allocate_exact_size(Vec2::new(label_col_width, 28.0), Sense::hover());
                ui.painter().text(
                    egui::Pos2::new(label_rect.left(), label_rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    label,
                    egui::FontId::proportional(13.0),
                    TEXT_MUTED,
                );

                if is_vertical {
                    let date = date_for_index(data, p.index);
                    ui.painter().text(
                        egui::Pos2::new(ui.cursor().left() + 8.0, label_rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        date,
                        egui::FontId::monospace(13.0),
                        TEXT_WHITE,
                    );
                    ui.add_space(field_width);
                } else {
                    ui.add_space(8.0);
                    // `centered_and_justified` sets horizontal_align = Center;
                    // DragValue's edit-mode TextEdit inherits that, so the
                    // number stays centered while editing instead of snapping
                    // to the left edge.
                    let resp = ui
                        .allocate_ui_with_layout(
                            Vec2::new(field_width, 28.0),
                            egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                            |ui| {
                                let vis = &mut ui.visuals_mut();
                                vis.extreme_bg_color = INPUT_BG;
                                vis.widgets.inactive.weak_bg_fill = INPUT_BG;
                                vis.widgets.inactive.bg_fill = INPUT_BG;
                                vis.widgets.hovered.weak_bg_fill = INPUT_HOVER;
                                vis.widgets.active.weak_bg_fill = INPUT_BG;
                                ui.add(DragValue::new(&mut p.price).speed(0.01))
                            },
                        )
                        .inner;
                    if resp.changed() {
                        changed = true;
                    }
                }
            });
        }
        changed
    }
}

fn outlined_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    let desired = Vec2::new(150.0, 32.0);
    let (rect, resp) = ui.allocate_exact_size(desired, Sense::click());
    let border = if resp.hovered() {
        TEXT_MUTED
    } else {
        BTN_OUTLINE
    };
    ui.painter()
        .rect_filled(rect, CornerRadius::same(6), Color32::TRANSPARENT);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(6),
        Stroke::new(1.0, border),
        egui::StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(13.0),
        TEXT_WHITE,
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

fn accent_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    let desired = Vec2::new(110.0, 32.0);
    let (rect, resp) = ui.allocate_exact_size(desired, Sense::click());
    let fill = if resp.hovered() { ACCENT_HOVER } else { ACCENT };
    ui.painter().rect_filled(rect, CornerRadius::same(6), fill);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(13.0),
        TEXT_WHITE,
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

fn date_for_index(data: &CandleData, index: f32) -> String {
    let rounded = index.round();
    if rounded < 0.0 {
        return "—".to_string();
    }
    let idx = rounded as usize;
    data.dates
        .get(idx)
        .cloned()
        .unwrap_or_else(|| "—".to_string())
}
