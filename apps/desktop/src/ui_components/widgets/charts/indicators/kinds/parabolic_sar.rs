use egui::{Painter, Pos2, Rect};

use zaned_chart_core::{
    CandleData, ComputedSeries, InputSpec, LegendEntry, ParamField, ParamKind, ParamSchema,
    ParamValues, Rgba, compute_parabolic_sar,
};

use super::super::super::camera::Camera;
use super::super::super::util::egui_color;
use super::super::trait_def::{Indicator, RenderTarget};

pub const ID: &str = "parabolic_sar";
pub const NAME: &str = "Parabolic SAR";
pub const LIKES: u32 = 13456;

pub static SCHEMA: ParamSchema = ParamSchema {
    fields: &[
        ParamField {
            key: "acceleration",
            label: "Acceleration",
            kind: ParamKind::Float {
                default: 0.02,
                min: 0.001,
                max: 0.1,
            },
        },
        ParamField {
            key: "max_acceleration",
            label: "Max Acceleration",
            kind: ParamKind::Float {
                default: 0.2,
                min: 0.05,
                max: 0.5,
            },
        },
        ParamField {
            key: "color",
            label: "Color",
            kind: ParamKind::Color {
                default: Rgba::from_rgb(171, 71, 188),
            },
        },
    ],
};

pub struct ParabolicSar;

pub fn factory() -> Box<dyn Indicator> {
    Box::new(ParabolicSar)
}

impl Indicator for ParabolicSar {
    fn id(&self) -> &'static str {
        ID
    }

    fn display_name(&self, params: &ParamValues) -> String {
        format!(
            "SAR({:.3}, {:.2})",
            params.float("acceleration"),
            params.float("max_acceleration")
        )
    }

    fn target(&self) -> RenderTarget {
        RenderTarget::MainOverlay
    }

    fn inputs(&self, _params: &ParamValues) -> Vec<InputSpec> {
        vec![InputSpec::Highs, InputSpec::Lows]
    }

    fn compute(&self, inputs: &[&[f32]], params: &ParamValues) -> ComputedSeries {
        let highs = inputs.first().copied().unwrap_or(&[]);
        let lows = inputs.get(1).copied().unwrap_or(&[]);
        let accel_start = params.float("acceleration");
        let accel_max = params.float("max_acceleration");
        let sar = compute_parabolic_sar(highs, lows, accel_start, accel_max);
        let mut out = ComputedSeries::default();
        out.series.insert("sar", sar);
        out
    }

    fn draw_main(
        &self,
        painter: &Painter,
        rect: Rect,
        camera: &Camera,
        _data: &CandleData,
        computed: &ComputedSeries,
        params: &ParamValues,
    ) {
        let Some(series) = computed.series.get("sar") else {
            return;
        };
        let color = egui_color(params.color("color"));

        for (i, v) in series.iter().enumerate() {
            if let Some(y) = v {
                let x_pixel = ((i as f64 - camera.x_offset) * camera.x_scale) as f32;
                let x = rect.left() + x_pixel;
                if x < rect.left() - 50.0 || x > rect.right() + 50.0 {
                    continue;
                }
                let y_pixel = (*y as f64 - camera.y_offset) * camera.y_scale;
                let screen_y = rect.bottom() - y_pixel as f32;
                painter.circle_filled(Pos2::new(x, screen_y), 2.5, color);
            }
        }
    }

    fn legend(
        &self,
        _data: &CandleData,
        computed: &ComputedSeries,
        params: &ParamValues,
        cursor_idx: Option<usize>,
    ) -> Vec<LegendEntry> {
        let Some(series) = computed.series.get("sar") else {
            return Vec::new();
        };
        if series.is_empty() {
            return Vec::new();
        }
        let idx = cursor_idx.unwrap_or(series.len() - 1).min(series.len() - 1);
        match series.get(idx).and_then(|v| *v) {
            Some(v) => vec![LegendEntry {
                label: format!("SAR {:.2}", v),
                color: params.color("color"),
            }],
            None => Vec::new(),
        }
    }
}
