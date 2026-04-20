use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke};

use zaned_chart_core::{
    CandleData, ComputedSeries, InputSpec, LegendEntry, ParamField, ParamKind, ParamSchema,
    ParamValues, Rgba, compute_adx,
};

use super::super::super::util::egui_color;
use super::super::trait_def::{Indicator, RenderTarget};

pub const ID: &str = "adx";
pub const NAME: &str = "ADX";
pub const LIKES: u32 = 11234;

const GUIDE_COLOR: Color32 = Color32::from_rgb(60, 60, 66);

pub static SCHEMA: ParamSchema = ParamSchema {
    fields: &[
        ParamField {
            key: "period",
            label: "Period",
            kind: ParamKind::Int {
                default: 14,
                min: 2,
                max: 100,
            },
        },
        ParamField {
            key: "adx_color",
            label: "ADX color",
            kind: ParamKind::Color {
                default: Rgba::from_rgb(200, 200, 200),
            },
        },
        ParamField {
            key: "di_plus_color",
            label: "+DI color",
            kind: ParamKind::Color {
                default: Rgba::from_rgb(76, 175, 80),
            },
        },
        ParamField {
            key: "di_minus_color",
            label: "-DI color",
            kind: ParamKind::Color {
                default: Rgba::from_rgb(244, 67, 54),
            },
        },
    ],
};

pub struct Adx;

pub fn factory() -> Box<dyn Indicator> {
    Box::new(Adx)
}

impl Indicator for Adx {
    fn id(&self) -> &'static str {
        ID
    }

    fn display_name(&self, params: &ParamValues) -> String {
        format!("ADX({})", params.int("period"))
    }

    fn target(&self) -> RenderTarget {
        RenderTarget::SubPane
    }

    fn inputs(&self, _params: &ParamValues) -> Vec<InputSpec> {
        vec![InputSpec::Highs, InputSpec::Lows, InputSpec::Closes]
    }

    fn compute(&self, inputs: &[&[f32]], params: &ParamValues) -> ComputedSeries {
        let highs = inputs.first().copied().unwrap_or(&[]);
        let lows = inputs.get(1).copied().unwrap_or(&[]);
        let closes = inputs.get(2).copied().unwrap_or(&[]);
        let period = params.int("period").max(2) as usize;
        let (adx_series, di_plus_series, di_minus_series) =
            compute_adx(highs, lows, closes, period);
        let mut out = ComputedSeries::default();
        out.series.insert("adx", adx_series);
        out.series.insert("di_plus", di_plus_series);
        out.series.insert("di_minus", di_minus_series);
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
        // Range 0-100, y_of same as RSI
        let y_of = |v: f32| -> f32 {
            pane_rect.top() + (1.0 - (v / 100.0).clamp(0.0, 1.0)) * pane_rect.height()
        };

        // Guide line at 25
        let y25 = y_of(25.0);
        painter.line_segment(
            [
                Pos2::new(pane_rect.left(), y25),
                Pos2::new(pane_rect.right(), y25),
            ],
            Stroke::new(0.5, GUIDE_COLOR),
        );

        // Draw three lines: ADX, +DI, -DI
        for (key, color_key) in [
            ("adx", "adx_color"),
            ("di_plus", "di_plus_color"),
            ("di_minus", "di_minus_color"),
        ] {
            let Some(series) = computed.series.get(key) else {
                continue;
            };
            let color = egui_color(params.color(color_key));
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
            ("adx", "ADX", "adx_color"),
            ("di_plus", "+DI", "di_plus_color"),
            ("di_minus", "-DI", "di_minus_color"),
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
