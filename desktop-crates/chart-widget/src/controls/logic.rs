use eframe::egui::{self, Color32, CornerRadius, RichText, Stroke, Vec2};

use super::super::drawings;
use super::super::indicators::IndicatorManager;
use super::super::multi_charts::{GridLayout, SyncFlags};
use super::indicators::{IndicatorBar, IndicatorBarEvent, IndicatorModal};

const BG: Color32 = Color32::from_rgb(0, 0, 0);
const BORDER: Color32 = Color32::from_rgb(30, 30, 33);
const ICON_COLOR: Color32 = Color32::from_rgb(120, 120, 130);
const ICON_HOVER: Color32 = Color32::from_rgb(200, 200, 210);
const HOVER_BG: Color32 = Color32::from_rgb(28, 28, 32);
const ICON_ACTIVE_BG: Color32 = Color32::from_rgb(35, 35, 40);

const TOOLBAR_ICONS: &[&str] = &[
    egui_phosphor::regular::TREND_UP,
    egui_phosphor::regular::PENCIL_SIMPLE,
    egui_phosphor::regular::PAINT_BUCKET,
    egui_phosphor::regular::CHART_BAR,
    egui_phosphor::regular::SQUARES_FOUR,
    egui_phosphor::regular::CHART_LINE,
    egui_phosphor::regular::TEXT_T,
    egui_phosphor::regular::CLOUD,
    egui_phosphor::regular::CURRENCY_DOLLAR,
    egui_phosphor::regular::SMILEY,
    egui_phosphor::regular::CROSSHAIR,
    egui_phosphor::regular::MAGNET,
    egui_phosphor::regular::LOCK_SIMPLE,
    egui_phosphor::regular::EYE,
    egui_phosphor::regular::TRASH,
];

const INDICATOR_TOOL_INDEX: usize = 0;
const PENCIL_TOOL_INDEX: usize = 1;
const GRIDS_TOOL_INDEX: usize = 4;
const EYE_TOOL_INDEX: usize = 13;
const TRASH_TOOL_INDEX: usize = 14;
const INDICATOR_INSERT_AFTER: usize = 10;
const SEPARATORS: &[usize] = &[10];
const TOOLTIPS: &[&str] = &[
    "Indicator",
    "Drawings",
    "",
    "Line Style",
    "Grids",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "Hide drawings",
    "Delete all drawings",
];

pub struct ChartToolbar {
    pub active_tool: usize,
    pub active_drawing: Option<&'static str>,
    pub indicator_bar: IndicatorBar,
    pub indicator_modal: IndicatorModal,
    pub show_indicators: bool,
    pub show_drawings: bool,
    pub show_grids: bool,
    /// Set true for one frame when the user clicks the multi-chart toggle.
    /// Drained by `ChartWidget::take_multi_chart_toggle_request()` and consumed
    /// by `MyApp` to swap `ChartView` variants.
    pub toggle_multi_chart_pending: bool,
    /// Layout currently active in the enclosing `MultiChartWidget` (or
    /// `Single` when the chart is not in multi mode). Set externally each
    /// frame so the inline grid bar can highlight the active option.
    pub active_grid_layout: GridLayout,
    /// Set when the user picks a layout from the inline grid bar. Drained
    /// by `ChartWidget::take_grid_layout_request()` and consumed by `MyApp`
    /// (single-mode) or `MultiChartWidget` (multi-mode) to apply it.
    pub pending_grid_layout: Option<GridLayout>,
    /// Sync flags currently active on the enclosing `MultiChartWidget`.
    /// Set externally each frame so the inline grid bar's Sync checkboxes
    /// reflect (and can mutate) the live state.
    pub active_sync_flags: SyncFlags,
    /// Set when the user toggles a Sync checkbox in the inline grid bar.
    /// Drained by `ChartWidget::take_sync_flags_request()`.
    pub pending_sync_flags: Option<SyncFlags>,
    /// Reflects whether the chart's drawings are currently hidden. Set
    /// externally each frame so the eye icon can render the active state.
    pub drawings_hidden: bool,
    /// Set true for one frame when the user clicks the eye icon. Drained by
    /// `ChartWidget` and applied to its visibility flag.
    pub pending_toggle_drawings_visibility: bool,
    /// Confirmation dialog for "delete all drawings". Toggled by the trash
    /// icon; rendered in `ChartToolbar::show`.
    pub clear_drawings_modal_open: bool,
    /// Set true when the user confirms the "delete all drawings" dialog.
    /// Drained by `ChartWidget` and applied to its drawings + database.
    pub pending_clear_drawings: bool,
}

/// Slice of `ChartToolbar` state that needs to stay in sync across all panes
/// in a `MultiChartWidget`. Without this, hovering a different pane (which
/// switches `active_view` mid-frame) would discard whichever inline bar the
/// user just opened, since the new active pane's toolbar starts from its own
/// (closed) UI state.
#[derive(Clone, Copy, Default)]
pub struct ToolbarUiState {
    pub active_tool: usize,
    pub active_drawing: Option<&'static str>,
    pub show_indicators: bool,
    pub show_drawings: bool,
    pub show_grids: bool,
}

impl ChartToolbar {
    pub fn ui_state(&self) -> ToolbarUiState {
        ToolbarUiState {
            active_tool: self.active_tool,
            active_drawing: self.active_drawing,
            show_indicators: self.show_indicators,
            show_drawings: self.show_drawings,
            show_grids: self.show_grids,
        }
    }

    pub fn set_ui_state(&mut self, state: ToolbarUiState) {
        self.active_tool = state.active_tool;
        self.active_drawing = state.active_drawing;
        self.show_indicators = state.show_indicators;
        self.show_drawings = state.show_drawings;
        self.show_grids = state.show_grids;
    }
}

impl Default for ChartToolbar {
    fn default() -> Self {
        Self {
            active_tool: 0,
            active_drawing: None,
            indicator_bar: IndicatorBar::default(),
            indicator_modal: IndicatorModal::default(),
            show_indicators: false,
            show_drawings: false,
            show_grids: false,
            toggle_multi_chart_pending: false,
            active_grid_layout: GridLayout::Single,
            pending_grid_layout: None,
            active_sync_flags: SyncFlags::default(),
            pending_sync_flags: None,
            drawings_hidden: false,
            pending_toggle_drawings_visibility: false,
            clear_drawings_modal_open: false,
            pending_clear_drawings: false,
        }
    }
}

impl ChartToolbar {
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        manager: &mut IndicatorManager,
    ) -> Option<IndicatorBarEvent> {
        let mut bar_event: Option<IndicatorBarEvent> = None;

        egui::Frame::new()
            .fill(BG)
            .inner_margin(egui::Margin::symmetric(8, 4))
            .stroke(Stroke::new(0.5, BORDER))
            .show(ui, |ui| {
                ui.style_mut().interaction.tooltip_delay = 0.0;
                ui.style_mut().animation_time = 0.0;
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;

                    for (i, icon) in TOOLBAR_ICONS.iter().enumerate() {
                        if self.show_indicators && i > INDICATOR_INSERT_AFTER {
                            continue;
                        }
                        // When the drawings bar is expanded, hide everything past
                        // the pencil so the inline row has room.
                        if self.show_drawings && i > PENCIL_TOOL_INDEX {
                            continue;
                        }
                        if self.show_grids && i > GRIDS_TOOL_INDEX {
                            continue;
                        }

                        let is_indicator_btn = i == INDICATOR_TOOL_INDEX;
                        let is_pencil_btn = i == PENCIL_TOOL_INDEX;
                        let is_grids_btn = i == GRIDS_TOOL_INDEX;
                        let is_eye_btn = i == EYE_TOOL_INDEX;
                        let is_trash_btn = i == TRASH_TOOL_INDEX;
                        let is_active = if is_indicator_btn {
                            self.show_indicators
                        } else if is_pencil_btn {
                            self.show_drawings || self.active_drawing.is_some()
                        } else if is_grids_btn {
                            self.show_grids
                                || !matches!(self.active_grid_layout, GridLayout::Single)
                        } else if is_eye_btn {
                            self.drawings_hidden
                        } else if is_trash_btn {
                            self.clear_drawings_modal_open
                        } else {
                            self.active_tool == i
                        };

                        let fill = if is_active {
                            ICON_ACTIVE_BG
                        } else {
                            Color32::TRANSPARENT
                        };
                        let color = if is_active { ICON_HOVER } else { ICON_COLOR };

                        let btn = ui.add(
                            egui::Button::new(RichText::new(*icon).size(15.0).color(color))
                                .fill(fill)
                                .corner_radius(CornerRadius::same(4))
                                .min_size(Vec2::new(28.0, 28.0)),
                        );

                        if btn.hovered() && !is_active {
                            let rect = btn.rect;
                            ui.painter().rect_filled(rect, 4.0, HOVER_BG);
                            ui.painter().text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                *icon,
                                egui::FontId::proportional(15.0),
                                ICON_HOVER,
                            );
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }

                        if btn.clicked() {
                            if is_indicator_btn {
                                self.show_indicators = !self.show_indicators;
                                if self.show_indicators {
                                    self.show_drawings = false;
                                    self.show_grids = false;
                                }
                            } else if is_pencil_btn {
                                self.show_drawings = !self.show_drawings;
                                if self.show_drawings {
                                    self.show_indicators = false;
                                    self.show_grids = false;
                                }
                            } else if is_grids_btn {
                                self.show_grids = !self.show_grids;
                                if self.show_grids {
                                    self.show_indicators = false;
                                    self.show_drawings = false;
                                }
                            } else if is_eye_btn {
                                self.pending_toggle_drawings_visibility = true;
                            } else if is_trash_btn {
                                self.clear_drawings_modal_open = true;
                            } else {
                                self.active_tool = i;
                            }
                        }

                        if let Some(tip) = TOOLTIPS.get(i) {
                            if !tip.is_empty() {
                                btn.clone().on_hover_ui(|ui| {
                                    ui.label(RichText::new(*tip).size(11.0));
                                });
                            }
                        }

                        // Inline drawings bar — replaces the rest of the toolbar
                        // when expanded.
                        if is_pencil_btn && self.show_drawings {
                            draw_drawings_inline(ui, &mut self.active_drawing);
                        }

                        if is_grids_btn && self.show_grids {
                            draw_grids_inline(
                                ui,
                                self.active_grid_layout,
                                &mut self.pending_grid_layout,
                                self.active_sync_flags,
                                &mut self.pending_sync_flags,
                            );
                        }

                        if i == INDICATOR_INSERT_AFTER && self.show_indicators {
                            if let Some(ev) = self.indicator_bar.show_inline(ui, manager) {
                                bar_event = Some(ev);
                            }
                        }

                        if SEPARATORS.contains(&i)
                            && !(i == INDICATOR_INSERT_AFTER && self.show_indicators)
                            && !(i == PENCIL_TOOL_INDEX && self.show_drawings)
                            && !(i == GRIDS_TOOL_INDEX && self.show_grids)
                        {
                            ui.add_space(2.0);
                            let (rect, _) =
                                ui.allocate_exact_size(Vec2::new(1.0, 18.0), egui::Sense::hover());
                            ui.painter().rect_filled(rect, 0.0, BORDER);
                            ui.add_space(2.0);
                        }
                    }
                });
            });

        self.indicator_modal.show(ui.ctx(), manager);

        if let Some(IndicatorBarEvent::OpenModal) = &bar_event {
            self.indicator_modal.open = true;
        }

        // Render the "delete all drawings" confirmation. Sets
        // `pending_clear_drawings = true` on confirm so the chart widget can
        // drain it and apply the wipe.
        if self.clear_drawings_modal_open {
            show_clear_drawings_modal(
                ui.ctx(),
                &mut self.clear_drawings_modal_open,
                &mut self.pending_clear_drawings,
            );
        }

        bar_event
    }
}

/// (layout, label, tooltip) for each option in the inline grid bar.
const GRID_OPTIONS: &[(GridLayout, &str, &str)] = &[
    (GridLayout::Single, "▢", "Single"),
    (GridLayout::Horizontal2, "▥", "1×2"),
    (GridLayout::Vertical2, "▤", "2×1"),
    (GridLayout::Grid1x3, "▥▥▥", "1×3"),
    (GridLayout::Grid3x1, "▤▤▤", "3×1"),
    (GridLayout::Grid2x2, "▦", "2×2"),
];

/// Render the grids bar inline next to the grids button. Mirrors the shape of
/// `draw_drawings_inline` — a separator, one button per layout (with a
/// tooltip), and the active layout highlighted.
fn draw_grids_inline(
    ui: &mut egui::Ui,
    active: GridLayout,
    pending: &mut Option<GridLayout>,
    sync: SyncFlags,
    pending_sync: &mut Option<SyncFlags>,
) {
    ui.add_space(4.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, 18.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 0.0, BORDER);
    ui.add_space(4.0);

    for (layout, label, tip) in GRID_OPTIONS {
        let is_active = active == *layout;
        let fill = if is_active {
            ICON_ACTIVE_BG
        } else {
            Color32::TRANSPARENT
        };
        let color = if is_active { ICON_HOVER } else { ICON_COLOR };

        let btn = ui.add(
            egui::Button::new(RichText::new(*label).size(13.0).color(color))
                .fill(fill)
                .corner_radius(CornerRadius::same(4))
                .min_size(Vec2::new(34.0, 28.0)),
        );

        if btn.hovered() && !is_active {
            let rect = btn.rect;
            ui.painter().rect_filled(rect, 4.0, HOVER_BG);
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                *label,
                egui::FontId::proportional(13.0),
                ICON_HOVER,
            );
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        if btn.clicked() && !is_active {
            *pending = Some(*layout);
        }

        btn.clone().on_hover_ui(|ui| {
            ui.label(RichText::new(*tip).size(11.0));
        });
    }

    // Sync checkboxes — only meaningful once we're in a multi layout, since a
    // single pane has nothing to sync against.
    if matches!(active, GridLayout::Single) {
        return;
    }

    ui.add_space(4.0);
    let (sep_rect, _) = ui.allocate_exact_size(Vec2::new(1.0, 18.0), egui::Sense::hover());
    ui.painter().rect_filled(sep_rect, 0.0, BORDER);
    ui.add_space(4.0);

    ui.label(RichText::new("Sync:").size(11.0).color(ICON_COLOR));

    let mut next = sync;
    ui.checkbox(&mut next.symbol, "Symbol");
    ui.checkbox(&mut next.time, "Time");
    ui.checkbox(&mut next.indicators, "Indicators");
    ui.checkbox(&mut next.crosshair, "Crosshair");
    if next != sync {
        *pending_sync = Some(next);
    }
}

/// Render the drawings bar inline next to the pencil button. Mirrors the shape
/// of `IndicatorBar::show_inline` — a separator, one icon button per tool
/// (with the tool's name as a tooltip), and the active tool highlighted.
fn draw_drawings_inline(ui: &mut egui::Ui, active_drawing: &mut Option<&'static str>) {
    ui.add_space(4.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, 18.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 0.0, BORDER);
    ui.add_space(4.0);

    for def in drawings::all() {
        let is_active = *active_drawing == Some(def.id);
        let fill = if is_active {
            ICON_ACTIVE_BG
        } else {
            Color32::TRANSPARENT
        };
        let color = if is_active { ICON_HOVER } else { ICON_COLOR };

        let btn = ui.add(
            egui::Button::new(RichText::new(def.icon).size(15.0).color(color))
                .fill(fill)
                .corner_radius(CornerRadius::same(4))
                .min_size(Vec2::new(28.0, 28.0)),
        );

        if btn.hovered() && !is_active {
            let rect = btn.rect;
            ui.painter().rect_filled(rect, 4.0, HOVER_BG);
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                def.icon,
                egui::FontId::proportional(15.0),
                ICON_HOVER,
            );
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        if btn.clicked() {
            *active_drawing = if is_active { None } else { Some(def.id) };
        }

        btn.clone().on_hover_ui(|ui| {
            ui.label(RichText::new(def.name).size(11.0));
        });
    }
}

const MODAL_BG: Color32 = Color32::from_rgb(16, 16, 20);
const MODAL_BORDER: Color32 = Color32::from_rgb(38, 38, 44);
const MODAL_TEXT: Color32 = Color32::from_rgb(230, 230, 234);
const MODAL_TEXT_MUTED: Color32 = Color32::from_rgb(170, 170, 178);
const MODAL_PRIMARY_BG: Color32 = Color32::from_rgb(58, 130, 246);
const MODAL_PRIMARY_TEXT: Color32 = Color32::from_rgb(255, 255, 255);
const MODAL_HELP_FG: Color32 = Color32::from_rgb(45, 145, 130);

fn show_clear_drawings_modal(ctx: &egui::Context, open: &mut bool, confirmed: &mut bool) {
    // Strip egui's default Modal frame (light stroke + shadow); we draw our
    // own dark frame inside.
    let modal = egui::Modal::new(egui::Id::new("clear_drawings_modal"))
        .frame(egui::Frame::NONE)
        .show(ctx, |ui| {
            ui.set_min_width(440.0);
            egui::Frame::new()
                .fill(MODAL_BG)
                .stroke(Stroke::new(1.0, MODAL_BORDER))
                .corner_radius(CornerRadius::same(8))
                .inner_margin(egui::Margin::same(20))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        // Question-mark badge.
                        let (badge_rect, _) =
                            ui.allocate_exact_size(Vec2::new(28.0, 28.0), egui::Sense::hover());
                        ui.painter().circle_stroke(
                            badge_rect.center(),
                            13.0,
                            Stroke::new(1.5, MODAL_HELP_FG),
                        );
                        ui.painter().text(
                            badge_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "?",
                            egui::FontId::proportional(15.0),
                            MODAL_HELP_FG,
                        );
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(
                                "Are you sure you want to delete all drawings in this chart?",
                            )
                            .size(13.0)
                            .color(MODAL_TEXT),
                        );
                    });

                    ui.add_space(18.0);

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let primary = egui::Button::new(
                            RichText::new("Delete").size(13.0).color(MODAL_PRIMARY_TEXT),
                        )
                        .fill(MODAL_PRIMARY_BG)
                        .corner_radius(CornerRadius::same(6))
                        .min_size(Vec2::new(96.0, 32.0));
                        if ui.add(primary).clicked() {
                            *confirmed = true;
                            *open = false;
                        }
                        ui.add_space(8.0);
                        let secondary = egui::Button::new(
                            RichText::new("Cancel").size(13.0).color(MODAL_TEXT_MUTED),
                        )
                        .fill(Color32::TRANSPARENT)
                        .stroke(Stroke::new(1.0, MODAL_BORDER))
                        .corner_radius(CornerRadius::same(6))
                        .min_size(Vec2::new(96.0, 32.0));
                        if ui.add(secondary).clicked() {
                            *open = false;
                        }
                    });
                });
        });
    if modal.should_close() {
        *open = false;
    }
}
