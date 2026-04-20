use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2};

use zaned_chart_core::{
    CandleData, ComputedSeries, InputSpec, LegendEntry, ParamField, ParamKind, ParamSchema,
    ParamValues, Rgba, compute_ppo,
};

use super::super::super::util::egui_color;
use super::super::trait_def::{Indicator, RenderTarget};

pub const ID: &str = "ppo";
pub const NAME: &str = "PPO";
pub const LIKES: u32 = 3421;

pub static SCHEMA: ParamSchema = ParamSchema {
    fields: &[
        ParamField {
            key: "fast",
            label: "Fast",
            kind: ParamKind::Int {
                default: 12,
                min: 2,
                max: 100,
            },
        },
        ParamField {
            key: "slow",
            label: "Slow",
            kind: ParamKind::Int {
                default: 26,
                min: 2,
                max: 100,
            },
        },
        ParamField {
            key: "signal",
            label: "Signal",
            kind: ParamKind::Int {
                default: 9,
                min: 2,
                max: 100,
            },
        },
        ParamField {
            key: "ppo_color",
            label: "PPO color",
            kind: ParamKind::Color {
                default: Rgba::from_rgb(76, 175, 80),
            },
        },
        ParamField {
            key: "signal_color",
            label: "Signal color",
            kind: ParamKind::Color {
                default: Rgba::from_rgb(255, 152, 0),
            },
        },
        ParamField {
            key: "histogram_color",
            label: "Histogram color",
            kind: ParamKind::Color {
                default: Rgba::from_rgb(129, 199, 132),
            },
        },
    ],
};

pub struct Ppo;

pub fn factory() -> Box<dyn Indicator> {
    Box::new(Ppo)
}

impl Indicator for Ppo {
    fn id(&self) -> &'static str {
        ID
    }

    fn display_name(&self, params: &ParamValues) -> String {
        format!(
            "PPO({},{},{})",
            params.int("fast"),
            params.int("slow"),
            params.int("signal")
        )
    }

    fn target(&self) -> RenderTarget {
        RenderTarget::SubPane
    }

    fn inputs(&self, _params: &ParamValues) -> Vec<InputSpec> {
        vec![InputSpec::Closes]
    }

    fn compute(&self, inputs: &[&[f32]], params: &ParamValues) -> ComputedSeries {
        let closes = inputs.first().copied().unwrap_or(&[]);
        let fast = params.int("fast").max(2) as usize;
        let slow = params.int("slow").max(2) as usize;
        let signal = params.int("signal").max(2) as usize;
        let (ppo_line, signal_line, histogram) = compute_ppo(closes, fast, slow, signal);
        let mut out = ComputedSeries::default();
        out.series.insert("ppo", ppo_line);
        out.series.insert("signal", signal_line);
        out.series.insert("histogram", histogram);
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
        // Find y range across all three series for scaling
        let mut y_min: f32 = 0.0;
        let mut y_max: f32 = 0.0;
        for key in ["ppo", "signal", "histogram"] {
            if let Some(series) = computed.series.get(key) {
                for v in series.iter().flatten() {
                    if *v < y_min {
                        y_min = *v;
                    }
                    if *v > y_max {
                        y_max = *v;
                    }
                }
            }
        }
        let range = (y_max - y_min).max(1e-6);
        let y_of = |v: f32| -> f32 {
            pane_rect.top() + ((y_max - v) / range) * pane_rect.height()
        };

        // Draw zero line
        let zero_y = y_of(0.0);
        if zero_y >= pane_rect.top() && zero_y <= pane_rect.bottom() {
            painter.line_segment(
                [
                    Pos2::new(pane_rect.left(), zero_y),
                    Pos2::new(pane_rect.right(), zero_y),
                ],
                Stroke::new(0.5, Color32::from_rgb(60, 60, 66)),
            );
        }

        // Draw histogram bars
        if let Some(hist) = computed.series.get("histogram") {
            let hist_color = egui_color(params.color("histogram_color"));
            let bar_width = if hist.len() >= 2 {
                ((x_mapper(1.0) - x_mapper(0.0)).abs() * 0.5).max(1.0)
            } else {
                1.0
            };
            for (i, v) in hist.iter().enumerate() {
                if let Some(val) = v {
                    let x = x_mapper(i as f32);
                    if x < pane_rect.left() - 50.0 || x > pane_rect.right() + 50.0 {
                        continue;
                    }
                    let top = y_of(*val);
                    let bottom = zero_y;
                    let (min_y, max_y) = if top < bottom {
                        (top, bottom)
                    } else {
                        (bottom, top)
                    };
                    let bar_rect = Rect::from_min_size(
                        Pos2::new(x - bar_width / 2.0, min_y),
                        Vec2::new(bar_width, (max_y - min_y).max(1.0)),
                    );
                    let alpha = if *val >= 0.0 { 180u8 } else { 120 };
                    let c = Color32::from_rgba_premultiplied(
                        hist_color.r(),
                        hist_color.g(),
                        hist_color.b(),
                        alpha,
                    );
                    painter.rect_filled(bar_rect, 0.0, c);
                }
            }
        }

        // Draw PPO and signal lines
        for (key, color_key) in [("ppo", "ppo_color"), ("signal", "signal_color")] {
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
            ("ppo", "PPO", "ppo_color"),
            ("signal", "Signal", "signal_color"),
            ("histogram", "Hist", "histogram_color"),
        ] {
            if let Some(series) = computed.series.get(key) {
                if series.is_empty() {
                    continue;
                }
                let idx = cursor_idx.unwrap_or(series.len() - 1).min(series.len() - 1);
                if let Some(Some(v)) = series.get(idx) {
                    entries.push(LegendEntry {
                        label: format!("{} {:.4}", label, v),
                        color: params.color(color_key),
                    });
                }
            }
        }
        entries
    }
}
