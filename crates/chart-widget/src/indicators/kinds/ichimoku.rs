use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke};

use chart_core::{
    CandleData, ComputedSeries, InputSpec, LegendEntry, ParamField, ParamKind, ParamSchema,
    ParamValues, Rgba, compute_ichimoku,
};

use super::super::super::camera::Camera;
use super::super::super::util::egui_color;
use super::super::trait_def::{Indicator, RenderTarget};

pub const ID: &str = "ichimoku";
pub const NAME: &str = "Ichimoku Cloud";
pub const LIKES: u32 = 21345;

pub static SCHEMA: ParamSchema = ParamSchema {
    fields: &[
        ParamField {
            key: "tenkan",
            label: "Tenkan Period",
            kind: ParamKind::Int {
                default: 9,
                min: 1,
                max: 100,
            },
        },
        ParamField {
            key: "kijun",
            label: "Kijun Period",
            kind: ParamKind::Int {
                default: 26,
                min: 1,
                max: 100,
            },
        },
        ParamField {
            key: "senkou_b",
            label: "Senkou B Period",
            kind: ParamKind::Int {
                default: 52,
                min: 1,
                max: 200,
            },
        },
        ParamField {
            key: "tenkan_color",
            label: "Tenkan Color",
            kind: ParamKind::Color {
                default: Rgba::from_rgb(33, 150, 243),
            },
        },
        ParamField {
            key: "kijun_color",
            label: "Kijun Color",
            kind: ParamKind::Color {
                default: Rgba::from_rgb(244, 67, 54),
            },
        },
        ParamField {
            key: "cloud_up_color",
            label: "Cloud Up Color",
            kind: ParamKind::Color {
                default: Rgba::from_rgb(76, 175, 80),
            },
        },
        ParamField {
            key: "cloud_down_color",
            label: "Cloud Down Color",
            kind: ParamKind::Color {
                default: Rgba::from_rgb(244, 67, 54),
            },
        },
        ParamField {
            key: "chikou_color",
            label: "Chikou Color",
            kind: ParamKind::Color {
                default: Rgba::from_rgb(156, 39, 176),
            },
        },
    ],
};

pub struct Ichimoku;

pub fn factory() -> Box<dyn Indicator> {
    Box::new(Ichimoku)
}

impl Indicator for Ichimoku {
    fn id(&self) -> &'static str {
        ID
    }

    fn display_name(&self, params: &ParamValues) -> String {
        format!(
            "Ichimoku({},{},{})",
            params.int("tenkan"),
            params.int("kijun"),
            params.int("senkou_b")
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
        let tenkan_period = params.int("tenkan").max(1) as usize;
        let kijun_period = params.int("kijun").max(1) as usize;
        let senkou_b_period = params.int("senkou_b").max(1) as usize;
        let (tenkan, kijun, senkou_a, senkou_b, chikou) = compute_ichimoku(
            highs,
            lows,
            closes,
            tenkan_period,
            kijun_period,
            senkou_b_period,
        );
        let mut out = ComputedSeries::default();
        out.series.insert("tenkan", tenkan);
        out.series.insert("kijun", kijun);
        out.series.insert("senkou_a", senkou_a);
        out.series.insert("senkou_b", senkou_b);
        out.series.insert("chikou", chikou);
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
        let tenkan_color = egui_color(params.color("tenkan_color"));
        let kijun_color = egui_color(params.color("kijun_color"));
        let chikou_color = egui_color(params.color("chikou_color"));
        let cloud_up_color = egui_color(params.color("cloud_up_color"));
        let cloud_down_color = egui_color(params.color("cloud_down_color"));

        let cloud_up_fill = Color32::from_rgba_premultiplied(
            cloud_up_color.r(),
            cloud_up_color.g(),
            cloud_up_color.b(),
            30,
        );
        let cloud_down_fill = Color32::from_rgba_premultiplied(
            cloud_down_color.r(),
            cloud_down_color.g(),
            cloud_down_color.b(),
            30,
        );

        let to_screen = |i: usize, y: f32| -> Pos2 {
            let x_pixel = ((i as f64 - camera.x_offset) * camera.x_scale) as f32;
            let y_pixel = (y as f64 - camera.y_offset) * camera.y_scale;
            Pos2::new(rect.left() + x_pixel, rect.bottom() - y_pixel as f32)
        };

        // Draw cloud fill between senkou_a and senkou_b
        // senkou series are longer than candle data (displaced forward)
        if let (Some(sa), Some(sb)) = (
            computed.series.get("senkou_a"),
            computed.series.get("senkou_b"),
        ) {
            let len = sa.len().min(sb.len());
            let mut i = 0;
            while i < len {
                // Find a contiguous visible segment where both are Some
                let mut upper_pts: Vec<Pos2> = Vec::new();
                let mut lower_pts: Vec<Pos2> = Vec::new();
                let mut seg_up_dominates = true;

                while i < len {
                    if let (Some(av), Some(bv)) = (sa[i], sb[i]) {
                        let x_pixel = ((i as f64 - camera.x_offset) * camera.x_scale) as f32;
                        let x = rect.left() + x_pixel;
                        if x >= rect.left() - 50.0 && x <= rect.right() + 50.0 {
                            let pt_a = to_screen(i, av);
                            let pt_b = to_screen(i, bv);

                            // Check if direction changed (a above b or b above a)
                            let a_above = av >= bv;
                            if upper_pts.is_empty() {
                                seg_up_dominates = a_above;
                            } else if a_above != seg_up_dominates {
                                // Flush current segment
                                if upper_pts.len() >= 2 {
                                    let fill = if seg_up_dominates {
                                        cloud_up_fill
                                    } else {
                                        cloud_down_fill
                                    };
                                    let mut polygon = upper_pts.clone();
                                    polygon.extend(lower_pts.iter().rev());
                                    painter.add(Shape::convex_polygon(polygon, fill, Stroke::NONE));
                                }
                                upper_pts.clear();
                                lower_pts.clear();
                                seg_up_dominates = a_above;
                            }

                            if a_above {
                                upper_pts.push(pt_a);
                                lower_pts.push(pt_b);
                            } else {
                                upper_pts.push(pt_b);
                                lower_pts.push(pt_a);
                            }
                        } else if !upper_pts.is_empty() {
                            break;
                        }
                    } else if !upper_pts.is_empty() {
                        break;
                    }
                    i += 1;
                }

                if upper_pts.len() >= 2 {
                    let fill = if seg_up_dominates {
                        cloud_up_fill
                    } else {
                        cloud_down_fill
                    };
                    let mut polygon = upper_pts.clone();
                    polygon.extend(lower_pts.iter().rev());
                    painter.add(Shape::convex_polygon(polygon, fill, Stroke::NONE));
                }
            }
        }

        // Draw tenkan line (blue)
        draw_line(painter, rect, camera, computed, "tenkan", tenkan_color, 1.5);
        // Draw kijun line (red)
        draw_line(painter, rect, camera, computed, "kijun", kijun_color, 1.5);
        // Draw chikou line (purple)
        draw_line(painter, rect, camera, computed, "chikou", chikou_color, 1.0);
        // Draw senkou_a as thin line
        draw_line(
            painter,
            rect,
            camera,
            computed,
            "senkou_a",
            cloud_up_color,
            1.0,
        );
        // Draw senkou_b as thin line
        draw_line(
            painter,
            rect,
            camera,
            computed,
            "senkou_b",
            cloud_down_color,
            1.0,
        );
    }

    fn legend(
        &self,
        _data: &CandleData,
        computed: &ComputedSeries,
        params: &ParamValues,
        cursor_idx: Option<usize>,
    ) -> Vec<LegendEntry> {
        let mut entries = Vec::new();

        let series_defs = [
            ("tenkan", "Tenkan", "tenkan_color"),
            ("kijun", "Kijun", "kijun_color"),
            ("senkou_a", "Senkou A", "cloud_up_color"),
            ("senkou_b", "Senkou B", "cloud_down_color"),
            ("chikou", "Chikou", "chikou_color"),
        ];

        for (key, label, color_key) in series_defs {
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

fn draw_line(
    painter: &Painter,
    rect: Rect,
    camera: &Camera,
    computed: &ComputedSeries,
    key: &str,
    color: Color32,
    width: f32,
) {
    let Some(series) = computed.series.get(key) else {
        return;
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
                            Stroke::new(width, color),
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
                        Stroke::new(width, color),
                    ));
                }
            }
        }
    }
    if current.len() >= 2 {
        painter.add(Shape::line(current, Stroke::new(width, color)));
    }
}
