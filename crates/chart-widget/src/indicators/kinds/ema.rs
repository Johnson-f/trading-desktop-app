use egui::{Painter, Pos2, Rect, Shape, Stroke};

use chart_core::{
    CandleData, ComputedSeries, InputSpec, LegendEntry, ParamField, ParamKind, ParamSchema,
    ParamValues, Rgba, compute_ema,
};

use super::super::super::camera::Camera;
use super::super::super::util::egui_color;
use super::super::trait_def::{Indicator, RenderTarget};

pub const ID: &str = "ema";
pub const NAME: &str = "EMA";
pub const LIKES: u32 = 36759;

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
                default: Rgba::from_rgb(78, 205, 196),
            },
        },
    ],
};

pub struct Ema;

pub fn factory() -> Box<dyn Indicator> {
    Box::new(Ema)
}

impl Indicator for Ema {
    fn id(&self) -> &'static str {
        ID
    }
    fn display_name(&self, params: &ParamValues) -> String {
        format!("EMA({})", params.int("period"))
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
        let series = compute_ema(closes, period);
        let mut out = ComputedSeries::default();
        out.series.insert("ema", series);
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
        let Some(series) = computed.series.get("ema") else {
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
        let Some(series) = computed.series.get("ema") else {
            return Vec::new();
        };
        if series.is_empty() {
            return Vec::new();
        }
        let idx = cursor_idx.unwrap_or(series.len() - 1).min(series.len() - 1);
        match series.get(idx).and_then(|v| *v) {
            Some(v) => vec![LegendEntry {
                label: format!("EMA{} {:.2}", params.int("period"), v),
                color: params.color("color"),
            }],
            None => Vec::new(),
        }
    }
}
