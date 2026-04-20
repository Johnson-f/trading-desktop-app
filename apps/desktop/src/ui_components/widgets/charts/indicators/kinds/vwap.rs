use egui::{Painter, Pos2, Rect, Shape, Stroke};

use zaned_chart_core::{
    CandleData, ComputedSeries, InputSpec, LegendEntry, ParamField, ParamKind, ParamSchema,
    ParamValues, Rgba, compute_vwap,
};

use super::super::super::camera::Camera;
use super::super::super::util::egui_color;
use super::super::trait_def::{Indicator, RenderTarget};

pub const ID: &str = "vwap";
pub const NAME: &str = "VWAP";
pub const LIKES: u32 = 18234;

pub static SCHEMA: ParamSchema = ParamSchema {
    fields: &[
        ParamField {
            key: "color",
            label: "Color",
            kind: ParamKind::Color {
                default: Rgba::from_rgb(255, 235, 59),
            },
        },
    ],
};

pub struct Vwap;

pub fn factory() -> Box<dyn Indicator> {
    Box::new(Vwap)
}

impl Indicator for Vwap {
    fn id(&self) -> &'static str {
        ID
    }

    fn display_name(&self, _params: &ParamValues) -> String {
        "VWAP".to_string()
    }

    fn target(&self) -> RenderTarget {
        RenderTarget::MainOverlay
    }

    fn inputs(&self, _params: &ParamValues) -> Vec<InputSpec> {
        vec![InputSpec::Highs, InputSpec::Lows, InputSpec::Closes, InputSpec::Volumes]
    }

    fn compute(&self, _inputs: &[&[f32]], _params: &ParamValues) -> ComputedSeries {
        ComputedSeries::default()
    }

    fn draw_main(
        &self,
        painter: &Painter,
        rect: Rect,
        camera: &Camera,
        data: &CandleData,
        _computed: &ComputedSeries,
        params: &ParamValues,
    ) {
        if data.is_empty() {
            return;
        }

        // Compute VWAP inline (needs dates for session resets)
        let highs: Vec<f32> = data.instances.iter().map(|c| c.high).collect();
        let lows: Vec<f32> = data.instances.iter().map(|c| c.low).collect();
        let closes: Vec<f32> = data.instances.iter().map(|c| c.close).collect();
        let volumes: Vec<f32> = data.instances.iter().map(|c| c.volume).collect();
        let series = compute_vwap(&highs, &lows, &closes, &volumes, &data.dates);

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
        data: &CandleData,
        _computed: &ComputedSeries,
        params: &ParamValues,
        cursor_idx: Option<usize>,
    ) -> Vec<LegendEntry> {
        if data.is_empty() {
            return Vec::new();
        }
        let highs: Vec<f32> = data.instances.iter().map(|c| c.high).collect();
        let lows: Vec<f32> = data.instances.iter().map(|c| c.low).collect();
        let closes: Vec<f32> = data.instances.iter().map(|c| c.close).collect();
        let volumes: Vec<f32> = data.instances.iter().map(|c| c.volume).collect();
        let series = compute_vwap(&highs, &lows, &closes, &volumes, &data.dates);
        if series.is_empty() {
            return Vec::new();
        }
        let idx = cursor_idx.unwrap_or(series.len() - 1).min(series.len() - 1);
        match series.get(idx).and_then(|v| *v) {
            Some(v) => vec![LegendEntry {
                label: format!("VWAP {:.2}", v),
                color: params.color("color"),
            }],
            None => Vec::new(),
        }
    }
}
