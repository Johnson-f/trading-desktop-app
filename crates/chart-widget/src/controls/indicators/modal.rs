use eframe::egui;

use super::super::super::indicators::{self, IndicatorManager, ParamValues};
use super::params_popover::{ParamsResponse, show as show_params_popover};

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
    pub editor: Option<ModalEditor>,
}

impl Default for IndicatorModal {
    fn default() -> Self {
        Self {
            open: false,
            editor: None,
        }
    }
}

// ── Main impl ──────────────────────────────────────────────────────────────────

impl IndicatorModal {
    pub fn show(&mut self, ui: &mut egui::Ui, manager: &mut IndicatorManager) {
        // Render the params-popover first so it stays alive even after a click
        // closes the picker dialog (`self.open = false` is set when an item is
        // selected).
        self.show_active_editor(ui, manager);

        if !self.open {
            return;
        }

        let theme = crate::shadcn_theme::theme();
        // The dialog body captures `&mut self`, so we can't also pass
        // `&mut self.open` into `CommandDialogProps`. Mirror it through a
        // local `bool`, then write back after the call. (Same pattern as
        // settings_modal / drawing_settings_modal in Tasks 3-4.)
        let mut open = self.open;
        let mut newly_selected: Option<&'static str> = None;

        egui_shadcn::command_dialog(
            ui,
            theme,
            egui_shadcn::CommandDialogProps::new(
                egui::Id::new("indicator_picker_dialog"),
                &mut open,
            )
            .title("Add indicator")
            .description("Search to find a study."),
            |ui, cmd| {
                egui_shadcn::command_input(
                    ui,
                    cmd,
                    egui_shadcn::CommandInputProps::new("Search indicators..."),
                );
                egui_shadcn::command_list(ui, cmd, Default::default(), |ui, cmd| {
                    egui_shadcn::command_empty(ui, cmd, "No indicators match your search.");

                    for category in CATEGORIES {
                        egui_shadcn::command_group(
                            ui,
                            cmd,
                            egui_shadcn::CommandGroupProps::new(*category),
                            |ui, cmd| {
                                for def in indicators::all() {
                                    if indicator_category(def.id) != *category {
                                        continue;
                                    }
                                    let resp = egui_shadcn::command_item(
                                        ui,
                                        cmd,
                                        egui_shadcn::CommandItemProps::new(def.id, def.name),
                                    );
                                    if let Some(r) = resp {
                                        if r.clicked() {
                                            newly_selected = Some(def.id);
                                        }
                                    }
                                }
                            },
                        );
                    }
                });
            },
        );

        self.open = open;

        if let Some(def_id) = newly_selected {
            if let Some(def) = indicators::get(def_id) {
                self.editor = Some(ModalEditor::Add {
                    def_id,
                    draft: def.params.defaults(),
                });
                self.open = false;
            }
        }
    }

    // ── Params popover (preserved verbatim from the old modal body) ──────────

    fn show_active_editor(&mut self, ui: &mut egui::Ui, manager: &mut IndicatorManager) {
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
}
