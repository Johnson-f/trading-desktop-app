use egui::{Color32, CornerRadius, DragValue, RichText, Stroke, Ui};

use super::super::super::indicators::{
    ParamField, ParamKind, ParamSchema, ParamValue, ParamValues,
};

const TEXT_WHITE: Color32 = Color32::from_rgb(240, 240, 242);
const TEXT_MUTED: Color32 = Color32::from_rgb(160, 160, 170);
const BG: Color32 = Color32::from_rgb(24, 24, 28);
const BORDER: Color32 = Color32::from_rgb(50, 50, 55);

pub enum ParamsResponse {
    Open,
    Confirm,
    Cancel,
}

pub fn show(
    ui: &mut Ui,
    schema: &ParamSchema,
    values: &mut ParamValues,
    confirm_label: &str,
) -> ParamsResponse {
    let mut response = ParamsResponse::Open;

    egui::Frame::new()
        .fill(BG)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_min_width(280.0);

            for field in schema.fields {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(field.label).size(11.0).color(TEXT_MUTED));
                    ui.add_space(8.0);
                    render_field(ui, field, values);
                });
                ui.add_space(4.0);
            }

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let cancel = egui_shadcn::Button::new(
                    RichText::new("Cancel").size(11.0).color(TEXT_MUTED),
                )
                .variant(egui_shadcn::ButtonVariant::Outline)
                .show(ui, crate::shadcn_theme::theme());
                if cancel.clicked() {
                    response = ParamsResponse::Cancel;
                }

                let confirm = egui_shadcn::Button::new(
                    RichText::new(confirm_label).size(11.0).color(TEXT_WHITE),
                )
                .variant(egui_shadcn::ButtonVariant::Default)
                .show(ui, crate::shadcn_theme::theme());
                if confirm.clicked() {
                    response = ParamsResponse::Confirm;
                }
            });
        });

    response
}

fn render_field(ui: &mut Ui, field: &ParamField, values: &mut ParamValues) {
    match field.kind {
        ParamKind::Int { min, max, .. } => {
            let current = match values.0.get(field.key) {
                Some(ParamValue::Int(v)) => *v,
                _ => return,
            };
            let mut v = current;
            ui.add(DragValue::new(&mut v).range(min..=max).speed(1));
            if v != current {
                values.0.insert(field.key, ParamValue::Int(v));
            }
        }
        ParamKind::Float { min, max, .. } => {
            let current = match values.0.get(field.key) {
                Some(ParamValue::Float(v)) => *v,
                _ => return,
            };
            let mut v = current;
            ui.add(DragValue::new(&mut v).range(min..=max).speed(0.01));
            if (v - current).abs() > f32::EPSILON {
                values.0.insert(field.key, ParamValue::Float(v));
            }
        }
        ParamKind::Color { .. } => {
            use super::super::super::util::{core_color, egui_color};
            let current = match values.0.get(field.key) {
                Some(ParamValue::Color(c)) => *c,
                _ => return,
            };
            let mut c = egui_color(current);
            if egui::color_picker::color_edit_button_srgba(
                ui,
                &mut c,
                egui::color_picker::Alpha::Opaque,
            )
            .changed()
            {
                values.0.insert(field.key, ParamValue::Color(core_color(c)));
            }
        }
    }
}
