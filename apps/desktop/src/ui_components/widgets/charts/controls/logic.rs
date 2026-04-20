use eframe::egui::{self, Color32, CornerRadius, RichText, Stroke, Vec2};

use super::super::candle::Timeframe;
use super::super::drawings;
use super::super::indicators::IndicatorManager;
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
const INDICATOR_INSERT_AFTER: usize = 9;
const SEPARATORS: &[usize] = &[9];
const TOOLTIPS: &[&str] = &[
    "Indicator",
    "Drawings",
    "",
    "Line Style",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
];

pub struct ChartToolbar {
    pub active_tool: usize,
    pub active_drawing: Option<&'static str>,
    /// User-selected base timeframe. Read by `ChartWidget` each frame; a
    /// change triggers re-aggregation + camera reset.
    pub timeframe: Timeframe,
    pub indicator_bar: IndicatorBar,
    pub indicator_modal: IndicatorModal,
    pub show_indicators: bool,
    pub show_drawings: bool,
}

impl Default for ChartToolbar {
    fn default() -> Self {
        Self {
            active_tool: 0,
            active_drawing: None,
            timeframe: Timeframe::Daily,
            indicator_bar: IndicatorBar::default(),
            indicator_modal: IndicatorModal::default(),
            show_indicators: false,
            show_drawings: false,
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

                    // Timeframe selector (leftmost). Clicking the short-label
                    // pill opens a popup with the full list.
                    draw_timeframe_selector(ui, &mut self.timeframe);

                    ui.add_space(2.0);
                    let (sep_rect, _) =
                        ui.allocate_exact_size(Vec2::new(1.0, 18.0), egui::Sense::hover());
                    ui.painter().rect_filled(sep_rect, 0.0, BORDER);
                    ui.add_space(2.0);

                    for (i, icon) in TOOLBAR_ICONS.iter().enumerate() {
                        if self.show_indicators && i > INDICATOR_INSERT_AFTER {
                            continue;
                        }
                        // When the drawings bar is expanded, hide everything past
                        // the pencil so the inline row has room.
                        if self.show_drawings && i > PENCIL_TOOL_INDEX {
                            continue;
                        }

                        let is_indicator_btn = i == INDICATOR_TOOL_INDEX;
                        let is_pencil_btn = i == PENCIL_TOOL_INDEX;
                        let is_active = if is_indicator_btn {
                            self.show_indicators
                        } else if is_pencil_btn {
                            self.show_drawings || self.active_drawing.is_some()
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
                            } else if is_pencil_btn {
                                self.show_drawings = !self.show_drawings;
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

                        if i == INDICATOR_INSERT_AFTER && self.show_indicators {
                            if let Some(ev) = self.indicator_bar.show_inline(ui, manager) {
                                bar_event = Some(ev);
                            }
                        }

                        if SEPARATORS.contains(&i)
                            && !(i == INDICATOR_INSERT_AFTER && self.show_indicators)
                            && !(i == PENCIL_TOOL_INDEX && self.show_drawings)
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

        bar_event
    }
}

/// Render the timeframe selector — a compact pill showing the current
/// timeframe's short label (e.g. "1D"). Click opens a popup with all
/// available timeframes; selecting one updates `*timeframe`.
fn draw_timeframe_selector(ui: &mut egui::Ui, timeframe: &mut Timeframe) {
    let btn = ui.add(
        egui::Button::new(
            RichText::new(timeframe.short_label())
                .size(12.0)
                .color(ICON_HOVER),
        )
        .fill(ICON_ACTIVE_BG)
        .corner_radius(CornerRadius::same(4))
        .min_size(Vec2::new(36.0, 24.0)),
    );
    if btn.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    egui::Popup::from_toggle_button_response(&btn)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
        .show(|ui| {
            ui.set_min_width(120.0);
            for tf in Timeframe::ALL {
                let selected = *timeframe == *tf;
                let label = format!("{}  {}", tf.short_label(), tf.long_label());
                if ui.selectable_label(selected, label).clicked() {
                    *timeframe = *tf;
                }
            }
        });
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
