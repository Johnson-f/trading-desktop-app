use egui::{Painter, Pos2, Rect, Shape, Stroke};

use zaned_chart_core::{
    CandleData, ComputedSeries, InputSpec, LegendEntry, ParamField, ParamKind, ParamSchema,
    ParamValues, Rgba, compute_supertrend,
};

use super::super::super::camera::Camera;
use super::super::super::util::egui_color;
use super::super::trait_def::{Indicator, RenderTarget};

pub const ID: &str = "supertrend";
pub const NAME: &str = "Supertrend";
pub const LIKES: u32 = 14321;

pub static SCHEMA: ParamSchema = ParamSchema {
    fields: &[
        ParamField {
            key: "period",
            label: "Period",
            kind: ParamKind::Int {
                default: 10,
                min: 1,
                max: 100,
            },
        },
        ParamField {
            key: "multiplier",
            label: "Multiplier",
            kind: ParamKind::Float {
                default: 3.0,
                min: 0.5,
                max: 10.0,
            },
        },
        ParamField {
            key: "up_color",
            label: "Up color",
            kind: ParamKind::Color {
                default: Rgba::from_rgb(76, 175, 80),
            },
        },
        ParamField {
            key: "down_color",
            label: "Down color",
            kind: ParamKind::Color {
                default: Rgba::from_rgb(244, 67, 54),
            },
        },
    ],
};

pub struct Supertrend;

pub fn factory() -> Box<dyn Indicator> {
    Box::new(Supertrend)
}

impl Indicator for Supertrend {
    fn id(&self) -> &'static str {
        ID
    }

    fn display_name(&self, params: &ParamValues) -> String {
        format!(
            "ST({}, {})",
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
        let (st_series, dir_series) = compute_supertrend(highs, lows, closes, period, multiplier);
        let mut out = ComputedSeries::default();
        out.series.insert("supertrend", st_series);
        out.series.insert("direction", dir_series);
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
        let st = computed.series.get("supertrend");
        let dir_series = computed.series.get("direction");
        let (Some(st), Some(dirs)) = (st, dir_series) else {
            return;
        };

        let up_color = egui_color(params.color("up_color"));
        let down_color = egui_color(params.color("down_color"));

        let mut current: Vec<Pos2> = Vec::new();
        let mut current_color = up_color;

        for (i, v) in st.iter().enumerate() {
            let dir = dirs.get(i).and_then(|d| *d).unwrap_or(1.0);
            let new_color = if dir > 0.0 { up_color } else { down_color };

            match v {
                Some(y) => {
                    let x_pixel = ((i as f64 - camera.x_offset) * camera.x_scale) as f32;
                    let x = rect.left() + x_pixel;
                    if x < rect.left() - 50.0 || x > rect.right() + 50.0 {
                        if current.len() >= 2 {
                            painter.add(Shape::line(
                                std::mem::take(&mut current),
                                Stroke::new(2.0, current_color),
                            ));
                        } else {
                            current.clear();
                        }
                        current_color = new_color;
                        continue;
                    }

                    let y_pixel = (*y as f64 - camera.y_offset) * camera.y_scale;
                    let pos = Pos2::new(x, rect.bottom() - y_pixel as f32);

                    // Color changed — flush segment
                    if new_color != current_color && !current.is_empty() {
                        // Add this point to end of old segment for continuity
                        current.push(pos);
                        if current.len() >= 2 {
                            painter.add(Shape::line(
                                std::mem::take(&mut current),
                                Stroke::new(2.0, current_color),
                            ));
                        } else {
                            current.clear();
                        }
                        current_color = new_color;
                        current.push(pos); // Start new segment from this point
                    } else {
                        current_color = new_color;
                        current.push(pos);
                    }
                }
                None => {
                    if current.len() >= 2 {
                        painter.add(Shape::line(
                            std::mem::take(&mut current),
                            Stroke::new(2.0, current_color),
                        ));
                    } else {
                        current.clear();
                    }
                }
            }
        }
        if current.len() >= 2 {
            painter.add(Shape::line(current, Stroke::new(2.0, current_color)));
        }
    }

    fn legend(
        &self,
        _data: &CandleData,
        computed: &ComputedSeries,
        params: &ParamValues,
        cursor_idx: Option<usize>,
    ) -> Vec<LegendEntry> {
        let dir_series = computed.series.get("direction");
        let st_series = computed.series.get("supertrend");
        let (Some(dirs), Some(sts)) = (dir_series, st_series) else {
            return Vec::new();
        };
        if sts.is_empty() {
            return Vec::new();
        }
        let idx = cursor_idx.unwrap_or(sts.len() - 1).min(sts.len() - 1);
        let dir = dirs.get(idx).and_then(|d| *d).unwrap_or(1.0);
        let (label, color_key) = if dir > 0.0 {
            ("ST\u{2191}", "up_color")
        } else {
            ("ST\u{2193}", "down_color")
        };
        vec![LegendEntry {
            label: label.to_string(),
            color: params.color(color_key),
        }]
    }
}
