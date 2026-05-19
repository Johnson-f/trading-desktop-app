use egui::{Painter, Pos2, Rect, Shape, Stroke};

use chart_core::{
    CandleData, ComputedSeries, InputSpec, LegendEntry, ParamField, ParamKind, ParamSchema,
    ParamValues, Rgba, compute_chandelier,
};

use super::super::super::camera::Camera;
use super::super::super::util::egui_color;
use super::super::trait_def::{Indicator, RenderTarget};

pub const ID: &str = "chandelier";
pub const NAME: &str = "Chandelier Exit";
pub const LIKES: u32 = 5678;

pub static SCHEMA: ParamSchema = ParamSchema {
    fields: &[
        ParamField {
            key: "period",
            label: "Period",
            kind: ParamKind::Int {
                default: 22,
                min: 5,
                max: 100,
            },
        },
        ParamField {
            key: "multiplier",
            label: "Multiplier",
            kind: ParamKind::Float {
                default: 3.0,
                min: 1.0,
                max: 10.0,
            },
        },
        ParamField {
            key: "long_color",
            label: "Long Color",
            kind: ParamKind::Color {
                default: Rgba::from_rgb(76, 175, 80),
            },
        },
        ParamField {
            key: "short_color",
            label: "Short Color",
            kind: ParamKind::Color {
                default: Rgba::from_rgb(244, 67, 54),
            },
        },
    ],
};

pub struct Chandelier;

pub fn factory() -> Box<dyn Indicator> {
    Box::new(Chandelier)
}

impl Indicator for Chandelier {
    fn id(&self) -> &'static str {
        ID
    }

    fn display_name(&self, params: &ParamValues) -> String {
        format!(
            "CE({}, {})",
            params.int("period"),
            params.float("multiplier")
        )
    }

    fn target(&self) -> RenderTarget {
        RenderTarget::MainOverlay
    }

    fn inputs(&self, _params: &ParamValues) -> Vec<InputSpec> {
        vec![InputSpec::Highs, InputSpec::Lows, InputSpec::Closes]
    }

    fn compute(&self, inputs: &[&[f32]], params: &ParamValues) -> ComputedSeries {
        let highs = inputs.first().copied().unwrap_or(&[]);
        let lows = inputs.get(1).copied().unwrap_or(&[]);
        let closes = inputs.get(2).copied().unwrap_or(&[]);
        let period = params.int("period").max(1) as usize;
        let multiplier = params.float("multiplier");
        let (long, short) = compute_chandelier(highs, lows, closes, period, multiplier);
        let mut out = ComputedSeries::default();
        out.series.insert("long", long);
        out.series.insert("short", short);
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
        let long_color = egui_color(params.color("long_color"));
        let short_color = egui_color(params.color("short_color"));

        for (key, color) in [("long", long_color), ("short", short_color)] {
            let Some(series) = computed.series.get(key) else {
                continue;
            };
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
                        let screen_y = rect.bottom() - y_pixel as f32;
                        current.push(Pos2::new(x, screen_y));
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
    }

    fn legend(
        &self,
        _data: &CandleData,
        computed: &ComputedSeries,
        params: &ParamValues,
        cursor_idx: Option<usize>,
    ) -> Vec<LegendEntry> {
        let mut entries = Vec::new();

        for (key, label, color_key) in [
            ("long", "CE Long", "long_color"),
            ("short", "CE Short", "short_color"),
        ] {
            if let Some(series) = computed.series.get(key) {
                if series.is_empty() {
                    continue;
                }
                let idx = cursor_idx.unwrap_or(series.len() - 1).min(series.len() - 1);
                if let Some(Some(v)) = series.get(idx) {
                    entries.push(LegendEntry {
                        label: format!("{} {:.2}", label, v),
                        color: params.color(color_key),
                    });
                }
            }
        }

        entries
    }
}
