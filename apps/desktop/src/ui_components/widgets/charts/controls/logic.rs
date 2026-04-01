use eframe::egui::{self, Color32, CornerRadius, RichText, Stroke, Vec2};

use super::{IndicatorBar, IndicatorModal};

const BG: Color32 = Color32::from_rgb(18, 18, 22);
const BORDER: Color32 = Color32::from_rgb(30, 30, 33);
const ICON_COLOR: Color32 = Color32::from_rgb(120, 120, 130);
const ICON_HOVER: Color32 = Color32::from_rgb(200, 200, 210);
const HOVER_BG: Color32 = Color32::from_rgb(28, 28, 32);
const ICON_ACTIVE_BG: Color32 = Color32::from_rgb(35, 35, 40);

const TOOLBAR_ICONS: &[&str] = &[
    egui_phosphor::regular::TREND_UP,       // 0 - Indicator
    egui_phosphor::regular::PENCIL_SIMPLE,  // 1 - Drawings
    egui_phosphor::regular::PAINT_BUCKET,   // 2
    egui_phosphor::regular::CHART_BAR,      // 3 - Line Style
    egui_phosphor::regular::CHART_LINE,     // 4
    egui_phosphor::regular::TEXT_T,         // 5
    egui_phosphor::regular::CLOUD,          // 6
    egui_phosphor::regular::CURRENCY_DOLLAR,// 7
    egui_phosphor::regular::SMILEY,         // 8
    egui_phosphor::regular::CROSSHAIR,      // 9
    egui_phosphor::regular::MAGNET,         // 10
    egui_phosphor::regular::LOCK_SIMPLE,    // 11
    egui_phosphor::regular::EYE,            // 12
    egui_phosphor::regular::TRASH,          // 13
];

const INDICATOR_TOOL_INDEX: usize = 0;
const INDICATOR_INSERT_AFTER: usize = 9; // after CROSSHAIR

const SEPARATORS: &[usize] = &[9];

const TOOLTIPS: &[&str] = &[
    "Indicator", "Drawings", "", "Line Style", "", "",
    "", "", "", "", "", "", "", "",
];

pub struct ChartToolbar {
    pub active_tool: usize,
    pub indicator_bar: IndicatorBar,
    pub indicator_modal: IndicatorModal,
    pub show_indicators: bool,
}

impl Default for ChartToolbar {
    fn default() -> Self {
        Self {
            active_tool: 0,
            indicator_bar: IndicatorBar::default(),
            indicator_modal: IndicatorModal::default(),
            show_indicators: false,
        }
    }
}

impl ChartToolbar {
    pub fn show(&mut self, ui: &mut egui::Ui) {
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
                        // Hide trailing icons when indicator bar is expanded
                        if self.show_indicators && i > INDICATOR_INSERT_AFTER {
                            continue;
                        }

                        let is_indicator_btn = i == INDICATOR_TOOL_INDEX;
                        let is_active = if is_indicator_btn {
                            self.show_indicators
                        } else {
                            self.active_tool == i
                        };

                        let fill = if is_active { ICON_ACTIVE_BG } else { Color32::TRANSPARENT };
                        let color = if is_active { ICON_HOVER } else { ICON_COLOR };

                        let btn = ui.add(
                            egui::Button::new(
                                RichText::new(*icon).size(15.0).color(color),
                            )
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
                            } else {
                                self.active_tool = i;
                            }
                        }

                        // Tooltip
                        if let Some(tip) = TOOLTIPS.get(i) {
                            if !tip.is_empty() {
                                btn.clone().on_hover_ui(|ui| {
                                    ui.label(RichText::new(*tip).size(11.0));
                                });
                            }
                        }

                        // Insert indicator bar after CROSSHAIR when expanded
                        if i == INDICATOR_INSERT_AFTER && self.show_indicators {
                            let (_toggled, open_modal) = self.indicator_bar.show_inline(ui);
                            if open_modal {
                                self.indicator_modal.open = true;
                            }
                        }

                        if SEPARATORS.contains(&i) && !(i == INDICATOR_INSERT_AFTER && self.show_indicators) {
                            ui.add_space(2.0);
                            let (rect, _) = ui.allocate_exact_size(
                                Vec2::new(1.0, 18.0),
                                egui::Sense::hover(),
                            );
                            ui.painter().rect_filled(rect, 0.0, BORDER);
                            ui.add_space(2.0);
                        }
                    }
                });
            });

        // Render the indicator modal (floating window)
        self.indicator_modal.show(ui.ctx());
    }
}
