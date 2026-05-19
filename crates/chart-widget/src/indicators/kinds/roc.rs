use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke};

use chart_core::{
    CandleData, ComputedSeries, InputSpec, LegendEntry, ParamField, ParamKind, ParamSchema,
    ParamValues, Rgba, compute_roc,
};

use super::super::super::util::egui_color;
use super::super::trait_def::{Indicator, RenderTarget};

pub const ID: &str = "roc";
pub const NAME: &str = "ROC";
pub const LIKES: u32 = 5432;

const GUIDE_COLOR: Color32 = Color32::from_rgb(60, 60, 66);

pub static SCHEMA: ParamSchema = ParamSchema {
    fields: &[
        ParamField {
            key: "period",
            label: "Period",
            kind: ParamKind::Int {
                default: 12,
                min: 1,
                max: 200,
            },
        },
        ParamField {
            key: "color",
            label: "Color",
            kind: ParamKind::Color {
                default: Rgba::from_rgb(121, 85, 72),
            },
        },
    ],
};

pub struct Roc;

pub fn factory() -> Box<dyn Indicator> {
    Box::new(Roc)
}

impl Indicator for Roc {
    fn id(&self) -> &'static str {
        ID
    }

    fn display_name(&self, params: &ParamValues) -> String {
        format!("ROC({})", params.int("period"))
    }

    fn target(&self) -> RenderTarget {
        RenderTarget::SubPane
    }

    fn inputs(&self, _params: &ParamValues) -> Vec<InputSpec> {
        vec![InputSpec::Closes]
    }

    fn compute(&self, inputs: &[&[f32]], params: &ParamValues) -> ComputedSeries {
        let closes = inputs.first().copied().unwrap_or(&[]);
        let period = params.int("period").max(1) as usize;
        let series = compute_roc(closes, period);
        let mut out = ComputedSeries::default();
        out.series.insert("roc", series);
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
        let Some(series) = computed.series.get("roc") else {
            return;
        };

        // Zero-centered auto-scaling
        let mut y_min: f32 = 0.0;
        let mut y_max: f32 = 0.0;
        for v in series.iter().flatten() {
            if *v < y_min {
                y_min = *v;
            }
            if *v > y_max {
                y_max = *v;
            }
        }
        let range = (y_max - y_min).max(1e-6);
        let y_of = |v: f32| -> f32 { pane_rect.top() + ((y_max - v) / range) * pane_rect.height() };

        // Draw zero line
        let zero_y = y_of(0.0);
        if zero_y >= pane_rect.top() && zero_y <= pane_rect.bottom() {
            painter.line_segment(
                [
                    Pos2::new(pane_rect.left(), zero_y),
                    Pos2::new(pane_rect.right(), zero_y),
                ],
                Stroke::new(0.5, GUIDE_COLOR),
            );
        }

        // Draw ROC line
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
        let Some(series) = computed.series.get("roc") else {
            return Vec::new();
        };
        if series.is_empty() {
            return Vec::new();
        }
        let idx = cursor_idx.unwrap_or(series.len() - 1).min(series.len() - 1);
        match series.get(idx).and_then(|v| *v) {
            Some(v) => vec![LegendEntry {
                label: format!("ROC {:.2}", v),
                color: params.color("color"),
            }],
            None => Vec::new(),
        }
    }
}
