use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke};

use zaned_chart_core::{
    compute_rsi, CandleData, ComputedSeries, InputSpec, LegendEntry, ParamField, ParamKind,
    ParamSchema, ParamValues, Rgba,
};

use super::super::super::util::egui_color;
use super::super::trait_def::{Indicator, RenderTarget};

pub const ID: &str = "rsi";
pub const NAME: &str = "RSI";
pub const LIKES: u32 = 26296;

const GUIDE_COLOR: Color32 = Color32::from_rgb(60, 60, 66);

pub static SCHEMA: ParamSchema = ParamSchema {
    fields: &[
        ParamField {
            key: "period",
            label: "Period",
            kind: ParamKind::Int { default: 14, min: 2, max: 100 },
        },
        ParamField {
            key: "color",
            label: "Color",
            kind: ParamKind::Color { default: Rgba::from_rgb(186, 127, 246) },
        },
    ],
};

pub struct Rsi;

pub fn factory() -> Box<dyn Indicator> {
    Box::new(Rsi)
}

impl Indicator for Rsi {
    fn id(&self) -> &'static str { ID }
    fn display_name(&self, params: &ParamValues) -> String {
        format!("RSI({})", params.int("period"))
    }
    fn target(&self) -> RenderTarget { RenderTarget::SubPane }

    fn inputs(&self, _params: &ParamValues) -> Vec<InputSpec> {
        vec![InputSpec::Closes]
    }

    fn compute(&self, inputs: &[&[f32]], params: &ParamValues) -> ComputedSeries {
        let closes = inputs.first().copied().unwrap_or(&[]);
        let period = params.int("period").max(2) as usize;
        let series = compute_rsi(closes, period);
        let mut out = ComputedSeries::default();
        out.series.insert("rsi", series);
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
        let y_of = |v: f32| -> f32 {
            pane_rect.top() + (1.0 - (v / 100.0).clamp(0.0, 1.0)) * pane_rect.height()
        };

        for v in [30.0f32, 70.0f32] {
            let y = y_of(v);
            painter.line_segment(
                [Pos2::new(pane_rect.left(), y), Pos2::new(pane_rect.right(), y)],
                Stroke::new(0.5, GUIDE_COLOR),
            );
        }

        let Some(series) = computed.series.get("rsi") else { return };
        let color = egui_color(params.color("color"));
        let mut current: Vec<Pos2> = Vec::with_capacity(series.len());
        for (i, v) in series.iter().enumerate() {
            match v {
                Some(val) => {
                    let x = x_mapper(i as f32);
                    if x < pane_rect.left() - 50.0 || x > pane_rect.right() + 50.0 {
                        if !current.is_empty() {
                            painter.add(Shape::line(std::mem::take(&mut current), Stroke::new(1.5, color)));
                        }
                        continue;
                    }
                    current.push(Pos2::new(x, y_of(*val)));
                }
                None => {
                    if !current.is_empty() {
                        painter.add(Shape::line(std::mem::take(&mut current), Stroke::new(1.5, color)));
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
        let Some(series) = computed.series.get("rsi") else { return Vec::new() };
        if series.is_empty() { return Vec::new(); }
        let idx = cursor_idx.unwrap_or(series.len() - 1).min(series.len() - 1);
        match series.get(idx).and_then(|v| *v) {
            Some(v) => vec![LegendEntry {
                label: format!("RSI {:.2}", v),
                color: params.color("color"),
            }],
            None => Vec::new(),
        }
    }
}
