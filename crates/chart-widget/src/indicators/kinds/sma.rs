use egui::{Painter, Pos2, Rect, Shape, Stroke};

use chart_core::{
    CandleData, ComputedSeries, InputSpec, LegendEntry, ParamField, ParamKind, ParamSchema,
    ParamValues, Rgba, compute_sma,
};

use super::super::super::camera::Camera;
use super::super::super::util::egui_color;
use super::super::trait_def::{Indicator, RenderTarget};

pub const ID: &str = "sma";
pub const NAME: &str = "SMA";
pub const LIKES: u32 = 31204;

pub static SCHEMA: ParamSchema = ParamSchema {
    fields: &[
        ParamField {
            key: "period",
            label: "Period",
            kind: ParamKind::Int {
                default: 20,
                min: 1,
                max: 500,
            },
        },
        ParamField {
            key: "color",
            label: "Color",
            kind: ParamKind::Color {
                default: Rgba::from_rgb(255, 152, 0),
            },
        },
    ],
};

pub struct Sma;

pub fn factory() -> Box<dyn Indicator> {
    Box::new(Sma)
}

impl Indicator for Sma {
    fn id(&self) -> &'static str {
        ID
    }
    fn display_name(&self, params: &ParamValues) -> String {
        format!("SMA({})", params.int("period"))
    }
    fn target(&self) -> RenderTarget {
        RenderTarget::MainOverlay
    }

    fn inputs(&self, _params: &ParamValues) -> Vec<InputSpec> {
        vec![InputSpec::Closes]
    }

    fn compute(&self, inputs: &[&[f32]], params: &ParamValues) -> ComputedSeries {
        let closes = inputs.first().copied().unwrap_or(&[]);
        let period = params.int("period").max(1) as usize;
        let series = compute_sma(closes, period);
        let mut out = ComputedSeries::default();
        out.series.insert("sma", series);
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
        let Some(series) = computed.series.get("sma") else {
            return;
        };
        let color = egui_color(params.color("color"));
        let mut current: Vec<Pos2> = Vec::with_capacity(series.len());
        for (i, v) in series.iter().enumerate() {
            match v {
                Some(y) => {
                    let x_pixel = ((i as f64 - camera.x_offset) * camera.x_scale) as f32;
                    let x = rect.left() + x_pixel;
                    if x < rect.left() - 50.0 || x > rect.right() + 50.0 {
                        if !current.is_empty() {
                            painter.add(Shape::line(
                                std::mem::take(&mut current),
                                Stroke::new(1.5, color),
                            ));
                        }
                        continue;
                    }
                    let y_pixel = (*y as f64 - camera.y_offset) * camera.y_scale;
                    let y = rect.bottom() - y_pixel as f32;
                    current.push(Pos2::new(x, y));
                }
                None => {
                    if !current.is_empty() {
                        painter.add(Shape::line(
                            std::mem::take(&mut current),
                            Stroke::new(1.5, color),
                        ));
                    }
                }
            }
        }
        if current.len() >= 2 {
            painter.add(Shape::line(current, Stroke::new(1.5, color)));
        }
    }

    fn legend(
        &self,
        _data: &CandleData,
        computed: &ComputedSeries,
        params: &ParamValues,
        cursor_idx: Option<usize>,
    ) -> Vec<LegendEntry> {
        let Some(series) = computed.series.get("sma") else {
            return Vec::new();
        };
        if series.is_empty() {
            return Vec::new();
        }
        let idx = cursor_idx.unwrap_or(series.len() - 1).min(series.len() - 1);
        match series.get(idx).and_then(|v| *v) {
            Some(v) => vec![LegendEntry {
                label: format!("SMA{} {:.2}", params.int("period"), v),
                color: params.color("color"),
            }],
            None => Vec::new(),
        }
    }
}
