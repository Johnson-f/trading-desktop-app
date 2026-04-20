use egui::{Painter, Pos2, Rect, Shape, Stroke};

use zaned_chart_core::{
    CandleData, ComputedSeries, InputSpec, LegendEntry, ParamField, ParamKind, ParamSchema,
    ParamValues, Rgba, compute_pivot_points,
};

use super::super::super::camera::Camera;
use super::super::super::util::egui_color;
use super::super::trait_def::{Indicator, RenderTarget};

pub const ID: &str = "pivot_points";
pub const NAME: &str = "Pivot Points";
pub const LIKES: u32 = 11987;

pub static SCHEMA: ParamSchema = ParamSchema {
    fields: &[
        ParamField {
            key: "pivot_type",
            label: "Type (0=Std, 1=Fib, 2=Woodie)",
            kind: ParamKind::Int {
                default: 0,
                min: 0,
                max: 2,
            },
        },
        ParamField {
            key: "pivot_color",
            label: "Pivot Color",
            kind: ParamKind::Color {
                default: Rgba::from_rgb(255, 235, 59),
            },
        },
        ParamField {
            key: "resistance_color",
            label: "Resistance Color",
            kind: ParamKind::Color {
                default: Rgba::from_rgb(244, 67, 54),
            },
        },
        ParamField {
            key: "support_color",
            label: "Support Color",
            kind: ParamKind::Color {
                default: Rgba::from_rgb(76, 175, 80),
            },
        },
    ],
};

pub struct PivotPoints;

pub fn factory() -> Box<dyn Indicator> {
    Box::new(PivotPoints)
}

impl Indicator for PivotPoints {
    fn id(&self) -> &'static str {
        ID
    }

    fn display_name(&self, params: &ParamValues) -> String {
        let type_name = match params.int("pivot_type") {
            1 => "Fib",
            2 => "Woodie",
            _ => "Std",
        };
        format!("PP({})", type_name)
    }

    fn target(&self) -> RenderTarget {
        RenderTarget::MainOverlay
    }

    fn inputs(&self, _params: &ParamValues) -> Vec<InputSpec> {
        vec![InputSpec::Highs, InputSpec::Lows, InputSpec::Closes]
    }

    fn compute(&self, _inputs: &[&[f32]], _params: &ParamValues) -> ComputedSeries {
        // Inline compute in draw_main (needs dates from CandleData)
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

        let highs: Vec<f32> = data.instances.iter().map(|c| c.high).collect();
        let lows: Vec<f32> = data.instances.iter().map(|c| c.low).collect();
        let closes: Vec<f32> = data.instances.iter().map(|c| c.close).collect();
        let pivot_type = params.int("pivot_type");

        let (pivot, r1, r2, r3, s1, s2, s3) =
            compute_pivot_points(&highs, &lows, &closes, &data.dates, pivot_type);

        let pivot_color = egui_color(params.color("pivot_color"));
        let resistance_color = egui_color(params.color("resistance_color"));
        let support_color = egui_color(params.color("support_color"));

        let series_list: [(&Vec<Option<f32>>, egui::Color32, f32); 7] = [
            (&pivot, pivot_color, 1.5),
            (&r1, resistance_color, 1.0),
            (&r2, resistance_color, 1.0),
            (&r3, resistance_color, 1.0),
            (&s1, support_color, 1.0),
            (&s2, support_color, 1.0),
            (&s3, support_color, 1.0),
        ];

        for (series, color, width) in &series_list {
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
                                    Stroke::new(*width, *color),
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
                                Stroke::new(*width, *color),
                            ));
                        }
                    }
                }
            }
            if current.len() >= 2 {
                painter.add(Shape::line(current, Stroke::new(*width, *color)));
            }
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
        let pivot_type = params.int("pivot_type");

        let (pivot, r1, r2, r3, s1, s2, s3) =
            compute_pivot_points(&highs, &lows, &closes, &data.dates, pivot_type);

        let pivot_color = params.color("pivot_color");
        let resistance_color = params.color("resistance_color");
        let support_color = params.color("support_color");

        let series_defs: [(&Vec<Option<f32>>, &str, Rgba); 7] = [
            (&pivot, "P", pivot_color),
            (&r1, "R1", resistance_color),
            (&r2, "R2", resistance_color),
            (&r3, "R3", resistance_color),
            (&s1, "S1", support_color),
            (&s2, "S2", support_color),
            (&s3, "S3", support_color),
        ];

        let mut entries = Vec::new();
        for (series, label, color) in &series_defs {
            if series.is_empty() {
                continue;
            }
            let idx = cursor_idx.unwrap_or(series.len() - 1).min(series.len() - 1);
            if let Some(Some(v)) = series.get(idx) {
                entries.push(LegendEntry {
                    label: format!("{} {:.2}", label, v),
                    color: *color,
                });
            }
        }
        entries
    }
}
