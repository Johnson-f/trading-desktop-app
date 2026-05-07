use eframe::egui::{self, Color32, CornerRadius, DragValue, FontId, Pos2, Rect, RichText, Sense,
                   Stroke, Vec2};

use super::super::super::CandleData;
use super::super::super::drawings::{
    COLOR_PALETTE, CommittedDrawing, DashStyle, DrawingStyle, DrawingsManager, ExtendCapabilities,
    FIB_RATIO_COUNT, KindStyle, WorldPoint,
};
use super::super::super::util::{core_color, egui_color};
use crate::theme;

const VERTICAL_LINE_ID: &str = "vertical_line";

/// Fixed modal dimensions — stays constant across Style/Position tabs so the
/// window doesn't resize when the user switches tabs.
const MODAL_WIDTH: f32 = 460.0;
const MODAL_HEIGHT: f32 = 420.0;

const HEADER_H: f32 = 40.0;
const TAB_H: f32 = 32.0;
const ACTION_H: f32 = 44.0;
const H_PAD: f32 = 16.0;

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
                    .fill(theme::SURFACE)
                    .stroke(Stroke::new(1.0, theme::BORDER_HOVER))
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(egui::Margin::ZERO),
            )
            .show(ctx, |ui| {
                ui.set_min_height(MODAL_HEIGHT);
                ui.style_mut().animation_time = 0.15;

                // ── Header band ──────────────────────────────────────────────
                let (header_rect, _) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), HEADER_H),
                    Sense::hover(),
                );

                // macOS close dot (red)
                let close_center = Pos2::new(header_rect.left() + H_PAD, header_rect.center().y);
                let close_id = ui.id().with("close_btn");
                let close_hit = Rect::from_center_size(close_center, Vec2::splat(16.0));
                let close_resp = ui.interact(close_hit, close_id, Sense::click());
                let dot_col = if close_resp.hovered() {
                    Color32::from_rgb(255, 110, 100)
                } else {
                    Color32::from_rgb(255, 95, 87)
                };
                ui.painter().circle_filled(close_center, 5.5, dot_col);
                if close_resp.clicked() {
                    should_close = true;
                }
                if close_resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }

                // Centered title
                ui.painter().text(
                    header_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    &display_name,
                    FontId::proportional(14.0),
                    theme::TEXT_PRIMARY,
                );

                // Hairline divider below header
                ui.painter().hline(
                    header_rect.left()..=header_rect.right(),
                    header_rect.bottom(),
                    Stroke::new(1.0, theme::BORDER),
                );

                // ── Tab bar ──────────────────────────────────────────────────
                let position_tab_label = if def_id == VERTICAL_LINE_ID {
                    "Date"
                } else {
                    "Price"
                };

                let (tab_rect, _) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), TAB_H),
                    Sense::hover(),
                );

                let painter = ui.painter();
                let tabs = [(Tab::Style, "Style"), (Tab::Position, position_tab_label)];
                let tab_w = 80.0_f32;
                let mut tx = tab_rect.left() + H_PAD;

                for (tab, label) in tabs {
                    let is_active = self.active_tab == tab;
                    let t_rect = Rect::from_min_size(
                        Pos2::new(tx, tab_rect.top()),
                        Vec2::new(tab_w, TAB_H),
                    );
                    let tab_id = ui.id().with(("dtab", label));
                    let tab_resp = ui.interact(t_rect, tab_id, Sense::click());

                    let hover_t = ui
                        .ctx()
                        .animate_bool_responsive(tab_id, tab_resp.hovered() || is_active);

                    let text_col = if is_active {
                        theme::TEXT_PRIMARY
                    } else {
                        let base = theme::TEXT_MUTED;
                        Color32::from_rgb(
                            base.r()
                                + ((theme::TEXT_PRIMARY.r() - base.r()) as f32
                                    * hover_t
                                    * 0.6) as u8,
                            base.g()
                                + ((theme::TEXT_PRIMARY.g() - base.g()) as f32
                                    * hover_t
                                    * 0.6) as u8,
                            base.b()
                                + ((theme::TEXT_PRIMARY.b() - base.b()) as f32
                                    * hover_t
                                    * 0.6) as u8,
                        )
                    };

                    painter.text(
                        t_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        label,
                        FontId::proportional(12.0),
                        text_col,
                    );

                    if is_active {
                        painter.hline(
                            t_rect.left() + 8.0..=t_rect.right() - 8.0,
                            t_rect.bottom() - 1.0,
                            Stroke::new(2.0, theme::ACCENT_TEAL),
                        );
                    }

                    if tab_resp.clicked() {
                        self.active_tab = tab;
                    }
                    if tab_resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }

                    tx += tab_w;
                }

                // Hairline divider below tab bar
                ui.painter().hline(
                    tab_rect.left()..=tab_rect.right(),
                    tab_rect.bottom(),
                    Stroke::new(1.0, theme::BORDER),
                );

                // ── Content area ─────────────────────────────────────────────
                // Reserve ACTION_H + 1px divider for the action row at the
                // bottom. The form card fills the rest.
                let avail = ui.available_size();
                let content_height = (avail.y - ACTION_H - 1.0).max(0.0);

                ui.allocate_ui(Vec2::new(avail.x, content_height), |ui| {
                    // 16px outer padding so the card doesn't touch the modal
                    // edge on any side.
                    let inner_margin = egui::Margin::same(16);
                    egui::Frame::new()
                        .fill(theme::SURFACE_HIGH)
                        .corner_radius(CornerRadius::same(6))
                        .inner_margin(inner_margin)
                        .outer_margin(egui::Margin::same(12))
                        .show(ui, |ui| {
                            ui.set_min_size(Vec2::new(
                                ui.available_width(),
                                content_height - 24.0,
                            ));
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

                // ── Action row ───────────────────────────────────────────────
                // Hairline above action row
                let action_top = ui.cursor().top();
                ui.painter().hline(
                    0.0..=MODAL_WIDTH,
                    action_top,
                    Stroke::new(1.0, theme::BORDER),
                );

                let (action_rect, _) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), ACTION_H),
                    Sense::hover(),
                );

                // Reset to Defaults — ghost outlined button (left)
                let reset_id = ui.id().with("reset_btn");
                let reset_rect = Rect::from_min_size(
                    Pos2::new(action_rect.left() + H_PAD, action_rect.center().y - 16.0),
                    Vec2::new(148.0, 32.0),
                );
                let reset_resp = ui.interact(reset_rect, reset_id, Sense::click());
                let reset_t = ui
                    .ctx()
                    .animate_bool_responsive(reset_id, reset_resp.hovered());
                let border_col = lerp_color(theme::BORDER_HOVER, theme::TEXT_MUTED, reset_t * 0.6);
                let text_col = lerp_color(theme::TEXT_MUTED, theme::TEXT_PRIMARY, reset_t * 0.7);
                ui.painter().rect(
                    reset_rect,
                    CornerRadius::same(6),
                    Color32::TRANSPARENT,
                    Stroke::new(1.0, border_col),
                    egui::StrokeKind::Inside,
                );
                ui.painter().text(
                    reset_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "Reset to Defaults",
                    FontId::proportional(12.0),
                    text_col,
                );
                if reset_resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if reset_resp.clicked() {
                    should_reset = true;
                }

                // Done — solid accent CTA (right)
                let done_id = ui.id().with("done_btn");
                let done_rect = Rect::from_min_size(
                    Pos2::new(
                        action_rect.right() - H_PAD - 96.0,
                        action_rect.center().y - 16.0,
                    ),
                    Vec2::new(96.0, 32.0),
                );
                let done_resp = ui.interact(done_rect, done_id, Sense::click());
                let done_t = ui
                    .ctx()
                    .animate_bool_responsive(done_id, done_resp.hovered());
                // Slightly dim fill on hover (fade back 15%)
                let done_fill = lerp_color(theme::ACCENT, theme::SURFACE_HIGH, done_t * 0.15);
                ui.painter()
                    .rect_filled(done_rect, CornerRadius::same(6), done_fill);
                ui.painter().text(
                    done_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "Done",
                    FontId::proportional(13.0),
                    Color32::WHITE,
                );
                if done_resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if done_resp.clicked() {
                    should_close = true;
                }
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

        // ── Color ─────────────────────────────────────────────────────────────
        ui.label(
            RichText::new("Color")
                .color(theme::TEXT_MUTED)
                .size(11.0),
        );
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::splat(6.0);
            for c in COLOR_PALETTE {
                let size = Vec2::splat(28.0);
                let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
                let swatch_col = egui_color(*c);
                ui.painter()
                    .rect_filled(rect, CornerRadius::same(4), swatch_col);

                if self.draft_style.color == *c {
                    // Selected: 2px TEXT_PRIMARY outer ring + CHECK glyph
                    ui.painter().rect_stroke(
                        rect.expand(2.0),
                        CornerRadius::same(6),
                        Stroke::new(2.0, theme::TEXT_PRIMARY),
                        egui::StrokeKind::Outside,
                    );
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        egui_phosphor::regular::CHECK,
                        FontId::proportional(12.0),
                        Color32::WHITE,
                    );
                } else if resp.hovered() {
                    ui.painter().rect_stroke(
                        rect,
                        CornerRadius::same(4),
                        Stroke::new(1.0, theme::BORDER_HOVER),
                        egui::StrokeKind::Outside,
                    );
                }

                if resp.clicked() {
                    self.draft_style.color = *c;
                    changed = true;
                }
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
            }
        });

        // ── Width ─────────────────────────────────────────────────────────────
        ui.add_space(12.0);
        ui.label(
            RichText::new("Width")
                .color(theme::TEXT_MUTED)
                .size(11.0),
        );
        ui.add_space(8.0);
        ui.scope(|ui| {
            apply_field_visuals(ui);
            if ui
                .add(egui::Slider::new(&mut self.draft_style.width, 0.5..=5.0).text("px"))
                .changed()
            {
                changed = true;
            }
        });

        // ── Style (Solid / Dashed / Dotted) ───────────────────────────────────
        ui.add_space(12.0);
        ui.label(
            RichText::new("Style")
                .color(theme::TEXT_MUTED)
                .size(11.0),
        );
        ui.add_space(8.0);

        // Allocate a horizontal band for the three frameless tab-like buttons
        let avail_w = ui.available_width();
        let btn_w = 72.0_f32;
        let style_band_h = 28.0_f32;
        let (band_rect, _) =
            ui.allocate_exact_size(Vec2::new(avail_w, style_band_h), Sense::hover());
        let painter = ui.painter();
        let mut bx = band_rect.left();

        for (d, label) in [
            (DashStyle::Solid, "Solid"),
            (DashStyle::Dashed, "Dashed"),
            (DashStyle::Dotted, "Dotted"),
        ] {
            let is_active = self.draft_style.dash == d;
            let btn_r =
                Rect::from_min_size(Pos2::new(bx, band_rect.top()), Vec2::new(btn_w, style_band_h));
            let btn_id = ui.id().with(("dstyle", label));
            let btn_resp = ui.interact(btn_r, btn_id, Sense::click());

            let hover_t = ui
                .ctx()
                .animate_bool_responsive(btn_id, btn_resp.hovered() || is_active);
            let text_col = if is_active {
                theme::TEXT_PRIMARY
            } else {
                let base = theme::TEXT_MUTED;
                Color32::from_rgb(
                    base.r()
                        + ((theme::TEXT_PRIMARY.r() - base.r()) as f32 * hover_t * 0.6) as u8,
                    base.g()
                        + ((theme::TEXT_PRIMARY.g() - base.g()) as f32 * hover_t * 0.6) as u8,
                    base.b()
                        + ((theme::TEXT_PRIMARY.b() - base.b()) as f32 * hover_t * 0.6) as u8,
                )
            };

            painter.text(
                btn_r.center(),
                egui::Align2::CENTER_CENTER,
                label,
                FontId::proportional(12.0),
                text_col,
            );

            if is_active {
                painter.hline(
                    btn_r.left() + 6.0..=btn_r.right() - 6.0,
                    btn_r.bottom() - 1.0,
                    Stroke::new(2.0, theme::ACCENT_TEAL),
                );
            }

            if btn_resp.clicked() {
                self.draft_style.dash = d;
                changed = true;
            }
            if btn_resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }

            bx += btn_w;
        }

        // ── Opacity ───────────────────────────────────────────────────────────
        ui.add_space(12.0);
        ui.label(
            RichText::new("Opacity")
                .color(theme::TEXT_MUTED)
                .size(11.0),
        );
        ui.add_space(8.0);
        ui.scope(|ui| {
            apply_field_visuals(ui);
            if ui
                .add(egui::Slider::new(&mut self.draft_style.opacity, 0.0..=1.0))
                .changed()
            {
                changed = true;
            }
        });

        // ── Extend ────────────────────────────────────────────────────────────
        if extend_caps.left || extend_caps.right {
            ui.add_space(12.0);
            ui.label(
                RichText::new("Extend")
                    .color(theme::TEXT_MUTED)
                    .size(11.0),
            );
            ui.add_space(8.0);
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
                ui.label(
                    RichText::new("Fill")
                        .color(theme::TEXT_MUTED)
                        .size(11.0),
                );
                ui.add_space(8.0);
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
                    RichText::new("Tine fill")
                        .color(theme::TEXT_MUTED)
                        .size(11.0),
                );
                ui.add_space(8.0);
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
                ui.label(
                    RichText::new("Levels")
                        .color(theme::TEXT_MUTED)
                        .size(11.0),
                );
                ui.add_space(8.0);
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
                    RichText::new("Zone colors")
                        .color(theme::TEXT_MUTED)
                        .size(11.0),
                );
                ui.add_space(8.0);
                let row = |ui: &mut egui::Ui,
                               label: &str,
                               color: &mut zaned_chart_core::Rgba|
                 -> bool {
                    let mut inner_changed = false;
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(label)
                                .color(theme::TEXT_PRIMARY)
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
                ui.label(
                    RichText::new("Label")
                        .color(theme::TEXT_MUTED)
                        .size(11.0),
                );
                ui.add_space(8.0);
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
        let field_width = 110.0;
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
                    Pos2::new(label_rect.left(), label_rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    label,
                    FontId::proportional(13.0),
                    theme::TEXT_MUTED,
                );

                if is_vertical {
                    let date = date_for_index(data, p.index);
                    ui.painter().text(
                        Pos2::new(ui.cursor().left() + 8.0, label_rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        date,
                        FontId::monospace(13.0),
                        theme::TEXT_PRIMARY,
                    );
                    ui.add_space(field_width);
                } else {
                    ui.add_space(8.0);
                    let resp = ui
                        .scope(|ui| {
                            apply_field_visuals(ui);
                            ui.add_sized(
                                Vec2::new(field_width, 28.0),
                                DragValue::new(&mut p.price)
                                    .speed(0.01)
                                    .min_decimals(2)
                                    .max_decimals(4),
                            )
                        })
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

// ── Helper: linear-interpolate two Color32 values ─────────────────────────────

/// Applies the modal's standard field visuals to a `Ui` scope: dark elevated
/// surface, bright readable text, subtle border that lights up on hover/focus.
/// Used by every numeric input + slider numeric box on the Style and Position
/// tabs so they all read identically.
fn apply_field_visuals(ui: &mut egui::Ui) {
    let vis = ui.visuals_mut();
    vis.extreme_bg_color = theme::SURFACE_HIGH;
    vis.widgets.inactive.weak_bg_fill = theme::SURFACE_HIGH;
    vis.widgets.inactive.bg_fill = theme::SURFACE_HIGH;
    vis.widgets.hovered.weak_bg_fill = theme::SURFACE_HIGH;
    vis.widgets.active.weak_bg_fill = theme::SURFACE_HIGH;
    vis.widgets.open.weak_bg_fill = theme::SURFACE_HIGH;
    vis.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, theme::TEXT_PRIMARY);
    vis.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, theme::TEXT_PRIMARY);
    vis.widgets.active.fg_stroke = egui::Stroke::new(1.0, theme::TEXT_PRIMARY);
    vis.widgets.open.fg_stroke = egui::Stroke::new(1.0, theme::TEXT_PRIMARY);
    vis.override_text_color = Some(theme::TEXT_PRIMARY);
    vis.widgets.inactive.bg_stroke = egui::Stroke::NONE;
    vis.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, theme::BORDER_HOVER);
    vis.widgets.active.bg_stroke = egui::Stroke::new(1.0, theme::ACCENT_TEAL);
    vis.widgets.open.bg_stroke = egui::Stroke::new(1.0, theme::ACCENT_TEAL);
    vis.selection.bg_fill = theme::ACCENT_TEAL;
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    Color32::from_rgb(
        (a.r() as f32 + (b.r() as f32 - a.r() as f32) * t) as u8,
        (a.g() as f32 + (b.g() as f32 - a.g() as f32) * t) as u8,
        (a.b() as f32 + (b.b() as f32 - a.b() as f32) * t) as u8,
    )
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
