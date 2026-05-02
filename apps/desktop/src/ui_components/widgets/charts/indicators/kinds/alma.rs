use egui::{Painter, Pos2, Rect, Shape, Stroke};

use zaned_chart_core::{
    CandleData, ComputedSeries, InputSpec, LegendEntry, ParamField, ParamKind, ParamSchema,
    ParamValues, Rgba, compute_alma,
};

use super::super::super::camera::Camera;
use super::super::super::util::egui_color;
use super::super::trait_def::{Indicator, RenderTarget};

pub const ID: &str = "alma";
pub const NAME: &str = "ALMA";
pub const LIKES: u32 = 6543;

pub static SCHEMA: ParamSchema = ParamSchema {
    fields: &[
        ParamField {
            key: "period",
            label: "Period",
            kind: ParamKind::Int {
                default: 9,
                min: 2,
                max: 200,
            },
        },
        ParamField {
            key: "offset",
            label: "Offset",
            kind: ParamKind::Float {
                default: 0.85,
                min: 0.0,
                max: 1.0,
            },
        },
        ParamField {
            key: "sigma",
            label: "Sigma",
            kind: ParamKind::Float {
                default: 6.0,
                min: 1.0,
                max: 20.0,
            },
        },
        ParamField {
            key: "color",
            label: "Color",
            kind: ParamKind::Color {
                default: Rgba::from_rgb(156, 39, 176),
            },
        },
    ],
};

pub struct Alma;

pub fn factory() -> Box<dyn Indicator> {
    Box::new(Alma)
}

impl Indicator for Alma {
    fn id(&self) -> &'static str {
        ID
    }
    fn display_name(&self, params: &ParamValues) -> String {
        format!(
            "ALMA({},{:.2},{:.1})",
            params.int("period"),
            params.float("offset"),
            params.float("sigma")
        )
    }
    fn target(&self) -> RenderTarget {
        RenderTarget::MainOverlay
    }

    fn inputs(&self, _params: &ParamValues) -> Vec<InputSpec> {
        vec![InputSpec::Closes]
    }

    fn compute(&self, inputs: &[&[f32]], params: &ParamValues) -> ComputedSeries {
        let closes = inputs.first().copied().unwrap_or(&[]);
        let period = params.int("period").max(2) as usize;
        let series = compute_alma(
            closes,
            period,
            params.float("offset"),
            params.float("sigma"),
        );
        let mut out = ComputedSeries::default();
        out.series.insert("alma", series);
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
        let Some(series) = computed.series.get("alma") else {
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
        let Some(series) = computed.series.get("alma") else {
            return Vec::new();
        };
        if series.is_empty() {
            return Vec::new();
        }
        let idx = cursor_idx.unwrap_or(series.len() - 1).min(series.len() - 1);
        match series.get(idx).and_then(|v| *v) {
            Some(v) => vec![LegendEntry {
                label: format!("ALMA{} {:.2}", params.int("period"), v),
                color: params.color("color"),
            }],
            None => Vec::new(),
        }
    }
}
