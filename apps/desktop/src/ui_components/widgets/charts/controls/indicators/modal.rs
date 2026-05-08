use std::collections::HashSet;

use eframe::egui::{self, Color32, CornerRadius, FontId, Pos2, Rect, RichText, Stroke, Vec2};

use super::super::super::indicators::{self, IndicatorManager, ParamValues, RenderTarget};
use super::params_popover::{ParamsResponse, show as show_params_popover};
use crate::theme;

// ── Category lookup ────────────────────────────────────────────────────────────
/// Maps an indicator `def_id` to a display category label.
fn indicator_category(id: &str) -> &'static str {
    match id {
        "ema" | "sma" | "wma" | "hma" | "alma" | "adx" | "ichimoku" => "Trend",
        "rsi" | "macd" | "ppo" | "stochastic" | "cci" | "roc" | "williams_r" | "mfi" => "Momentum",
        "bollinger" | "keltner" | "atr" => "Volatility",
        "volume" | "obv" | "vwap" => "Volume",
        "pivot_points" | "parabolic_sar" | "supertrend" | "chandelier" => "Other",
        _ => "Other",
    }
}

/// Ordered category names for the grouped list display.
const CATEGORIES: &[&str] = &["Trend", "Momentum", "Volatility", "Volume", "Other"];

// ── Modal state ────────────────────────────────────────────────────────────────

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

// ── Constants ──────────────────────────────────────────────────────────────────
const MODAL_W: f32 = 740.0;
const MODAL_H: f32 = 580.0;
const HEADER_H: f32 = 44.0;
const TAB_H: f32 = 32.0;
const SEARCH_H: f32 = 32.0;
const ROW_H: f32 = 36.0;
const SIDEBAR_ROW_H: f32 = 32.0;
const SIDEBAR_W: f32 = 220.0;
const H_PAD: f32 = 16.0; // horizontal padding inside modal chrome
const CORNER: f32 = 8.0;
const DIVIDER_ALPHA: Color32 = Color32::from_rgb(38, 38, 44);

// ── Main impl ──────────────────────────────────────────────────────────────────

impl IndicatorModal {
    pub fn show(&mut self, ctx: &egui::Context, manager: &mut IndicatorManager) {
        if !self.open {
            return;
        }

        let screen_rect = ctx.content_rect();
        let modal_size = Vec2::new(MODAL_W, MODAL_H);
        let center = screen_rect.center() - modal_size / 2.0;

        egui::Window::new("##indicator_modal")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .fixed_size(modal_size)
            .default_pos(Pos2::new(center.x, center.y))
            .frame(
                egui::Frame::new()
                    .fill(theme::SURFACE)
                    .stroke(Stroke::new(1.0, theme::BORDER_HOVER))
                    .corner_radius(CornerRadius::same(CORNER as u8))
                    .inner_margin(egui::Margin::ZERO),
            )
            .show(ctx, |ui| {
                ui.set_min_height(MODAL_H);
                // Smooth animations for hover fades
                ui.style_mut().animation_time = 0.15;

                self.draw_header(ui);
                self.draw_hairline(ui);
                self.draw_tab_bar(ui);
                self.draw_hairline(ui);

                let body_height = ui.available_height();
                ui.horizontal(|ui| {
                    ui.set_height(body_height);

                    // Left: search + grouped indicator list
                    ui.vertical(|ui| {
                        let left_w = MODAL_W - SIDEBAR_W - 1.0; // 1px for divider
                        ui.set_width(left_w);
                        ui.set_height(body_height);

                        self.draw_search(ui);
                        self.draw_hairline(ui);

                        // Scroll area — flush rows, no spacing
                        let scroll_height = ui.available_height();
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .max_height(scroll_height)
                            .show(ui, |ui| {
                                ui.style_mut().spacing.item_spacing.y = 0.0;
                                self.draw_indicator_list_grouped(ui, manager);
                            });

                        self.maybe_draw_editor(ui, manager);
                    });

                    // Vertical divider
                    let divider_rect =
                        Rect::from_min_size(ui.cursor().min, Vec2::new(1.0, body_height));
                    ui.painter().rect_filled(divider_rect, 0.0, DIVIDER_ALPHA);
                    ui.advance_cursor_after_rect(divider_rect);

                    // Right: added indicators sidebar
                    ui.vertical(|ui| {
                        ui.set_width(SIDEBAR_W - 1.0);
                        ui.set_height(body_height);
                        self.draw_added_sidebar(ui, manager);
                    });
                });
            });
    }

    // ── Header ────────────────────────────────────────────────────────────────

    fn draw_header(&mut self, ui: &mut egui::Ui) {
        let (rect, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), HEADER_H),
            egui::Sense::hover(),
        );
        let painter = ui.painter();

        // Close dot (left)
        let close_center = Pos2::new(rect.left() + H_PAD, rect.center().y);
        let close_rect = Rect::from_center_size(close_center, Vec2::splat(14.0));
        let close_id = ui.id().with("close_btn");
        let close_resp = ui.interact(close_rect, close_id, egui::Sense::click());
        let dot_col = if close_resp.hovered() {
            Color32::from_rgb(255, 110, 100)
        } else {
            Color32::from_rgb(255, 95, 87)
        };
        painter.circle_filled(close_center, 5.5, dot_col);
        if close_resp.clicked() {
            self.open = false;
            self.editor = None;
        }
        if close_resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        // Centered title
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "Indicators",
            FontId::proportional(14.0),
            theme::TEXT_PRIMARY,
        );

        // Ghost "+ New Indicator" button (right)
        let btn_text = format!("{}  New Indicator", egui_phosphor::regular::PLUS);
        let ghost_id = ui.id().with("new_ind_btn");
        let btn_galley = ui.painter().layout_no_wrap(
            btn_text.clone(),
            FontId::proportional(11.0),
            theme::ACCENT,
        );
        let btn_size = Vec2::new(btn_galley.size().x + 16.0, 22.0);
        let btn_min = Pos2::new(
            rect.right() - H_PAD - btn_size.x,
            rect.center().y - btn_size.y / 2.0,
        );
        let btn_rect = Rect::from_min_size(btn_min, btn_size);
        let ghost_resp = ui.interact(btn_rect, ghost_id, egui::Sense::click());

        let t = ui
            .ctx()
            .animate_bool_responsive(ghost_id, ghost_resp.hovered());
        let border_alpha = (t * 200.0) as u8;
        let border_col = Color32::from_rgba_premultiplied(
            theme::ACCENT.r(),
            theme::ACCENT.g(),
            theme::ACCENT.b(),
            border_alpha,
        );
        painter.rect(
            btn_rect,
            CornerRadius::same(4),
            Color32::TRANSPARENT,
            Stroke::new(1.0, border_col),
            egui::StrokeKind::Outside,
        );
        painter.text(
            btn_rect.center(),
            egui::Align2::CENTER_CENTER,
            btn_text,
            FontId::proportional(11.0),
            theme::ACCENT,
        );
        if ghost_resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
    }

    // ── Tab bar ───────────────────────────────────────────────────────────────

    fn draw_tab_bar(&mut self, ui: &mut egui::Ui) {
        let (band_rect, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), TAB_H), egui::Sense::hover());

        let painter = ui.painter();
        let tabs = ["Favorites", "All Indicators", "My Indicators"];
        let tab_w = 100.0_f32;
        let mut x = band_rect.left() + H_PAD;

        for (i, tab) in tabs.iter().enumerate() {
            let is_active = self.active_tab == i;
            let tab_rect =
                Rect::from_min_size(Pos2::new(x, band_rect.top()), Vec2::new(tab_w, TAB_H));
            let tab_id = ui.id().with(("tab", i));
            let tab_resp = ui.interact(tab_rect, tab_id, egui::Sense::click());

            // Animate hover brightness
            let hover_t = ui
                .ctx()
                .animate_bool_responsive(tab_id, tab_resp.hovered() || is_active);

            let text_col = if is_active {
                theme::TEXT_PRIMARY
            } else {
                let base = theme::TEXT_MUTED;
                Color32::from_rgb(
                    base.r() + ((theme::TEXT_PRIMARY.r() - base.r()) as f32 * hover_t * 0.6) as u8,
                    base.g() + ((theme::TEXT_PRIMARY.g() - base.g()) as f32 * hover_t * 0.6) as u8,
                    base.b() + ((theme::TEXT_PRIMARY.b() - base.b()) as f32 * hover_t * 0.6) as u8,
                )
            };

            painter.text(
                tab_rect.center(),
                egui::Align2::CENTER_CENTER,
                *tab,
                FontId::proportional(12.0),
                text_col,
            );

            if is_active {
                // 2px teal underline at bottom of tab rect
                painter.hline(
                    tab_rect.left() + 8.0..=tab_rect.right() - 8.0,
                    tab_rect.bottom() - 1.0,
                    Stroke::new(2.0, theme::ACCENT_TEAL),
                );
            }

            if tab_resp.clicked() {
                self.active_tab = i;
            }
            if tab_resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }

            x += tab_w;
        }
    }

    // ── Search bar ────────────────────────────────────────────────────────────

    fn draw_search(&mut self, ui: &mut egui::Ui) {
        let available_w = ui.available_width();
        let search_rect = Rect::from_min_size(
            ui.cursor().min + Vec2::new(H_PAD, 8.0),
            Vec2::new(available_w - H_PAD * 2.0, SEARCH_H),
        );

        // Allocate the full slot (including 8px top margin + 8px bottom margin)
        ui.allocate_exact_size(
            Vec2::new(available_w, SEARCH_H + 16.0),
            egui::Sense::hover(),
        );

        let focus_id = egui::Id::new("indicator_search_te");
        let is_focused = ui.ctx().memory(|m| m.has_focus(focus_id));

        let border_col = if is_focused {
            theme::ACCENT
        } else {
            Color32::TRANSPARENT
        };

        // Background pill
        ui.painter().rect(
            search_rect,
            CornerRadius::same(6),
            theme::SURFACE_HIGH,
            Stroke::new(1.0, border_col),
            egui::StrokeKind::Outside,
        );

        // Icon + TextEdit via a child ui placed inside the search_rect
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(search_rect));
        child.style_mut().visuals.extreme_bg_color = Color32::TRANSPARENT;
        child.style_mut().visuals.widgets.inactive.bg_fill = Color32::TRANSPARENT;
        child.style_mut().visuals.widgets.inactive.bg_stroke = Stroke::NONE;
        child.style_mut().visuals.widgets.hovered.bg_fill = Color32::TRANSPARENT;
        child.style_mut().visuals.widgets.hovered.bg_stroke = Stroke::NONE;
        child.style_mut().visuals.widgets.active.bg_fill = Color32::TRANSPARENT;
        child.style_mut().visuals.widgets.active.bg_stroke = Stroke::NONE;
        child.style_mut().visuals.selection.bg_fill = theme::ACCENT_BG;

        child.horizontal_centered(|ui| {
            ui.add_space(10.0);
            ui.label(
                RichText::new(egui_phosphor::regular::MAGNIFYING_GLASS)
                    .size(12.0)
                    .color(theme::TEXT_MUTED),
            );
            ui.add_space(6.0);
            let te = egui::TextEdit::singleline(&mut self.search_query)
                .id(focus_id)
                .hint_text(
                    RichText::new("Search indicators...")
                        .color(theme::TEXT_DIM)
                        .size(12.0),
                )
                .desired_width(f32::INFINITY)
                .frame(egui::Frame::NONE)
                .text_color(theme::TEXT_PRIMARY)
                .font(FontId::proportional(12.0));
            ui.add(te);
        });
    }

    // ── Grouped indicator list ─────────────────────────────────────────────────

    fn draw_indicator_list_grouped(&mut self, ui: &mut egui::Ui, manager: &mut IndicatorManager) {
        let query = self.search_query.to_lowercase();
        let all_defs = indicators::all();

        // Collect added def_ids for "already added" tint + checkmark
        let added_ids: HashSet<&'static str> = manager.active.iter().map(|a| a.def_id).collect();

        for &cat in CATEGORIES {
            // Gather defs for this category that pass tab + search filters
            let cat_defs: Vec<_> = all_defs
                .iter()
                .filter(|def| {
                    if indicator_category(def.id) != cat {
                        return false;
                    }
                    if !query.is_empty() && !def.name.to_lowercase().contains(&query) {
                        return false;
                    }
                    if self.active_tab == 0 && !self.favorited.contains(def.id) {
                        return false;
                    }
                    true
                })
                .collect();

            if cat_defs.is_empty() {
                continue;
            }

            // Category header row
            let avail_w = ui.available_width();
            let (hdr_rect, _) =
                ui.allocate_exact_size(Vec2::new(avail_w, 24.0), egui::Sense::hover());
            let painter = ui.painter();
            painter.rect_filled(hdr_rect, 0.0, theme::CATEGORY_HEADER_BG);
            painter.hline(
                hdr_rect.left()..=hdr_rect.right(),
                hdr_rect.bottom(),
                Stroke::new(1.0, DIVIDER_ALPHA),
            );
            painter.text(
                Pos2::new(hdr_rect.left() + H_PAD, hdr_rect.center().y),
                egui::Align2::LEFT_CENTER,
                cat,
                FontId::proportional(11.0),
                theme::TEXT_MUTED,
            );

            // Indicator rows for this category
            for def in cat_defs {
                let is_added = added_ids.contains(def.id);
                let is_fav = self.favorited.contains(def.id);

                let row_id = ui.id().with(("ind_row", def.id));
                let (row_rect, row_resp) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), ROW_H),
                    egui::Sense::hover(),
                );

                // Smooth hover animation
                let hover_t = ui.ctx().animate_bool_responsive(row_id, row_resp.hovered());

                // Row background: base tint if added, + hover tint
                if is_added {
                    ui.painter().rect_filled(
                        row_rect,
                        0.0,
                        Color32::from_rgba_premultiplied(42, 108, 255, 12),
                    );
                }
                if hover_t > 0.001 {
                    let alpha = (20.0 * hover_t) as u8;
                    ui.painter().rect_filled(
                        row_rect,
                        0.0,
                        Color32::from_rgba_premultiplied(42, 108, 255, alpha),
                    );
                }

                // Bottom hairline
                ui.painter().hline(
                    row_rect.left() + H_PAD..=row_rect.right(),
                    row_rect.bottom(),
                    Stroke::new(1.0, DIVIDER_ALPHA),
                );

                // ── Content inside the row via a child ui ──────────────────
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(row_rect));
                child.style_mut().spacing.item_spacing = Vec2::ZERO;

                child.horizontal_centered(|ui| {
                    ui.add_space(H_PAD);

                    // Star / favorite icon (left)
                    // Only the Regular phosphor font is loaded; use STAR for both states,
                    // differentiated by color (amber = active, dim = inactive).
                    let star_icon = egui_phosphor::regular::STAR;
                    let star_col = if is_fav {
                        theme::STAR_ACTIVE
                    } else {
                        theme::ICON_INACTIVE
                    };

                    let star_id = ui.id().with(("star", def.id));
                    let star_resp = ui.add(
                        egui::Button::new(RichText::new(star_icon).size(13.0).color(star_col))
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::NONE)
                            .min_size(Vec2::new(20.0, ROW_H)),
                    );
                    let _ = star_id; // used implicitly via widget id
                    if star_resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if star_resp.clicked() {
                        if is_fav {
                            self.favorited.remove(def.id);
                        } else {
                            self.favorited.insert(def.id);
                        }
                    }

                    ui.add_space(6.0);

                    // Indicator name + category subtitle (stacked via painter)
                    let name_text_w = ui.available_width() - 60.0; // reserve right side
                    let name_rect =
                        Rect::from_min_size(ui.cursor().min, Vec2::new(name_text_w, ROW_H));
                    ui.allocate_exact_size(Vec2::new(name_text_w, ROW_H), egui::Sense::hover());

                    let painter = ui.painter();
                    // Name — 13px TEXT_PRIMARY
                    painter.text(
                        Pos2::new(name_rect.left(), name_rect.center().y - 7.0),
                        egui::Align2::LEFT_CENTER,
                        def.name,
                        FontId::proportional(13.0),
                        theme::TEXT_PRIMARY,
                    );
                    // Category subtitle — 10px TEXT_MUTED
                    painter.text(
                        Pos2::new(name_rect.left(), name_rect.center().y + 7.0),
                        egui::Align2::LEFT_CENTER,
                        indicator_category(def.id),
                        FontId::proportional(10.0),
                        theme::TEXT_MUTED,
                    );

                    // Trailing add/check button (right-aligned)
                    let (add_icon, add_col) = if is_added {
                        (egui_phosphor::regular::CHECK, theme::ACCENT_TEAL)
                    } else {
                        (egui_phosphor::regular::PLUS_CIRCLE, theme::ICON_INACTIVE)
                    };

                    let add_resp = ui.add(
                        egui::Button::new(RichText::new(add_icon).size(15.0).color(add_col))
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::NONE)
                            .min_size(Vec2::new(32.0, ROW_H)),
                    );
                    if add_resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if add_resp.clicked() && !is_added {
                        // Add immediately with default params — user can tune
                        // via the gear icon on the sidebar row afterwards. The
                        // old "open params popover before adding" flow was
                        // removed; commit-then-edit is the friendlier UX.
                        manager.add(def.id, def.params.defaults());
                    }

                    ui.add_space(8.0);
                });
            }
        }
    }

    // ── Params popover (unchanged logic) ──────────────────────────────────────

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

    // ── Added indicators sidebar ───────────────────────────────────────────────

    fn draw_added_sidebar(&mut self, ui: &mut egui::Ui, manager: &mut IndicatorManager) {
        let available_h = ui.available_height();
        let sidebar_rect = ui.available_rect_before_wrap();

        // Sidebar background (slightly different from modal surface)
        ui.painter().rect_filled(
            sidebar_rect,
            CornerRadius::ZERO,
            Color32::from_rgb(20, 20, 23),
        );

        // Inner padding via a child ui
        let inner_rect = sidebar_rect.shrink2(Vec2::new(0.0, 0.0));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner_rect));
        child.style_mut().spacing.item_spacing.y = 0.0;

        // Header row
        let count = manager.active.len();
        let (hdr_rect, _) = child.allocate_exact_size(
            Vec2::new(child.available_width(), HEADER_H),
            egui::Sense::hover(),
        );
        child.painter().text(
            Pos2::new(hdr_rect.left() + 12.0, hdr_rect.center().y),
            egui::Align2::LEFT_CENTER,
            format!("Added Indicators ({})", count),
            FontId::proportional(12.0),
            theme::TEXT_STRONG,
        );
        child.painter().hline(
            hdr_rect.left()..=hdr_rect.right(),
            hdr_rect.bottom(),
            Stroke::new(1.0, DIVIDER_ALPHA),
        );

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

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .max_height(available_h - HEADER_H)
            .show(&mut child, |ui| {
                ui.style_mut().spacing.item_spacing.y = 0.0;

                if !main_ids.is_empty() {
                    draw_sidebar_section_header(ui, "Main Chart");
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
                }

                if !sub_ids.is_empty() {
                    draw_sidebar_section_header(ui, "Sub Chart");
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

                if main_ids.is_empty() && sub_ids.is_empty() {
                    let avail = ui.available_rect_before_wrap();
                    ui.painter().text(
                        avail.center(),
                        egui::Align2::CENTER_CENTER,
                        "No indicators added",
                        FontId::proportional(11.0),
                        theme::TEXT_MUTED,
                    );
                    ui.allocate_exact_size(avail.size(), egui::Sense::hover());
                }
            });

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
    }

    // ── Shared hairline helper ─────────────────────────────────────────────────

    fn draw_hairline(&self, ui: &mut egui::Ui) {
        let avail_w = ui.available_width();
        let (line_rect, _) = ui.allocate_exact_size(Vec2::new(avail_w, 1.0), egui::Sense::hover());
        ui.painter().hline(
            line_rect.left()..=line_rect.right(),
            line_rect.top(),
            Stroke::new(1.0, DIVIDER_ALPHA),
        );
    }
}

// ── Free functions ─────────────────────────────────────────────────────────────

fn draw_sidebar_section_header(ui: &mut egui::Ui, label: &str) {
    let avail_w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(avail_w, 28.0), egui::Sense::hover());
    ui.painter().text(
        Pos2::new(rect.left() + 12.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        FontId::proportional(10.0),
        theme::TEXT_MUTED,
    );
    ui.painter().hline(
        rect.left()..=rect.right(),
        rect.bottom(),
        Stroke::new(1.0, DIVIDER_ALPHA),
    );
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
    let row_id = ui.id().with(("sb_row", instance_id));
    let avail_w = ui.available_width();
    let (row_rect, row_resp) =
        ui.allocate_exact_size(Vec2::new(avail_w, SIDEBAR_ROW_H), egui::Sense::hover());

    // Hover tint
    let hover_t = ui.ctx().animate_bool_responsive(row_id, row_resp.hovered());
    if hover_t > 0.001 {
        let alpha = (20.0 * hover_t) as u8;
        ui.painter().rect_filled(
            row_rect,
            0.0,
            Color32::from_rgba_premultiplied(42, 108, 255, alpha),
        );
    }

    // Hairline below
    ui.painter().hline(
        row_rect.left()..=row_rect.right(),
        row_rect.bottom(),
        Stroke::new(1.0, DIVIDER_ALPHA),
    );

    // Content
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(row_rect));
    child.horizontal_centered(|ui| {
        // Zero out item-spacing so the icon buttons align flush against the
        // right edge — egui's default 8px inter-widget spacing was causing
        // the X icon to overflow the sidebar's right boundary.
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.add_space(12.0);

        // Label — takes remaining space minus icon buttons + trailing pad.
        // Layout: 12px add_space + label_w + 24px gear + 24px X + 8px pad = avail_w.
        let label_w = (avail_w - 12.0 - 24.0 - 24.0 - 8.0).max(0.0);
        let (lbl_rect, _) =
            ui.allocate_exact_size(Vec2::new(label_w, SIDEBAR_ROW_H), egui::Sense::hover());
        ui.painter().text(
            Pos2::new(lbl_rect.left(), lbl_rect.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            FontId::proportional(12.0),
            theme::TEXT_PRIMARY,
        );

        // Gear icon
        let gear_resp = ui.add(
            egui::Button::new(
                RichText::new(egui_phosphor::regular::GEAR)
                    .size(12.0)
                    .color(theme::ICON_INACTIVE),
            )
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::NONE)
            .min_size(Vec2::new(24.0, SIDEBAR_ROW_H)),
        );
        if gear_resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if gear_resp.clicked() {
            if let Some(a) = manager.active.iter().find(|a| a.instance_id == instance_id) {
                *to_edit = Some((instance_id, def_id, a.params.clone()));
            }
        }

        // X (remove) icon
        let del_resp = ui.add(
            egui::Button::new(
                RichText::new(egui_phosphor::regular::X)
                    .size(12.0)
                    .color(theme::ICON_INACTIVE),
            )
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::NONE)
            .min_size(Vec2::new(24.0, SIDEBAR_ROW_H)),
        );
        if del_resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if del_resp.clicked() {
            *to_remove = Some(instance_id);
        }

        ui.add_space(8.0);
    });
}
