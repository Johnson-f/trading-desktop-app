use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke};

use chart_core::{
    CandleData, ComputedSeries, InputSpec, LegendEntry, ParamField, ParamKind, ParamSchema,
    ParamValues, Rgba, compute_keltner,
};

use super::super::super::camera::Camera;
use super::super::super::util::egui_color;
use super::super::trait_def::{Indicator, RenderTarget};

pub const ID: &str = "keltner";
pub const NAME: &str = "Keltner Channel";
pub const LIKES: u32 = 7234;

pub static SCHEMA: ParamSchema = ParamSchema {
    fields: &[
        ParamField {
            key: "period",
            label: "Period",
            kind: ParamKind::Int {
                default: 20,
                min: 5,
                max: 200,
            },
        },
        ParamField {
            key: "multiplier",
            label: "Multiplier",
            kind: ParamKind::Float {
                default: 2.0,
                min: 0.5,
                max: 5.0,
            },
        },
        ParamField {
            key: "color",
            label: "Color",
            kind: ParamKind::Color {
                default: Rgba::from_rgb(255, 193, 7),
            },
        },
    ],
};

pub struct Keltner;

pub fn factory() -> Box<dyn Indicator> {
    Box::new(Keltner)
}

impl Indicator for Keltner {
    fn id(&self) -> &'static str {
        ID
    }
    fn display_name(&self, params: &ParamValues) -> String {
        format!(
            "KC({}, {})",
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
        let (upper, middle, lower) = compute_keltner(highs, lows, closes, period, multiplier);
        let mut out = ComputedSeries::default();
        out.series.insert("upper", upper);
        out.series.insert("middle", middle);
        out.series.insert("lower", lower);
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
        let color = egui_color(params.color("color"));
        let fill_color = Color32::from_rgba_premultiplied(color.r(), color.g(), color.b(), 20);

        let to_screen = |i: usize, y: f32| -> Pos2 {
            let x_pixel = ((i as f64 - camera.x_offset) * camera.x_scale) as f32;
            let y_pixel = (y as f64 - camera.y_offset) * camera.y_scale;
            Pos2::new(rect.left() + x_pixel, rect.bottom() - y_pixel as f32)
        };

        let upper = computed.series.get("upper");
        let lower = computed.series.get("lower");

        // Draw filled region between upper and lower
        if let (Some(up), Some(lo)) = (upper, lower) {
            let len = up.len().min(lo.len());
            let mut i = 0;
            while i < len {
                let mut upper_pts: Vec<Pos2> = Vec::new();
                let mut lower_pts: Vec<Pos2> = Vec::new();
                while i < len {
                    if let (Some(uv), Some(lv)) = (up[i], lo[i]) {
                        let x_pixel = ((i as f64 - camera.x_offset) * camera.x_scale) as f32;
                        let x = rect.left() + x_pixel;
                        if x >= rect.left() - 50.0 && x <= rect.right() + 50.0 {
                            upper_pts.push(to_screen(i, uv));
                            lower_pts.push(to_screen(i, lv));
                        }
                    } else if !upper_pts.is_empty() {
                        break;
                    }
                    i += 1;
                }
                if upper_pts.len() >= 2 {
                    let mut polygon = upper_pts.clone();
                    polygon.extend(lower_pts.iter().rev());
                    painter.add(Shape::convex_polygon(polygon, fill_color, Stroke::NONE));
                }
            }
        }

        // Draw the three lines
        for (series_key, stroke_width) in [("upper", 1.0f32), ("middle", 1.5), ("lower", 1.0)] {
            let Some(series) = computed.series.get(series_key) else {
                continue;
            };
            let mut current: Vec<Pos2> = Vec::with_capacity(series.len());
            for (i, v) in series.iter().enumerate() {
                match v {
                    Some(y) => {
                        let pt = to_screen(i, *y);
                        if pt.x < rect.left() - 50.0 || pt.x > rect.right() + 50.0 {
                            if !current.is_empty() {
                                painter.add(Shape::line(
                                    std::mem::take(&mut current),
                                    Stroke::new(stroke_width, color),
                                ));
                            }
                            continue;
                        }
                        current.push(pt);
                    }
                    None => {
                        if !current.is_empty() {
                            painter.add(Shape::line(
                                std::mem::take(&mut current),
                                Stroke::new(stroke_width, color),
                            ));
                        }
                    }
                }
            }
            if current.len() >= 2 {
                painter.add(Shape::line(current, Stroke::new(stroke_width, color)));
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
        let color = params.color("color");
        let mut entries = Vec::new();
        for (key, label) in [("upper", "KC↑"), ("middle", "KC"), ("lower", "KC↓")] {
            if let Some(series) = computed.series.get(key) {
                if series.is_empty() {
                    continue;
                }
                let idx = cursor_idx.unwrap_or(series.len() - 1).min(series.len() - 1);
                if let Some(Some(v)) = series.get(idx) {
                    entries.push(LegendEntry {
                        label: format!("{} {:.2}", label, v),
                        color,
                    });
                }
            }
        }
        entries
    }
}
