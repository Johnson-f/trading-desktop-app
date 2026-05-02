use egui::{Painter, Pos2, Rect, Shape, Stroke};

use zaned_chart_core::{
    CandleData, ComputedSeries, InputSpec, LegendEntry, ParamField, ParamKind, ParamSchema,
    ParamValues, Rgba, compute_obv,
};

use super::super::super::util::{egui_color, format_volume};
use super::super::trait_def::{Indicator, RenderTarget};

pub const ID: &str = "obv";
pub const NAME: &str = "OBV";
pub const LIKES: u32 = 9876;

pub static SCHEMA: ParamSchema = ParamSchema {
    fields: &[ParamField {
        key: "color",
        label: "Color",
        kind: ParamKind::Color {
            default: Rgba::from_rgb(63, 81, 181),
        },
    }],
};

pub struct Obv;

pub fn factory() -> Box<dyn Indicator> {
    Box::new(Obv)
}

impl Indicator for Obv {
    fn id(&self) -> &'static str {
        ID
    }

    fn display_name(&self, _params: &ParamValues) -> String {
        "OBV".to_string()
    }

    fn target(&self) -> RenderTarget {
        RenderTarget::SubPane
    }

    fn inputs(&self, _params: &ParamValues) -> Vec<InputSpec> {
        vec![InputSpec::Closes, InputSpec::Volumes]
    }

    fn compute(&self, inputs: &[&[f32]], _params: &ParamValues) -> ComputedSeries {
        let closes = inputs.first().copied().unwrap_or(&[]);
        let volumes = inputs.get(1).copied().unwrap_or(&[]);
        let series = compute_obv(closes, volumes);
        let mut out = ComputedSeries::default();
        out.series.insert("obv", series);
        out
    }

    fn draw_pane(
        &self,
        painter: &Painter,
        pane_rect: Rect,
        x_mapper: &dyn Fn(f32) -> f32,
        _data: &CandleData,
        computed: &ComputedSeries,
        params: &ParamValues,
    ) {
        let Some(series) = computed.series.get("obv") else {
            return;
        };

        // Auto-scaled: find min/max of series
        let mut y_min: f32 = f32::MAX;
        let mut y_max: f32 = f32::MIN;
        for v in series.iter().flatten() {
            if *v < y_min {
                y_min = *v;
            }
            if *v > y_max {
                y_max = *v;
            }
        }
        if y_min == f32::MAX || y_max == f32::MIN {
            return;
        }
        let range = (y_max - y_min).max(1e-6);
        let y_of = |v: f32| -> f32 { pane_rect.top() + ((y_max - v) / range) * pane_rect.height() };

        let color = egui_color(params.color("color"));
        let mut current: Vec<Pos2> = Vec::with_capacity(series.len());
        for (i, v) in series.iter().enumerate() {
            match v {
                Some(val) => {
                    let x = x_mapper(i as f32);
                    if x < pane_rect.left() - 50.0 || x > pane_rect.right() + 50.0 {
                        if !current.is_empty() {
                            painter.add(Shape::line(
                                std::mem::take(&mut current),
                                Stroke::new(1.5, color),
                            ));
                        }
                        continue;
                    }
                    current.push(Pos2::new(x, y_of(*val)));
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
        let Some(series) = computed.series.get("obv") else {
            return Vec::new();
        };
        if series.is_empty() {
            return Vec::new();
        }
        let idx = cursor_idx.unwrap_or(series.len() - 1).min(series.len() - 1);
        match series.get(idx).and_then(|v| *v) {
            Some(v) => vec![LegendEntry {
                label: format!("OBV {}", format_volume(v)),
                color: params.color("color"),
            }],
            None => Vec::new(),
        }
    }
}
