use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke, Vec2};

use chart_core::{
    CandleData, ComputedSeries, InputSpec, LegendEntry, ParamField, ParamKind, ParamSchema,
    ParamValues, Rgba, compute_vma,
};

use super::super::super::util::{egui_color, format_volume};
use super::super::trait_def::{Indicator, RenderTarget};

pub const ID: &str = "volume";
pub const NAME: &str = "VOL";
pub const LIKES: u32 = 12293;

const PANE_BG: Color32 = Color32::from_rgb(0, 0, 0);
const LABEL_COLOR: Color32 = Color32::from_rgb(100, 100, 110);

pub static SCHEMA: ParamSchema = ParamSchema {
    fields: &[
        ParamField {
            key: "period",
            label: "Period",
            kind: ParamKind::Int {
                default: 50,
                min: 1,
                max: 500,
            },
        },
        ParamField {
            key: "up_color",
            label: "Up color",
            kind: ParamKind::Color {
                default: Rgba::from_rgba_premultiplied(78, 205, 196, 128),
            },
        },
        ParamField {
            key: "down_color",
            label: "Down color",
            kind: ParamKind::Color {
                default: Rgba::from_rgba_premultiplied(255, 107, 107, 128),
            },
        },
        ParamField {
            key: "vma_color",
            label: "VMA color",
            kind: ParamKind::Color {
                default: Rgba::from_rgb(88, 166, 255),
            },
        },
    ],
};

pub struct Volume;

pub fn factory() -> Box<dyn Indicator> {
    Box::new(Volume)
}

impl Indicator for Volume {
    fn id(&self) -> &'static str {
        ID
    }
    fn display_name(&self, params: &ParamValues) -> String {
        format!("VOL({})", params.int("period"))
    }
    fn target(&self) -> RenderTarget {
        RenderTarget::SubPane
    }

    fn inputs(&self, _params: &ParamValues) -> Vec<InputSpec> {
        vec![InputSpec::Volumes]
    }

    fn compute(&self, inputs: &[&[f32]], params: &ParamValues) -> ComputedSeries {
        let volumes = inputs.first().copied().unwrap_or(&[]);
        let period = params.int("period").max(1) as usize;
        let vma = compute_vma(volumes, period);
        let mut out = ComputedSeries::default();
        out.series.insert("vma", vma);
        out
    }

    fn draw_pane(
        &self,
        painter: &Painter,
        pane_rect: Rect,
        x_mapper: &dyn Fn(f32) -> f32,
        data: &CandleData,
        computed: &ComputedSeries,
        params: &ParamValues,
    ) {
        if data.len() == 0 {
            return;
        }

        painter.rect_filled(pane_rect, 0.0, PANE_BG);

        let up = egui_color(params.color("up_color"));
        let down = egui_color(params.color("down_color"));
        let vma_color = egui_color(params.color("vma_color"));

        let mut max_vol: f32 = 0.0;
        for (i, c) in data.instances.iter().enumerate() {
            let x = x_mapper(i as f32);
            if x < pane_rect.left() - 50.0 || x > pane_rect.right() + 50.0 {
                continue;
            }
            if c.volume > max_vol {
                max_vol = c.volume;
            }
        }
        if max_vol <= 0.0 {
            return;
        }

        let vol_height = pane_rect.height();
        let bar_width = if data.len() >= 2 {
            ((x_mapper(1.0) - x_mapper(0.0)).abs() * 0.6).max(1.0)
        } else {
            1.0
        };
        let y_of = |v: f32| pane_rect.bottom() - (v / max_vol) * vol_height * 0.9;

        for (i, c) in data.instances.iter().enumerate() {
            let x = x_mapper(i as f32);
            if x < pane_rect.left() - 50.0 || x > pane_rect.right() + 50.0 {
                continue;
            }
            let bar_top = y_of(c.volume);
            let bar_height = pane_rect.bottom() - bar_top;
            let bar_left = x - bar_width / 2.0;
            let bar_rect = Rect::from_min_size(
                Pos2::new(bar_left, bar_top),
                Vec2::new(bar_width, bar_height),
            );
            if bar_rect.right() < pane_rect.left() || bar_rect.left() > pane_rect.right() {
                continue;
            }
            let color = if c.close >= c.open { up } else { down };
            painter.rect_filled(bar_rect, 0.0, color);
        }

        if let Some(vma) = computed.series.get("vma") {
            let mut current: Vec<Pos2> = Vec::with_capacity(vma.len());
            for (i, v) in vma.iter().enumerate() {
                match v {
                    Some(val) => {
                        let x = x_mapper(i as f32);
                        if x < pane_rect.left() - 50.0 || x > pane_rect.right() + 50.0 {
                            if current.len() >= 2 {
                                painter.add(Shape::line(
                                    std::mem::take(&mut current),
                                    Stroke::new(1.5, vma_color),
                                ));
                            } else {
                                current.clear();
                            }
                            continue;
                        }
                        current.push(Pos2::new(x, y_of(*val)));
                    }
                    None => {
                        if current.len() >= 2 {
                            painter.add(Shape::line(
                                std::mem::take(&mut current),
                                Stroke::new(1.5, vma_color),
                            ));
                        } else {
                            current.clear();
                        }
                    }
                }
            }
            if current.len() >= 2 {
                painter.add(Shape::line(current, Stroke::new(1.5, vma_color)));
            }
        }

        let label_font = FontId::monospace(9.0);
        let label_x = pane_rect.right() - 50.0;
        painter.text(
            Pos2::new(label_x, pane_rect.top() + 10.0),
            Align2::LEFT_CENTER,
            format_volume(max_vol),
            label_font.clone(),
            LABEL_COLOR,
        );
        painter.text(
            Pos2::new(label_x, pane_rect.center().y),
            Align2::LEFT_CENTER,
            format_volume(max_vol / 2.0),
            label_font,
            LABEL_COLOR,
        );
    }

    fn legend(
        &self,
        data: &CandleData,
        computed: &ComputedSeries,
        params: &ParamValues,
        cursor_idx: Option<usize>,
    ) -> Vec<LegendEntry> {
        let n = data.len();
        if n == 0 {
            return Vec::new();
        }
        let idx = cursor_idx.unwrap_or(n - 1).min(n - 1);
        let mut out = Vec::with_capacity(2);
        let candle = &data.instances[idx];
        let vol_color = if candle.close >= candle.open {
            params.color("up_color")
        } else {
            params.color("down_color")
        };
        out.push(LegendEntry {
            label: format!("VOL {}", format_volume(candle.volume)),
            color: vol_color,
        });
        if let Some(vma_series) = computed.series.get("vma") {
            if let Some(Some(vma_val)) = vma_series.get(idx) {
                out.push(LegendEntry {
                    label: format!("VMA {}", format_volume(*vma_val)),
                    color: params.color("vma_color"),
                });
            }
        }
        out
    }
}
