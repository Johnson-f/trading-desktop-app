//! Detect unfilled open gaps and paint them as shaded bands.
//!
//! A gap is a candle whose range is entirely above or below the previous
//! candle's range (`low[i] > high[i-1]` or `high[i] < low[i-1]`). The band
//! spans from the gap bar to the first later bar whose range touches the
//! gap's price band; if none, it extends to the last bar.

use egui::{Align2, Color32, FontId, Pos2, Rect};

use super::camera::Camera;
use super::candle::CandleData;

const FILL_COLOR: Color32 = Color32::from_rgba_premultiplied(28, 28, 34, 170);
const LABEL_COLOR: Color32 = Color32::from_rgb(150, 150, 160);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gap {
    pub start_idx: usize,
    pub end_idx: usize,
    pub price_low: f32,
    pub price_high: f32,
}

pub fn detect(data: &CandleData) -> Vec<Gap> {
    let n = data.instances.len();
    if n < 2 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for i in 1..n {
        let prev = &data.instances[i - 1];
        let cur = &data.instances[i];
        let (price_low, price_high) = if cur.low > prev.high {
            (prev.high, cur.low) // gap up
        } else if cur.high < prev.low {
            (cur.high, prev.low) // gap down
        } else {
            continue;
        };

        // Skip gaps that have already been filled — any later bar whose range
        // intersects the band counts as a fill.
        let filled = ((i + 1)..n).any(|j| {
            let c = &data.instances[j];
            c.low <= price_high && c.high >= price_low
        });
        if filled {
            continue;
        }

        out.push(Gap {
            start_idx: i,
            end_idx: n - 1,
            price_low,
            price_high,
        });
    }
    out
}

pub fn paint(ui: &egui::Ui, chart_rect: Rect, camera: &Camera, data: &CandleData) {
    let gaps = detect(data);
    if gaps.is_empty() {
        return;
    }
    let painter = ui.painter_at(chart_rect);
    let font = FontId::monospace(9.0);

    for gap in gaps {
        let x_left = chart_rect.left()
            + ((gap.start_idx as f64 - 0.5 - camera.x_offset) * camera.x_scale) as f32;
        let x_right = chart_rect.left()
            + ((gap.end_idx as f64 + 0.5 - camera.x_offset) * camera.x_scale) as f32;
        let y_top = chart_rect.bottom()
            - ((gap.price_high as f64 - camera.y_offset) * camera.y_scale) as f32;
        let y_bottom = chart_rect.bottom()
            - ((gap.price_low as f64 - camera.y_offset) * camera.y_scale) as f32;

        let rect = Rect::from_min_max(
            Pos2::new(x_left.max(chart_rect.left()), y_top),
            Pos2::new(x_right.min(chart_rect.right()), y_bottom),
        );
        if rect.width() <= 0.0 || rect.height() <= 0.0 {
            continue;
        }
        if !rect.intersects(chart_rect) {
            continue;
        }

        painter.rect_filled(rect, 0.0, FILL_COLOR);

        // Label sits just outside the band on the left, vertically centered.
        let label = format!("{:.2}-{:.2}", gap.price_low, gap.price_high);
        let center_y = (rect.top() + rect.bottom()) * 0.5;
        painter.text(
            Pos2::new(rect.left() - 4.0, center_y),
            Align2::RIGHT_CENTER,
            label,
            font.clone(),
            LABEL_COLOR,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chart_core::CandleInstance;

    fn mk(n: usize, highs_lows: &[(f32, f32)]) -> CandleData {
        let instances: Vec<CandleInstance> = (0..n)
            .map(|i| {
                let (h, l) = highs_lows[i];
                CandleInstance {
                    index: i as f32,
                    open: l,
                    high: h,
                    low: l,
                    close: h,
                    volume: 100.0,
                }
            })
            .collect();
        CandleData {
            instances,
            dates: vec![String::new(); n],
        }
    }

    #[test]
    fn no_gap_when_ranges_overlap() {
        let d = mk(3, &[(10.0, 5.0), (11.0, 6.0), (12.0, 7.0)]);
        assert!(detect(&d).is_empty());
    }

    #[test]
    fn detects_gap_up() {
        // prev high = 5, current low = 7 → gap up (5..7).
        let d = mk(3, &[(5.0, 4.0), (8.0, 7.0), (9.0, 8.0)]);
        let gaps = detect(&d);
        assert_eq!(gaps.len(), 1);
        assert_eq!(gaps[0].start_idx, 1);
        assert!((gaps[0].price_low - 5.0).abs() < 1e-3);
        assert!((gaps[0].price_high - 7.0).abs() < 1e-3);
    }

    #[test]
    fn detects_gap_down() {
        // prev low = 10, current high = 8 → gap down (8..10).
        let d = mk(3, &[(12.0, 10.0), (8.0, 7.0), (7.5, 7.0)]);
        let gaps = detect(&d);
        assert_eq!(gaps.len(), 1);
        assert_eq!(gaps[0].start_idx, 1);
        assert!((gaps[0].price_low - 8.0).abs() < 1e-3);
        assert!((gaps[0].price_high - 10.0).abs() < 1e-3);
    }

    #[test]
    fn filled_gap_is_dropped() {
        // Gap up at i=1 spans 5..7. i=3 dips back to 6 — that's a fill, so
        // the gap should not be reported at all.
        let d = mk(4, &[(5.0, 4.0), (8.0, 7.0), (9.0, 8.0), (10.0, 6.0)]);
        assert!(detect(&d).is_empty());
    }

    #[test]
    fn unfilled_gap_extends_to_last_bar() {
        let d = mk(3, &[(5.0, 4.0), (8.0, 7.0), (9.0, 8.0)]);
        let gaps = detect(&d);
        assert_eq!(gaps[0].end_idx, 2);
    }
}
