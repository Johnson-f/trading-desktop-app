# Indicator Expansion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add 22 new technical analysis indicators to the charting engine, organized into 7 implementation phases from simplest to most complex.

**Architecture:** Each indicator follows an established pattern: a stateless struct in `apps/desktop/.../indicators/kinds/` implementing the `Indicator` trait, backed by a pure compute function in `crates/chart-core/src/indicators/`. Indicators are registered in the static `DEFS` array in `registry.rs`. No structural changes to the trait, manager, or rendering pipeline are needed — the existing `InputSpec` variants, `ParamKind::Float`, and `ComputedSeries` HashMap already support multi-input, float params, and multi-series output.

**Tech Stack:** Rust, egui (rendering), `ta` crate v0.5.0 (technical analysis primitives), `zaned-chart-core` crate (pure compute functions)

**Spec:** `docs/superpowers/specs/2026-04-20-indicator-expansion-design.md`

---

## Key Reference Files

These files define the patterns every task follows. Read them before starting any task:

| File | Purpose |
|------|---------|
| `crates/chart-core/src/indicators/ema.rs` | Template: `ta` crate wrapper compute function with tests |
| `crates/chart-core/src/indicators/mod.rs` | Export hub for all compute functions |
| `crates/chart-core/src/lib.rs` | Re-exports compute functions to crate consumers |
| `crates/chart-core/src/series.rs` | `InputSpec`, `ComputedSeries`, `LegendEntry` types |
| `crates/chart-core/src/params.rs` | `ParamKind`, `ParamField`, `ParamSchema`, `ParamValues` types |
| `apps/desktop/src/ui_components/widgets/charts/indicators/kinds/ema.rs` | Template: MainOverlay indicator (single line) |
| `apps/desktop/src/ui_components/widgets/charts/indicators/kinds/rsi.rs` | Template: SubPane indicator (single line + guide lines) |
| `apps/desktop/src/ui_components/widgets/charts/indicators/kinds/volume.rs` | Template: SubPane with bars + line overlay |
| `apps/desktop/src/ui_components/widgets/charts/indicators/kinds/mod.rs` | Module declarations for all indicator kinds |
| `apps/desktop/src/ui_components/widgets/charts/indicators/registry.rs` | Static `DEFS` array registering all indicators |

---

## Task 0: Shared Utilities in chart-core

**Files:**
- Create: `crates/chart-core/src/indicators/util.rs`
- Modify: `crates/chart-core/src/indicators/mod.rs`

Several indicators need shared helpers: `ta::DataItem` construction, rolling window min/max, WMA, and session boundary detection. Build these first.

- [ ] **Step 1: Write tests for `build_data_items`**

```rust
// crates/chart-core/src/indicators/util.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_data_items_zips_hlc() {
        let h = [10.0f32, 20.0];
        let l = [5.0, 15.0];
        let c = [8.0, 18.0];
        let items = build_data_items(&h, &l, &c, None);
        assert_eq!(items.len(), 2);
        assert!((items[0].high() - 10.0).abs() < 1e-5);
        assert!((items[0].low() - 5.0).abs() < 1e-5);
        assert!((items[0].close() - 8.0).abs() < 1e-5);
        assert!((items[0].volume() - 0.0).abs() < 1e-5);
    }

    #[test]
    fn build_data_items_with_volume() {
        let h = [10.0f32];
        let l = [5.0];
        let c = [8.0];
        let v = [1000.0f32];
        let items = build_data_items(&h, &l, &c, Some(&v));
        assert!((items[0].volume() - 1000.0).abs() < 1e-5);
    }

    #[test]
    fn build_data_items_mismatched_lengths_uses_shortest() {
        let h = [10.0f32, 20.0, 30.0];
        let l = [5.0, 15.0];
        let c = [8.0, 18.0];
        let items = build_data_items(&h, &l, &c, None);
        assert_eq!(items.len(), 2);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd /Users/user/Zaned && cargo test -p zaned-chart-core util::tests --no-default-features 2>&1 | tail -5`
Expected: compilation errors — `build_data_items` not defined yet.

- [ ] **Step 3: Implement `build_data_items`**

```rust
// crates/chart-core/src/indicators/util.rs
use ta::DataItem;

/// Build `ta::DataItem` structs from parallel OHLCV slices. Uses the shortest
/// slice length. If `volumes` is `None`, volume defaults to 0.0.
pub fn build_data_items(
    highs: &[f32],
    lows: &[f32],
    closes: &[f32],
    volumes: Option<&[f32]>,
) -> Vec<DataItem> {
    let n = highs.len().min(lows.len()).min(closes.len());
    let n = volumes.map_or(n, |v| n.min(v.len()));
    (0..n)
        .map(|i| {
            let vol = volumes.map_or(0.0, |v| v[i] as f64);
            // DataItem requires open — we use close as a reasonable stand-in
            // since the ta indicators that need DataItem don't use open.
            DataItem::builder()
                .open(closes[i] as f64)
                .high(highs[i] as f64)
                .low(lows[i] as f64)
                .close(closes[i] as f64)
                .volume(vol)
                .build()
                .unwrap()
        })
        .collect()
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd /Users/user/Zaned && cargo test -p zaned-chart-core util::tests --no-default-features 2>&1 | tail -10`
Expected: 3 tests pass.

- [ ] **Step 5: Write tests for `rolling_high` and `rolling_low`**

Add to the tests module in `util.rs`:

```rust
    #[test]
    fn rolling_high_basic() {
        let data = [1.0f32, 3.0, 2.0, 5.0, 4.0];
        let rh = rolling_high(&data, 3);
        assert_eq!(rh.len(), 5);
        assert_eq!(rh[0], None);    // not enough data
        assert_eq!(rh[1], None);    // not enough data
        assert!((rh[2].unwrap() - 3.0).abs() < 1e-5);  // max(1,3,2)
        assert!((rh[3].unwrap() - 5.0).abs() < 1e-5);  // max(3,2,5)
        assert!((rh[4].unwrap() - 5.0).abs() < 1e-5);  // max(2,5,4)
    }

    #[test]
    fn rolling_low_basic() {
        let data = [5.0f32, 3.0, 4.0, 1.0, 2.0];
        let rl = rolling_low(&data, 3);
        assert_eq!(rl.len(), 5);
        assert_eq!(rl[0], None);
        assert_eq!(rl[1], None);
        assert!((rl[2].unwrap() - 3.0).abs() < 1e-5);  // min(5,3,4)
        assert!((rl[3].unwrap() - 1.0).abs() < 1e-5);  // min(3,4,1)
        assert!((rl[4].unwrap() - 1.0).abs() < 1e-5);  // min(4,1,2)
    }

    #[test]
    fn rolling_period_larger_than_data_all_none() {
        let data = [1.0f32, 2.0];
        assert!(rolling_high(&data, 5).iter().all(|v| v.is_none()));
        assert!(rolling_low(&data, 5).iter().all(|v| v.is_none()));
    }
```

- [ ] **Step 6: Run tests to verify they fail**

Run: `cd /Users/user/Zaned && cargo test -p zaned-chart-core util::tests --no-default-features 2>&1 | tail -5`
Expected: compilation errors.

- [ ] **Step 7: Implement `rolling_high` and `rolling_low`**

```rust
/// Highest value in a trailing window of `period` bars. Returns `None` for
/// bars where fewer than `period` values are available.
pub fn rolling_high(data: &[f32], period: usize) -> Vec<Option<f32>> {
    data.iter()
        .enumerate()
        .map(|(i, _)| {
            if i + 1 < period {
                None
            } else {
                let start = i + 1 - period;
                data[start..=i]
                    .iter()
                    .copied()
                    .reduce(f32::max)
            }
        })
        .collect()
}

/// Lowest value in a trailing window of `period` bars. Returns `None` for
/// bars where fewer than `period` values are available.
pub fn rolling_low(data: &[f32], period: usize) -> Vec<Option<f32>> {
    data.iter()
        .enumerate()
        .map(|(i, _)| {
            if i + 1 < period {
                None
            } else {
                let start = i + 1 - period;
                data[start..=i]
                    .iter()
                    .copied()
                    .reduce(f32::min)
            }
        })
        .collect()
}
```

- [ ] **Step 8: Run tests to verify they pass**

Run: `cd /Users/user/Zaned && cargo test -p zaned-chart-core util::tests --no-default-features 2>&1 | tail -10`
Expected: 6 tests pass.

- [ ] **Step 9: Write tests for `compute_wma`**

```rust
    #[test]
    fn wma_basic() {
        // WMA(3) of [1,2,3,4]: weights [1,2,3], divisor=6
        // bar 2: (1*1 + 2*2 + 3*3) / 6 = 14/6 ≈ 2.333
        // bar 3: (2*1 + 3*2 + 4*3) / 6 = 20/6 ≈ 3.333
        let data = [1.0f32, 2.0, 3.0, 4.0];
        let wma = compute_wma(&data, 3);
        assert_eq!(wma.len(), 4);
        assert_eq!(wma[0], None);
        assert_eq!(wma[1], None);
        assert!((wma[2].unwrap() - 14.0 / 6.0).abs() < 1e-4);
        assert!((wma[3].unwrap() - 20.0 / 6.0).abs() < 1e-4);
    }

    #[test]
    fn wma_period_one_is_identity() {
        let data = [5.0f32, 10.0, 15.0];
        let wma = compute_wma(&data, 1);
        assert!((wma[0].unwrap() - 5.0).abs() < 1e-5);
        assert!((wma[1].unwrap() - 10.0).abs() < 1e-5);
        assert!((wma[2].unwrap() - 15.0).abs() < 1e-5);
    }

    #[test]
    fn wma_empty_returns_empty() {
        assert!(compute_wma(&[], 3).is_empty());
    }

    #[test]
    fn wma_period_zero_returns_all_none() {
        let data = [1.0f32, 2.0];
        assert!(compute_wma(&data, 0).iter().all(|v| v.is_none()));
    }
```

- [ ] **Step 10: Run tests to verify they fail, then implement `compute_wma`**

```rust
/// Weighted Moving Average. Weight of bar `i` in the window is `i + 1`
/// (most recent bar gets the highest weight). Returns `None` for bars
/// where fewer than `period` values are available.
pub fn compute_wma(data: &[f32], period: usize) -> Vec<Option<f32>> {
    if period == 0 {
        return vec![None; data.len()];
    }
    let divisor: f32 = (period * (period + 1)) as f32 / 2.0;
    data.iter()
        .enumerate()
        .map(|(i, _)| {
            if i + 1 < period {
                None
            } else {
                let start = i + 1 - period;
                let sum: f32 = data[start..=i]
                    .iter()
                    .enumerate()
                    .map(|(w, v)| (w + 1) as f32 * v)
                    .sum();
                Some(sum / divisor)
            }
        })
        .collect()
}
```

- [ ] **Step 11: Run tests to verify all pass**

Run: `cd /Users/user/Zaned && cargo test -p zaned-chart-core util::tests --no-default-features 2>&1 | tail -10`
Expected: 10 tests pass.

- [ ] **Step 12: Write tests for `session_boundaries`**

```rust
    #[test]
    fn session_boundaries_finds_day_changes() {
        let dates = vec![
            "2026-01-05".to_string(),
            "2026-01-05".to_string(),
            "2026-01-06".to_string(),
            "2026-01-06".to_string(),
            "2026-01-07".to_string(),
        ];
        let bounds = session_boundaries(&dates);
        // Each boundary is the index of the first bar of a new session.
        assert_eq!(bounds, vec![0, 2, 4]);
    }

    #[test]
    fn session_boundaries_daily_data_every_bar_is_session() {
        let dates = vec![
            "2026-01-05".to_string(),
            "2026-01-06".to_string(),
            "2026-01-07".to_string(),
        ];
        let bounds = session_boundaries(&dates);
        assert_eq!(bounds, vec![0, 1, 2]);
    }

    #[test]
    fn session_boundaries_empty() {
        let bounds = session_boundaries(&[]);
        assert!(bounds.is_empty());
    }
```

- [ ] **Step 13: Implement `session_boundaries` and run tests**

```rust
/// Find indices where the date portion (first 10 chars, YYYY-MM-DD) changes.
/// Each returned index is the first bar of a new session. Used by VWAP and
/// Pivot Points for per-session resets.
pub fn session_boundaries(dates: &[String]) -> Vec<usize> {
    if dates.is_empty() {
        return Vec::new();
    }
    let mut boundaries = vec![0usize];
    for i in 1..dates.len() {
        let prev = &dates[i - 1].get(..10).unwrap_or(&dates[i - 1]);
        let curr = &dates[i].get(..10).unwrap_or(&dates[i]);
        if prev != curr {
            boundaries.push(i);
        }
    }
    boundaries
}
```

Run: `cd /Users/user/Zaned && cargo test -p zaned-chart-core util::tests --no-default-features 2>&1 | tail -10`
Expected: 13 tests pass.

- [ ] **Step 14: Wire up `util` module in mod.rs**

Add to `crates/chart-core/src/indicators/mod.rs`:

```rust
pub mod util;
```

Do NOT add re-exports to `lib.rs` — the util functions are used internally by other compute functions, not by the desktop app directly.

- [ ] **Step 15: Run full chart-core tests and commit**

Run: `cd /Users/user/Zaned && cargo test -p zaned-chart-core 2>&1 | tail -10`
Expected: all tests pass.

```bash
git add crates/chart-core/src/indicators/util.rs crates/chart-core/src/indicators/mod.rs
git commit -m "feat(chart-core): add shared indicator utilities (DataItem builder, rolling min/max, WMA, session boundaries)"
```

---

## Task 1: SMA (Simple Moving Average)

**Files:**
- Modify: `crates/chart-core/src/indicators/sma.rs` (existing empty stub)
- Modify: `crates/chart-core/src/indicators/mod.rs`
- Modify: `crates/chart-core/src/lib.rs`
- Create: `apps/desktop/src/ui_components/widgets/charts/indicators/kinds/sma.rs`
- Modify: `apps/desktop/src/ui_components/widgets/charts/indicators/kinds/mod.rs`
- Modify: `apps/desktop/src/ui_components/widgets/charts/indicators/registry.rs`

- [ ] **Step 1: Write compute function tests**

```rust
// crates/chart-core/src/indicators/sma.rs
use ta::Next;
use ta::indicators::SimpleMovingAverage;

pub fn compute_sma(closes: &[f32], period: usize) -> Vec<Option<f32>> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sma_rolling_mean() {
        let closes = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let sma = compute_sma(&closes, 3);
        assert_eq!(sma.len(), 5);
        // ta::SMA returns growing-window mean until filled
        assert!((sma[0].unwrap() - 1.0).abs() < 1e-5);   // mean(1)
        assert!((sma[1].unwrap() - 1.5).abs() < 1e-5);   // mean(1,2)
        assert!((sma[2].unwrap() - 2.0).abs() < 1e-5);   // mean(1,2,3)
        assert!((sma[3].unwrap() - 3.0).abs() < 1e-5);   // mean(2,3,4)
        assert!((sma[4].unwrap() - 4.0).abs() < 1e-5);   // mean(3,4,5)
    }

    #[test]
    fn sma_empty_returns_empty() {
        assert!(compute_sma(&[], 5).is_empty());
    }

    #[test]
    fn sma_period_zero_returns_all_none() {
        let closes = vec![1.0, 2.0];
        assert!(compute_sma(&closes, 0).iter().all(|v| v.is_none()));
    }

    #[test]
    fn sma_every_bar_has_value() {
        let closes = vec![1.0, 2.0, 3.0];
        let sma = compute_sma(&closes, 10);
        assert!(sma.iter().all(|v| v.is_some()));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd /Users/user/Zaned && cargo test -p zaned-chart-core sma::tests --no-default-features 2>&1 | tail -5`
Expected: FAIL — `todo!()` panics.

- [ ] **Step 3: Implement compute function**

Replace `todo!()` in `crates/chart-core/src/indicators/sma.rs`:

```rust
pub fn compute_sma(closes: &[f32], period: usize) -> Vec<Option<f32>> {
    let Ok(mut sma) = SimpleMovingAverage::new(period) else {
        return vec![None; closes.len()];
    };
    closes
        .iter()
        .map(|c| Some(sma.next(*c as f64) as f32))
        .collect()
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd /Users/user/Zaned && cargo test -p zaned-chart-core sma::tests --no-default-features 2>&1 | tail -10`
Expected: 4 tests pass.

- [ ] **Step 5: Export from chart-core**

Add to `crates/chart-core/src/indicators/mod.rs`:
```rust
mod sma;
pub use sma::compute_sma;
```

Add to `crates/chart-core/src/lib.rs` re-export line:
```rust
pub use indicators::{compute_ema, compute_rsi, compute_sma, compute_vma};
```

- [ ] **Step 6: Create the desktop indicator kind**

```rust
// apps/desktop/src/ui_components/widgets/charts/indicators/kinds/sma.rs
use egui::{Painter, Pos2, Rect, Shape, Stroke};

use zaned_chart_core::{
    CandleData, ComputedSeries, InputSpec, LegendEntry, ParamField, ParamKind, ParamSchema,
    ParamValues, Rgba, compute_sma,
};

use super::super::super::camera::Camera;
use super::super::super::util::egui_color;
use super::super::trait_def::{Indicator, RenderTarget};

pub const ID: &str = "sma";
pub const NAME: &str = "SMA";
pub const LIKES: u32 = 31204;

pub static SCHEMA: ParamSchema = ParamSchema {
    fields: &[
        ParamField {
            key: "period",
            label: "Period",
            kind: ParamKind::Int {
                default: 20,
                min: 1,
                max: 500,
            },
        },
        ParamField {
            key: "color",
            label: "Color",
            kind: ParamKind::Color {
                default: Rgba::from_rgb(255, 152, 0),
            },
        },
    ],
};

pub struct Sma;

pub fn factory() -> Box<dyn Indicator> {
    Box::new(Sma)
}

impl Indicator for Sma {
    fn id(&self) -> &'static str {
        ID
    }
    fn display_name(&self, params: &ParamValues) -> String {
        format!("SMA({})", params.int("period"))
    }
    fn target(&self) -> RenderTarget {
        RenderTarget::MainOverlay
    }

    fn inputs(&self, _params: &ParamValues) -> Vec<InputSpec> {
        vec![InputSpec::Closes]
    }

    fn compute(&self, inputs: &[&[f32]], params: &ParamValues) -> ComputedSeries {
        let closes = inputs.first().copied().unwrap_or(&[]);
        let period = params.int("period").max(1) as usize;
        let series = compute_sma(closes, period);
        let mut out = ComputedSeries::default();
        out.series.insert("sma", series);
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
        let Some(series) = computed.series.get("sma") else {
            return;
        };
        let color = egui_color(params.color("color"));
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
                                Stroke::new(1.5, color),
                            ));
                        }
                        continue;
                    }
                    let y_pixel = (*y as f64 - camera.y_offset) * camera.y_scale;
                    let y = rect.bottom() - y_pixel as f32;
                    current.push(Pos2::new(x, y));
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
        let Some(series) = computed.series.get("sma") else {
            return Vec::new();
        };
        if series.is_empty() {
            return Vec::new();
        }
        let idx = cursor_idx.unwrap_or(series.len() - 1).min(series.len() - 1);
        match series.get(idx).and_then(|v| *v) {
            Some(v) => vec![LegendEntry {
                label: format!("SMA{} {:.2}", params.int("period"), v),
                color: params.color("color"),
            }],
            None => Vec::new(),
        }
    }
}
```

- [ ] **Step 7: Register the indicator**

Add to `apps/desktop/src/ui_components/widgets/charts/indicators/kinds/mod.rs`:
```rust
pub mod sma;
```

Add to the `DEFS` array in `apps/desktop/src/ui_components/widgets/charts/indicators/registry.rs`:

Import at the top:
```rust
use super::kinds::{ema, rsi, sma, volume};
```

Add entry after the EMA entry in `DEFS`:
```rust
    IndicatorDef {
        id: sma::ID,
        name: sma::NAME,
        short_name: "SMA",
        target: RenderTarget::MainOverlay,
        params: sma::SCHEMA,
        likes: sma::LIKES,
        description: "Simple Moving Average. Equal-weight average of the last N closing prices. Smoother than EMA but slower to react.",
        factory: sma::factory,
    },
```

- [ ] **Step 8: Build and test**

Run: `cd /Users/user/Zaned && cargo test -p zaned-chart-core sma::tests && cargo check -p zaned-desktop 2>&1 | tail -10`
Expected: tests pass, desktop compiles.

- [ ] **Step 9: Commit**

```bash
git add crates/chart-core/src/indicators/sma.rs crates/chart-core/src/indicators/mod.rs crates/chart-core/src/lib.rs apps/desktop/src/ui_components/widgets/charts/indicators/kinds/sma.rs apps/desktop/src/ui_components/widgets/charts/indicators/kinds/mod.rs apps/desktop/src/ui_components/widgets/charts/indicators/registry.rs
git commit -m "feat(indicators): add SMA (Simple Moving Average)"
```

---

## Task 2: WMA (Weighted Moving Average)

**Files:**
- Create: `crates/chart-core/src/indicators/wma.rs`
- Modify: `crates/chart-core/src/indicators/mod.rs`
- Modify: `crates/chart-core/src/lib.rs`
- Create: `apps/desktop/src/ui_components/widgets/charts/indicators/kinds/wma.rs`
- Modify: `apps/desktop/src/ui_components/widgets/charts/indicators/kinds/mod.rs`
- Modify: `apps/desktop/src/ui_components/widgets/charts/indicators/registry.rs`

- [ ] **Step 1: Write compute function with tests**

```rust
// crates/chart-core/src/indicators/wma.rs
use super::util::compute_wma;

/// Weighted Moving Average — delegates to the shared `compute_wma` in util.
/// Re-exported from chart-core for the desktop indicator kind.
pub fn compute_wma_indicator(closes: &[f32], period: usize) -> Vec<Option<f32>> {
    compute_wma(closes, period)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wma_indicator_delegates_to_util() {
        let closes = vec![1.0f32, 2.0, 3.0, 4.0];
        let result = compute_wma_indicator(&closes, 3);
        assert_eq!(result.len(), 4);
        assert_eq!(result[0], None);
        assert_eq!(result[1], None);
        assert!((result[2].unwrap() - 14.0 / 6.0).abs() < 1e-4);
    }
}
```

- [ ] **Step 2: Run tests, then export**

Run: `cd /Users/user/Zaned && cargo test -p zaned-chart-core wma::tests --no-default-features 2>&1 | tail -5`

Add to `crates/chart-core/src/indicators/mod.rs`:
```rust
mod wma;
pub use wma::compute_wma_indicator;
```

Add to `crates/chart-core/src/lib.rs` re-export line.

- [ ] **Step 3: Create desktop indicator kind**

Same pattern as SMA but uses `compute_wma_indicator`. Key differences:
- `ID: "wma"`, `NAME: "WMA"`, `LIKES: 8432`
- Default color: `Rgba::from_rgb(233, 30, 99)` (pink)
- Series key: `"wma"`
- Description: `"Weighted Moving Average. Linearly weights recent prices more heavily, falling between SMA and EMA in responsiveness."`

Create `apps/desktop/src/ui_components/widgets/charts/indicators/kinds/wma.rs` following the exact same structure as `sma.rs` above, substituting the constants and using `compute_wma_indicator` instead of `compute_sma`.

- [ ] **Step 4: Register in kinds/mod.rs and registry.rs**

Add `pub mod wma;` to kinds/mod.rs.
Add `wma` to the import and a new `IndicatorDef` entry in `registry.rs`.

- [ ] **Step 5: Build, test, commit**

Run: `cd /Users/user/Zaned && cargo test -p zaned-chart-core wma::tests && cargo check -p zaned-desktop 2>&1 | tail -10`

```bash
git add -A && git commit -m "feat(indicators): add WMA (Weighted Moving Average)"
```

---

## Task 3: Hull Moving Average (HMA)

**Files:**
- Create: `crates/chart-core/src/indicators/hma.rs`
- Modify: `crates/chart-core/src/indicators/mod.rs`
- Modify: `crates/chart-core/src/lib.rs`
- Create: `apps/desktop/src/ui_components/widgets/charts/indicators/kinds/hma.rs`
- Modify: `apps/desktop/src/ui_components/widgets/charts/indicators/kinds/mod.rs`
- Modify: `apps/desktop/src/ui_components/widgets/charts/indicators/registry.rs`

- [ ] **Step 1: Write compute function with tests**

```rust
// crates/chart-core/src/indicators/hma.rs
use super::util::compute_wma;

/// Hull Moving Average: WMA(2 * WMA(n/2) - WMA(n), sqrt(n)).
/// Designed to eliminate lag while maintaining smoothness.
pub fn compute_hma(closes: &[f32], period: usize) -> Vec<Option<f32>> {
    if period < 2 || closes.is_empty() {
        return vec![None; closes.len()];
    }
    let half = period / 2;
    let sqrt_p = (period as f64).sqrt().round() as usize;

    let wma_half = compute_wma(closes, half.max(1));
    let wma_full = compute_wma(closes, period);

    // diff = 2 * WMA(half) - WMA(full)
    let diff: Vec<f32> = wma_half
        .iter()
        .zip(wma_full.iter())
        .map(|(h, f)| match (h, f) {
            (Some(hv), Some(fv)) => 2.0 * hv - fv,
            _ => 0.0,
        })
        .collect();

    // Track which bars have valid diff values
    let valid: Vec<bool> = wma_half
        .iter()
        .zip(wma_full.iter())
        .map(|(h, f)| h.is_some() && f.is_some())
        .collect();

    let wma_diff = compute_wma(&diff, sqrt_p.max(1));

    wma_diff
        .iter()
        .enumerate()
        .map(|(i, v)| {
            if valid[i] {
                *v
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hma_has_values_after_warmup() {
        let closes: Vec<f32> = (1..=20).map(|i| i as f32).collect();
        let hma = compute_hma(&closes, 9);
        assert_eq!(hma.len(), 20);
        // After warmup (period-1 bars minimum), values should appear
        let non_none_count = hma.iter().filter(|v| v.is_some()).count();
        assert!(non_none_count > 5, "expected values after warmup, got {non_none_count}");
    }

    #[test]
    fn hma_empty_returns_empty() {
        assert!(compute_hma(&[], 9).is_empty());
    }

    #[test]
    fn hma_period_one_returns_all_none() {
        let closes = vec![1.0f32, 2.0, 3.0];
        assert!(compute_hma(&closes, 1).iter().all(|v| v.is_none()));
    }

    #[test]
    fn hma_leads_price_on_linear_trend() {
        // HMA is designed to track trends closely. On a linear sequence
        // the HMA should be close to or slightly ahead of an SMA.
        let closes: Vec<f32> = (1..=50).map(|i| i as f32).collect();
        let hma = compute_hma(&closes, 9);
        let last = hma.last().unwrap().unwrap();
        // On linear data, HMA should be close to the last price
        assert!((last - 50.0).abs() < 5.0, "HMA on linear trend should be near price, got {last}");
    }
}
```

- [ ] **Step 2: Run tests, export, create desktop kind, register**

Follow the same pattern as Task 2:
- `ID: "hma"`, `NAME: "Hull MA"`, `LIKES: 12876`
- Default color: `Rgba::from_rgb(0, 188, 212)` (cyan)
- Description: `"Hull Moving Average. Eliminates lag using weighted moving averages of different periods. Very responsive to price changes."`

- [ ] **Step 3: Build, test, commit**

```bash
git commit -m "feat(indicators): add HMA (Hull Moving Average)"
```

---

## Task 4: ALMA (Arnaud Legoux Moving Average)

**Files:**
- Create: `crates/chart-core/src/indicators/alma.rs`
- Modify: `crates/chart-core/src/indicators/mod.rs`
- Modify: `crates/chart-core/src/lib.rs`
- Create: `apps/desktop/src/ui_components/widgets/charts/indicators/kinds/alma.rs`
- Modify: `apps/desktop/src/ui_components/widgets/charts/indicators/kinds/mod.rs`
- Modify: `apps/desktop/src/ui_components/widgets/charts/indicators/registry.rs`

- [ ] **Step 1: Write compute function with tests**

```rust
// crates/chart-core/src/indicators/alma.rs

/// Arnaud Legoux Moving Average. Gaussian-weighted MA with offset and sigma.
/// - `offset` (0.0–1.0): shifts the Gaussian peak (0.85 = near recent end)
/// - `sigma` (1.0–20.0): controls Gaussian width (6.0 = moderate smoothing)
pub fn compute_alma(closes: &[f32], period: usize, offset: f32, sigma: f32) -> Vec<Option<f32>> {
    if period < 2 || closes.is_empty() || sigma <= 0.0 {
        return vec![None; closes.len()];
    }

    let m = offset * (period as f32 - 1.0);
    let s = period as f32 / sigma;

    // Precompute weights
    let weights: Vec<f32> = (0..period)
        .map(|i| {
            let diff = i as f32 - m;
            (-diff * diff / (2.0 * s * s)).exp()
        })
        .collect();
    let weight_sum: f32 = weights.iter().sum();

    closes
        .iter()
        .enumerate()
        .map(|(i, _)| {
            if i + 1 < period {
                return None;
            }
            let start = i + 1 - period;
            let val: f32 = closes[start..=i]
                .iter()
                .zip(weights.iter())
                .map(|(c, w)| c * w)
                .sum::<f32>()
                / weight_sum;
            Some(val)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alma_basic() {
        let closes: Vec<f32> = (1..=20).map(|i| i as f32).collect();
        let alma = compute_alma(&closes, 9, 0.85, 6.0);
        assert_eq!(alma.len(), 20);
        // First 8 bars should be None (period-1)
        assert!(alma[7].is_none());
        assert!(alma[8].is_some());
    }

    #[test]
    fn alma_offset_one_weights_most_recent() {
        // With offset=1.0, the Gaussian peak is at the most recent bar
        let closes = vec![1.0f32, 1.0, 1.0, 1.0, 10.0];
        let alma = compute_alma(&closes, 5, 1.0, 6.0);
        let val = alma[4].unwrap();
        // Should be heavily weighted toward 10.0
        assert!(val > 5.0, "ALMA with offset=1.0 should weight recent, got {val}");
    }

    #[test]
    fn alma_empty_returns_empty() {
        assert!(compute_alma(&[], 9, 0.85, 6.0).is_empty());
    }

    #[test]
    fn alma_period_one_returns_all_none() {
        let closes = vec![1.0f32, 2.0];
        assert!(compute_alma(&closes, 1, 0.85, 6.0).iter().all(|v| v.is_none()));
    }
}
```

- [ ] **Step 2: Export from chart-core**

- [ ] **Step 3: Create desktop kind**

Key differences from SMA:
- `ID: "alma"`, `NAME: "ALMA"`, `LIKES: 6543`
- Params: period (Int, default 9), offset (Float 0.0–1.0, default 0.85), sigma (Float 1.0–20.0, default 6.0), color
- `compute()` calls `compute_alma(closes, period, offset, sigma)`
- Default color: `Rgba::from_rgb(156, 39, 176)` (purple)
- Description: `"Arnaud Legoux Moving Average. Gaussian-weighted filter with adjustable offset and smoothness. Reduces noise while maintaining responsiveness."`

- [ ] **Step 4: Register, build, test, commit**

```bash
git commit -m "feat(indicators): add ALMA (Arnaud Legoux Moving Average)"
```

---

## Task 5: Bollinger Bands

**Files:**
- Create: `crates/chart-core/src/indicators/bollinger.rs`
- Modify: `crates/chart-core/src/indicators/mod.rs`
- Modify: `crates/chart-core/src/lib.rs`
- Create: `apps/desktop/src/ui_components/widgets/charts/indicators/kinds/bollinger.rs`
- Modify: `apps/desktop/src/ui_components/widgets/charts/indicators/kinds/mod.rs`
- Modify: `apps/desktop/src/ui_components/widgets/charts/indicators/registry.rs`

- [ ] **Step 1: Write compute function with tests**

```rust
// crates/chart-core/src/indicators/bollinger.rs
use ta::Next;
use ta::indicators::BollingerBands as TaBollinger;

/// Returns three series: (upper, middle, lower).
pub fn compute_bollinger(
    closes: &[f32],
    period: usize,
    multiplier: f32,
) -> (Vec<Option<f32>>, Vec<Option<f32>>, Vec<Option<f32>>) {
    let Ok(mut bb) = TaBollinger::new(period, multiplier as f64) else {
        let empty = vec![None; closes.len()];
        return (empty.clone(), empty.clone(), empty);
    };
    let mut upper = Vec::with_capacity(closes.len());
    let mut middle = Vec::with_capacity(closes.len());
    let mut lower = Vec::with_capacity(closes.len());
    for c in closes {
        let out = bb.next(*c as f64);
        upper.push(Some(out.upper as f32));
        middle.push(Some(out.average as f32));
        lower.push(Some(out.lower as f32));
    }
    (upper, middle, lower)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bollinger_produces_three_series() {
        let closes: Vec<f32> = (1..=30).map(|i| i as f32).collect();
        let (upper, middle, lower) = compute_bollinger(&closes, 20, 2.0);
        assert_eq!(upper.len(), 30);
        assert_eq!(middle.len(), 30);
        assert_eq!(lower.len(), 30);
    }

    #[test]
    fn bollinger_upper_above_lower() {
        let closes: Vec<f32> = (1..=30).map(|i| i as f32 + (i as f32 * 0.1).sin()).collect();
        let (upper, _, lower) = compute_bollinger(&closes, 20, 2.0);
        for (u, l) in upper.iter().zip(lower.iter()) {
            if let (Some(uv), Some(lv)) = (u, l) {
                assert!(uv >= lv, "upper {uv} should be >= lower {lv}");
            }
        }
    }

    #[test]
    fn bollinger_empty_returns_empty() {
        let (u, m, l) = compute_bollinger(&[], 20, 2.0);
        assert!(u.is_empty() && m.is_empty() && l.is_empty());
    }

    #[test]
    fn bollinger_invalid_period_returns_all_none() {
        let (u, _, _) = compute_bollinger(&[1.0, 2.0], 0, 2.0);
        assert!(u.iter().all(|v| v.is_none()));
    }
}
```

- [ ] **Step 2: Export from chart-core**

- [ ] **Step 3: Create desktop indicator kind with band rendering**

This is the first indicator with multi-series MainOverlay rendering. The key addition is the filled region between upper and lower bands.

```rust
// apps/desktop/src/ui_components/widgets/charts/indicators/kinds/bollinger.rs
use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke};

use zaned_chart_core::{
    CandleData, ComputedSeries, InputSpec, LegendEntry, ParamField, ParamKind, ParamSchema,
    ParamValues, Rgba, compute_bollinger,
};

use super::super::super::camera::Camera;
use super::super::super::util::egui_color;
use super::super::trait_def::{Indicator, RenderTarget};

pub const ID: &str = "bollinger";
pub const NAME: &str = "Bollinger Bands";
pub const LIKES: u32 = 28473;

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
            label: "StdDev Multiplier",
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
                default: Rgba::from_rgb(33, 150, 243),
            },
        },
    ],
};

pub struct Bollinger;

pub fn factory() -> Box<dyn Indicator> {
    Box::new(Bollinger)
}

impl Indicator for Bollinger {
    fn id(&self) -> &'static str {
        ID
    }
    fn display_name(&self, params: &ParamValues) -> String {
        format!(
            "BB({}, {})",
            params.int("period"),
            params.float("multiplier")
        )
    }
    fn target(&self) -> RenderTarget {
        RenderTarget::MainOverlay
    }

    fn inputs(&self, _params: &ParamValues) -> Vec<InputSpec> {
        vec![InputSpec::Closes]
    }

    fn compute(&self, inputs: &[&[f32]], params: &ParamValues) -> ComputedSeries {
        let closes = inputs.first().copied().unwrap_or(&[]);
        let period = params.int("period").max(1) as usize;
        let mult = params.float("multiplier");
        let (upper, middle, lower) = compute_bollinger(closes, period, mult);
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

        // Helper: map bar index + price to screen position
        let to_screen = |i: usize, y: f32| -> Pos2 {
            let x_pixel = ((i as f64 - camera.x_offset) * camera.x_scale) as f32;
            let y_pixel = (y as f64 - camera.y_offset) * camera.y_scale;
            Pos2::new(rect.left() + x_pixel, rect.bottom() - y_pixel as f32)
        };

        let upper = computed.series.get("upper");
        let middle = computed.series.get("middle");
        let lower = computed.series.get("lower");

        // Draw filled region between upper and lower
        if let (Some(up), Some(lo)) = (upper, lower) {
            let len = up.len().min(lo.len());
            let mut i = 0;
            while i < len {
                // Collect a run of valid upper+lower pairs
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
                // Build polygon: upper left-to-right, then lower right-to-left
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
        for (key, label) in [("upper", "BB↑"), ("middle", "BB"), ("lower", "BB↓")] {
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
```

- [ ] **Step 4: Register, build, test, commit**

```bash
git commit -m "feat(indicators): add Bollinger Bands with filled region rendering"
```

---

## Task 6: Keltner Channel

**Files:**
- Create: `crates/chart-core/src/indicators/keltner.rs`
- Modify: `crates/chart-core/src/indicators/mod.rs`, `lib.rs`
- Create: `apps/desktop/src/ui_components/widgets/charts/indicators/kinds/keltner.rs`
- Modify: `apps/desktop/.../kinds/mod.rs`, `registry.rs`

- [ ] **Step 1: Write compute function**

```rust
// crates/chart-core/src/indicators/keltner.rs
use ta::Next;
use ta::indicators::KeltnerChannel as TaKeltner;

use super::util::build_data_items;

/// Returns three series: (upper, middle, lower).
pub fn compute_keltner(
    highs: &[f32],
    lows: &[f32],
    closes: &[f32],
    period: usize,
    multiplier: f32,
) -> (Vec<Option<f32>>, Vec<Option<f32>>, Vec<Option<f32>>) {
    let items = build_data_items(highs, lows, closes, None);
    let Ok(mut kc) = TaKeltner::new(period, multiplier as f64) else {
        let empty = vec![None; items.len()];
        return (empty.clone(), empty.clone(), empty);
    };
    let mut upper = Vec::with_capacity(items.len());
    let mut middle = Vec::with_capacity(items.len());
    let mut lower = Vec::with_capacity(items.len());
    for item in &items {
        let out = kc.next(item);
        upper.push(Some(out.upper as f32));
        middle.push(Some(out.average as f32));
        lower.push(Some(out.lower as f32));
    }
    (upper, middle, lower)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keltner_produces_three_series() {
        let h: Vec<f32> = (1..=30).map(|i| i as f32 + 1.0).collect();
        let l: Vec<f32> = (1..=30).map(|i| i as f32 - 1.0).collect();
        let c: Vec<f32> = (1..=30).map(|i| i as f32).collect();
        let (u, m, lo) = compute_keltner(&h, &l, &c, 20, 2.0);
        assert_eq!(u.len(), 30);
        assert_eq!(m.len(), 30);
        assert_eq!(lo.len(), 30);
    }

    #[test]
    fn keltner_upper_above_lower() {
        let h: Vec<f32> = (1..=30).map(|i| i as f32 + 2.0).collect();
        let l: Vec<f32> = (1..=30).map(|i| i as f32 - 2.0).collect();
        let c: Vec<f32> = (1..=30).map(|i| i as f32).collect();
        let (u, _, lo) = compute_keltner(&h, &l, &c, 20, 2.0);
        for i in 20..30 {
            assert!(u[i].unwrap() >= lo[i].unwrap());
        }
    }
}
```

- [ ] **Step 2: Create desktop kind**

Reuses the band rendering pattern from Bollinger Bands. Key differences:
- `ID: "keltner"`, `NAME: "Keltner Channel"`, `LIKES: 7234`
- Inputs: `vec![InputSpec::Highs, InputSpec::Lows, InputSpec::Closes]`
- Compute calls: `compute_keltner(highs, lows, closes, period, multiplier)`
- `inputs[0]` = highs, `inputs[1]` = lows, `inputs[2]` = closes
- Default color: `Rgba::from_rgb(255, 193, 7)` (amber)
- Description: `"Keltner Channel. ATR-based volatility bands around an EMA. Tighter than Bollinger during low volatility, wider during high."`
- `draw_main()` is identical to Bollinger's band rendering — copy the implementation.

- [ ] **Step 3: Register, build, test, commit**

```bash
git commit -m "feat(indicators): add Keltner Channel"
```

---

## Task 7: MACD

**Files:**
- Modify: `crates/chart-core/src/indicators/macd.rs` (existing empty stub)
- Modify: `crates/chart-core/src/indicators/mod.rs`, `lib.rs`
- Create: `apps/desktop/src/ui_components/widgets/charts/indicators/kinds/macd.rs`
- Modify: `apps/desktop/.../kinds/mod.rs`, `registry.rs`

- [ ] **Step 1: Write compute function with tests**

```rust
// crates/chart-core/src/indicators/macd.rs
use ta::Next;
use ta::indicators::MovingAverageConvergenceDivergence as TaMacd;

/// Returns three series: (macd_line, signal_line, histogram).
pub fn compute_macd(
    closes: &[f32],
    fast: usize,
    slow: usize,
    signal: usize,
) -> (Vec<Option<f32>>, Vec<Option<f32>>, Vec<Option<f32>>) {
    let Ok(mut macd) = TaMacd::new(fast, slow, signal) else {
        let empty = vec![None; closes.len()];
        return (empty.clone(), empty.clone(), empty);
    };
    let mut macd_line = Vec::with_capacity(closes.len());
    let mut signal_line = Vec::with_capacity(closes.len());
    let mut histogram = Vec::with_capacity(closes.len());
    for c in closes {
        let out = macd.next(*c as f64);
        macd_line.push(Some(out.macd as f32));
        signal_line.push(Some(out.signal as f32));
        histogram.push(Some(out.histogram as f32));
    }
    (macd_line, signal_line, histogram)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macd_produces_three_series() {
        let closes: Vec<f32> = (1..=50).map(|i| i as f32).collect();
        let (m, s, h) = compute_macd(&closes, 12, 26, 9);
        assert_eq!(m.len(), 50);
        assert_eq!(s.len(), 50);
        assert_eq!(h.len(), 50);
    }

    #[test]
    fn macd_histogram_is_macd_minus_signal() {
        let closes: Vec<f32> = (1..=50).map(|i| i as f32 + (i as f32 * 0.3).sin()).collect();
        let (m, s, h) = compute_macd(&closes, 12, 26, 9);
        for i in 0..50 {
            if let (Some(mv), Some(sv), Some(hv)) = (m[i], s[i], h[i]) {
                assert!((hv - (mv - sv)).abs() < 1e-3, "histogram should be macd - signal at bar {i}");
            }
        }
    }

    #[test]
    fn macd_empty_returns_empty() {
        let (m, s, h) = compute_macd(&[], 12, 26, 9);
        assert!(m.is_empty() && s.is_empty() && h.is_empty());
    }

    #[test]
    fn macd_invalid_params_returns_all_none() {
        let (m, _, _) = compute_macd(&[1.0, 2.0], 0, 26, 9);
        assert!(m.iter().all(|v| v.is_none()));
    }
}
```

- [ ] **Step 2: Create desktop kind with histogram + lines rendering**

```rust
// apps/desktop/src/ui_components/widgets/charts/indicators/kinds/macd.rs
use egui::{Painter, Pos2, Rect, Shape, Stroke, Vec2};

use zaned_chart_core::{
    CandleData, ComputedSeries, InputSpec, LegendEntry, ParamField, ParamKind, ParamSchema,
    ParamValues, Rgba, compute_macd,
};

use super::super::super::util::egui_color;
use super::super::trait_def::{Indicator, RenderTarget};

pub const ID: &str = "macd";
pub const NAME: &str = "MACD";
pub const LIKES: u32 = 42156;

pub static SCHEMA: ParamSchema = ParamSchema {
    fields: &[
        ParamField {
            key: "fast",
            label: "Fast Period",
            kind: ParamKind::Int { default: 12, min: 2, max: 100 },
        },
        ParamField {
            key: "slow",
            label: "Slow Period",
            kind: ParamKind::Int { default: 26, min: 2, max: 100 },
        },
        ParamField {
            key: "signal",
            label: "Signal Period",
            kind: ParamKind::Int { default: 9, min: 2, max: 100 },
        },
        ParamField {
            key: "macd_color",
            label: "MACD Color",
            kind: ParamKind::Color { default: Rgba::from_rgb(33, 150, 243) },
        },
        ParamField {
            key: "signal_color",
            label: "Signal Color",
            kind: ParamKind::Color { default: Rgba::from_rgb(255, 152, 0) },
        },
        ParamField {
            key: "histogram_color",
            label: "Histogram Color",
            kind: ParamKind::Color { default: Rgba::from_rgb(100, 181, 246) },
        },
    ],
};

pub struct Macd;

pub fn factory() -> Box<dyn Indicator> {
    Box::new(Macd)
}

impl Indicator for Macd {
    fn id(&self) -> &'static str { ID }
    fn display_name(&self, params: &ParamValues) -> String {
        format!("MACD({},{},{})", params.int("fast"), params.int("slow"), params.int("signal"))
    }
    fn target(&self) -> RenderTarget { RenderTarget::SubPane }

    fn inputs(&self, _params: &ParamValues) -> Vec<InputSpec> {
        vec![InputSpec::Closes]
    }

    fn compute(&self, inputs: &[&[f32]], params: &ParamValues) -> ComputedSeries {
        let closes = inputs.first().copied().unwrap_or(&[]);
        let fast = params.int("fast").max(2) as usize;
        let slow = params.int("slow").max(2) as usize;
        let signal = params.int("signal").max(2) as usize;
        let (macd_line, signal_line, histogram) = compute_macd(closes, fast, slow, signal);
        let mut out = ComputedSeries::default();
        out.series.insert("macd", macd_line);
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
        for key in ["macd", "signal", "histogram"] {
            if let Some(series) = computed.series.get(key) {
                for v in series.iter().flatten() {
                    if *v < y_min { y_min = *v; }
                    if *v > y_max { y_max = *v; }
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
                [Pos2::new(pane_rect.left(), zero_y), Pos2::new(pane_rect.right(), zero_y)],
                Stroke::new(0.5, egui::Color32::from_rgb(60, 60, 66)),
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
                    let (min_y, max_y) = if top < bottom { (top, bottom) } else { (bottom, top) };
                    let bar_rect = Rect::from_min_size(
                        Pos2::new(x - bar_width / 2.0, min_y),
                        Vec2::new(bar_width, (max_y - min_y).max(1.0)),
                    );
                    let alpha = if *val >= 0.0 { 180u8 } else { 120 };
                    let c = egui::Color32::from_rgba_premultiplied(
                        hist_color.r(), hist_color.g(), hist_color.b(), alpha,
                    );
                    painter.rect_filled(bar_rect, 0.0, c);
                }
            }
        }

        // Draw MACD and signal lines
        for (key, color_key) in [("macd", "macd_color"), ("signal", "signal_color")] {
            let Some(series) = computed.series.get(key) else { continue; };
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
            ("macd", "MACD", "macd_color"),
            ("signal", "Signal", "signal_color"),
            ("histogram", "Hist", "histogram_color"),
        ] {
            if let Some(series) = computed.series.get(key) {
                if series.is_empty() { continue; }
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
```

- [ ] **Step 3: Register, build, test, commit**

```bash
git commit -m "feat(indicators): add MACD with histogram + signal line rendering"
```

---

## Task 8: PPO (Percentage Price Oscillator)

Structurally identical to MACD but uses `ta::PercentagePriceOscillator`.

**Files:** Same pattern as Task 7.

- [ ] **Step 1: Write compute function**

```rust
// crates/chart-core/src/indicators/ppo.rs
use ta::Next;
use ta::indicators::PercentagePriceOscillator as TaPpo;

pub fn compute_ppo(
    closes: &[f32],
    fast: usize,
    slow: usize,
    signal: usize,
) -> (Vec<Option<f32>>, Vec<Option<f32>>, Vec<Option<f32>>) {
    let Ok(mut ppo) = TaPpo::new(fast, slow, signal) else {
        let empty = vec![None; closes.len()];
        return (empty.clone(), empty.clone(), empty);
    };
    let mut ppo_line = Vec::with_capacity(closes.len());
    let mut signal_line = Vec::with_capacity(closes.len());
    let mut histogram = Vec::with_capacity(closes.len());
    for c in closes {
        let out = ppo.next(*c as f64);
        ppo_line.push(Some(out.ppo as f32));
        signal_line.push(Some(out.signal as f32));
        histogram.push(Some(out.histogram as f32));
    }
    (ppo_line, signal_line, histogram)
}
```

- [ ] **Step 2: Create desktop kind**

Copy the MACD desktop kind pattern. Differences:
- `ID: "ppo"`, `NAME: "PPO"`, `LIKES: 3421`
- Series keys: `"ppo"`, `"signal"`, `"histogram"`
- Param color keys: `"ppo_color"`, `"signal_color"`, `"histogram_color"`
- Default PPO color: `Rgba::from_rgb(76, 175, 80)` (green)
- Description: `"Percentage Price Oscillator. Like MACD but expressed as a percentage, making it comparable across different price levels."`

- [ ] **Step 3: Register, build, test, commit**

```bash
git commit -m "feat(indicators): add PPO (Percentage Price Oscillator)"
```

---

## Task 9: Slow Stochastic

**Files:**
- Create: `crates/chart-core/src/indicators/stochastic.rs`
- Create: `apps/desktop/.../kinds/stochastic.rs`
- Modify: mod.rs, lib.rs, registry.rs as usual

- [ ] **Step 1: Write compute function**

```rust
// crates/chart-core/src/indicators/stochastic.rs
use ta::Next;
use ta::indicators::SlowStochastic as TaStoch;

use super::util::build_data_items;

/// Returns two series: (%K, %D).
pub fn compute_stochastic(
    highs: &[f32],
    lows: &[f32],
    closes: &[f32],
    period: usize,
) -> (Vec<Option<f32>>, Vec<Option<f32>>) {
    let items = build_data_items(highs, lows, closes, None);
    let Ok(mut stoch) = TaStoch::new(period, period) else {
        let empty = vec![None; items.len()];
        return (empty.clone(), empty);
    };
    let mut k_series = Vec::with_capacity(items.len());
    let mut d_series = Vec::with_capacity(items.len());
    for item in &items {
        let out = stoch.next(item);
        k_series.push(Some(out.k as f32));
        d_series.push(Some(out.d as f32));
    }
    (k_series, d_series)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stochastic_produces_two_series() {
        let h: Vec<f32> = (1..=30).map(|i| i as f32 + 2.0).collect();
        let l: Vec<f32> = (1..=30).map(|i| i as f32 - 2.0).collect();
        let c: Vec<f32> = (1..=30).map(|i| i as f32).collect();
        let (k, d) = compute_stochastic(&h, &l, &c, 14);
        assert_eq!(k.len(), 30);
        assert_eq!(d.len(), 30);
    }

    #[test]
    fn stochastic_values_in_0_100_range() {
        let h: Vec<f32> = (1..=50).map(|i| i as f32 + 3.0).collect();
        let l: Vec<f32> = (1..=50).map(|i| (i as f32 - 3.0).max(0.1)).collect();
        let c: Vec<f32> = (1..=50).map(|i| i as f32).collect();
        let (k, _) = compute_stochastic(&h, &l, &c, 14);
        for v in k.iter().flatten() {
            assert!(*v >= 0.0 && *v <= 100.0, "stoch %K out of range: {v}");
        }
    }
}
```

- [ ] **Step 2: Create desktop kind**

SubPane with two lines and guide lines at 20/80, range 0–100 (same rendering approach as RSI).
- `ID: "stochastic"`, `NAME: "Stochastic"`, `LIKES: 19832`
- Inputs: `vec![InputSpec::Highs, InputSpec::Lows, InputSpec::Closes]`
- Params: period (Int, default 14), k_color, d_color
- Default K color: `Rgba::from_rgb(33, 150, 243)`, D color: `Rgba::from_rgb(255, 152, 0)`
- Description: `"Slow Stochastic Oscillator. Compares closing price to the high-low range. %K above 80 suggests overbought; below 20, oversold."`
- `draw_pane()`: same as RSI but draw two lines (%K and %D) and guide lines at 20/80

- [ ] **Step 3: Register, build, test, commit**

```bash
git commit -m "feat(indicators): add Slow Stochastic oscillator"
```

---

## Task 10: CCI (Commodity Channel Index)

- [ ] **Step 1: Write compute function**

```rust
// crates/chart-core/src/indicators/cci.rs
use ta::Next;
use ta::indicators::CommodityChannelIndex as TaCci;
use super::util::build_data_items;

pub fn compute_cci(
    highs: &[f32],
    lows: &[f32],
    closes: &[f32],
    period: usize,
) -> Vec<Option<f32>> {
    let items = build_data_items(highs, lows, closes, None);
    let Ok(mut cci) = TaCci::new(period) else {
        return vec![None; items.len()];
    };
    items.iter().map(|item| Some(cci.next(item) as f32)).collect()
}
```

Tests: verify length, verify values fluctuate around zero, verify empty/invalid period.

- [ ] **Step 2: Create desktop kind**

SubPane, single line + guide lines at +100/-100. Zero-centered auto-scaling.
- `ID: "cci"`, `NAME: "CCI"`, `LIKES: 8765`
- Inputs: `[Highs, Lows, Closes]`
- Default color: `Rgba::from_rgb(0, 150, 136)` (teal)
- Description: `"Commodity Channel Index. Measures deviation from the statistical mean. Readings above +100 or below -100 suggest extreme conditions."`

- [ ] **Step 3: Register, build, test, commit**

```bash
git commit -m "feat(indicators): add CCI (Commodity Channel Index)"
```

---

## Task 11: Williams %R

- [ ] **Step 1: Write compute function (custom)**

```rust
// crates/chart-core/src/indicators/williams_r.rs
use super::util::{rolling_high, rolling_low};

pub fn compute_williams_r(
    highs: &[f32],
    lows: &[f32],
    closes: &[f32],
    period: usize,
) -> Vec<Option<f32>> {
    let rh = rolling_high(highs, period);
    let rl = rolling_low(lows, period);
    let n = closes.len().min(rh.len()).min(rl.len());
    (0..n)
        .map(|i| match (rh[i], rl[i]) {
            (Some(hh), Some(ll)) => {
                let range = hh - ll;
                if range.abs() < 1e-10 {
                    Some(-50.0)
                } else {
                    Some((hh - closes[i]) / range * -100.0)
                }
            }
            _ => None,
        })
        .collect()
}
```

Tests: values in -100..0 range, valid after warmup.

- [ ] **Step 2: Create desktop kind**

SubPane, single line + guide lines at -20/-80, range -100 to 0.
- `ID: "williams_r"`, `NAME: "Williams %R"`, `LIKES: 7654`
- Inputs: `[Highs, Lows, Closes]`
- Default color: `Rgba::from_rgb(244, 67, 54)` (red)
- Description: `"Williams %R. Momentum oscillator ranging from -100 to 0. Above -20 suggests overbought; below -80, oversold."`

- [ ] **Step 3: Register, build, test, commit**

```bash
git commit -m "feat(indicators): add Williams %R"
```

---

## Task 12: ROC (Rate of Change)

- [ ] **Step 1: Write compute function**

```rust
// crates/chart-core/src/indicators/roc.rs
use ta::Next;
use ta::indicators::RateOfChange as TaRoc;

pub fn compute_roc(closes: &[f32], period: usize) -> Vec<Option<f32>> {
    let Ok(mut roc) = TaRoc::new(period) else {
        return vec![None; closes.len()];
    };
    closes.iter().map(|c| Some(roc.next(*c as f64) as f32)).collect()
}
```

- [ ] **Step 2: Create desktop kind**

SubPane, single line + zero guide line.
- `ID: "roc"`, `NAME: "ROC"`, `LIKES: 5432`
- Inputs: `[Closes]`
- Default color: `Rgba::from_rgb(121, 85, 72)` (brown)
- Description: `"Rate of Change. Percentage change between current price and N bars ago. Positive values indicate upward momentum."`

- [ ] **Step 3: Register, build, test, commit**

```bash
git commit -m "feat(indicators): add ROC (Rate of Change)"
```

---

## Task 13: MFI (Money Flow Index)

- [ ] **Step 1: Write compute function**

```rust
// crates/chart-core/src/indicators/mfi.rs
use ta::Next;
use ta::indicators::MoneyFlowIndex as TaMfi;
use super::util::build_data_items;

pub fn compute_mfi(
    highs: &[f32],
    lows: &[f32],
    closes: &[f32],
    volumes: &[f32],
    period: usize,
) -> Vec<Option<f32>> {
    let items = build_data_items(highs, lows, closes, Some(volumes));
    let Ok(mut mfi) = TaMfi::new(period) else {
        return vec![None; items.len()];
    };
    items.iter().map(|item| Some(mfi.next(item) as f32)).collect()
}
```

- [ ] **Step 2: Create desktop kind**

SubPane, single line + guide lines at 20/80, range 0–100 (like RSI).
- `ID: "mfi"`, `NAME: "MFI"`, `LIKES: 6543`
- Inputs: `[Highs, Lows, Closes, Volumes]`
- Default color: `Rgba::from_rgb(255, 87, 34)` (deep orange)
- Description: `"Money Flow Index. Volume-weighted RSI. Above 80 suggests overbought with heavy volume; below 20, oversold."`

- [ ] **Step 3: Register, build, test, commit**

```bash
git commit -m "feat(indicators): add MFI (Money Flow Index)"
```

---

## Task 14: OBV (On Balance Volume)

- [ ] **Step 1: Write compute function (custom)**

```rust
// crates/chart-core/src/indicators/obv.rs

pub fn compute_obv(closes: &[f32], volumes: &[f32]) -> Vec<Option<f32>> {
    let n = closes.len().min(volumes.len());
    if n == 0 {
        return Vec::new();
    }
    let mut result = Vec::with_capacity(n);
    let mut obv: f64 = 0.0;
    result.push(Some(0.0f32));
    for i in 1..n {
        if closes[i] > closes[i - 1] {
            obv += volumes[i] as f64;
        } else if closes[i] < closes[i - 1] {
            obv -= volumes[i] as f64;
        }
        result.push(Some(obv as f32));
    }
    result
}
```

Note: Using a custom implementation rather than `ta::OnBalanceVolume` since OBV is straightforward and avoids the DataItem complexity.

- [ ] **Step 2: Create desktop kind**

SubPane, single line, auto-scaled. No guide lines.
- `ID: "obv"`, `NAME: "OBV"`, `LIKES: 9876`
- Inputs: `[Closes, Volumes]`
- Default color: `Rgba::from_rgb(63, 81, 181)` (indigo)
- Description: `"On Balance Volume. Cumulative volume flow — adds volume on up days, subtracts on down days. Rising OBV suggests accumulation."`

- [ ] **Step 3: Register, build, test, commit**

```bash
git commit -m "feat(indicators): add OBV (On Balance Volume)"
```

---

## Task 15: VWAP (Volume Weighted Average Price)

- [ ] **Step 1: Write compute function (custom, uses session boundaries)**

```rust
// crates/chart-core/src/indicators/vwap.rs
use super::util::session_boundaries;

pub fn compute_vwap(
    highs: &[f32],
    lows: &[f32],
    closes: &[f32],
    volumes: &[f32],
    dates: &[String],
) -> Vec<Option<f32>> {
    let n = highs.len().min(lows.len()).min(closes.len()).min(volumes.len());
    if n == 0 {
        return Vec::new();
    }
    let boundaries = session_boundaries(dates);
    let mut result = vec![None; n];
    let mut cum_tp_vol: f64 = 0.0;
    let mut cum_vol: f64 = 0.0;
    let mut boundary_idx = 0;

    for i in 0..n {
        // Check if we've crossed into a new session
        if boundary_idx + 1 < boundaries.len() && i >= boundaries[boundary_idx + 1] {
            boundary_idx += 1;
            cum_tp_vol = 0.0;
            cum_vol = 0.0;
        }
        let typical_price = (highs[i] + lows[i] + closes[i]) as f64 / 3.0;
        cum_tp_vol += typical_price * volumes[i] as f64;
        cum_vol += volumes[i] as f64;
        if cum_vol > 0.0 {
            result[i] = Some((cum_tp_vol / cum_vol) as f32);
        }
    }
    result
}
```

Note: `compute_vwap` needs `dates` to detect session boundaries — this means the desktop kind's `compute()` will need access to `CandleData.dates`. Since the current `Indicator::compute()` only receives `&[&[f32]]`, the VWAP desktop kind will need to use `draw_main()` with the `data: &CandleData` parameter to perform the actual VWAP calculation there, OR we can have the compute function ignore the standard inputs and instead compute from within `draw_main()`. The cleaner approach: perform the computation in `compute()` by passing dates as a special float-encoded series, OR compute it in `draw_main()` since it receives `data: &CandleData`. Choose the latter for simplicity.

Update: The `compute` function on the Indicator trait receives `&[&[f32]]` — no access to dates. Two options:
1. Compute in `draw_main()` where `data: &CandleData` is available
2. Add a new compute method that takes CandleData

Option 1 is simpler and avoids trait changes. The VWAP indicator will compute in `draw_main()` and `draw_pane()`, caching via `ComputedSeries` won't be used for the actual data — instead it will store a sentinel. Actually, the cleanest approach is to compute it in the manager's `ensure_computed` by special-casing VWAP. But that couples the manager to a specific indicator.

**Simplest approach:** Since `draw_main()` already receives `data: &CandleData`, compute VWAP there on each frame. The computation is O(n) and fast. Store the result in the ComputedSeries from `compute()` as a dummy, and recompute in draw. This avoids any trait or manager changes.

Actually, looking more carefully at the trait: `compute()` is the pure computation step, and `draw_main()` receives both `data` and `computed`. The simplest approach: have `compute()` return empty series, and compute the VWAP inline in `draw_main()` using `data.dates`, `data.instances`. The downside is it recomputes every frame, but it's O(n) so negligible.

- [ ] **Step 2: Create desktop kind**

MainOverlay, single line. Key difference: computes inline in `draw_main()`.
- `ID: "vwap"`, `NAME: "VWAP"`, `LIKES: 18234`
- Inputs: `[Highs, Lows, Closes, Volumes]` (for input resolution, even though actual computation happens in draw)
- Default color: `Rgba::from_rgb(255, 235, 59)` (yellow)
- Description: `"Volume Weighted Average Price. Average price weighted by volume, resetting each session. Key institutional reference level."`

- [ ] **Step 3: Register, build, test, commit**

```bash
git commit -m "feat(indicators): add VWAP (Volume Weighted Average Price)"
```

---

## Task 16: ATR (Average True Range)

- [ ] **Step 1: Write compute function**

```rust
// crates/chart-core/src/indicators/atr.rs
use ta::Next;
use ta::indicators::AverageTrueRange as TaAtr;
use super::util::build_data_items;

pub fn compute_atr(
    highs: &[f32],
    lows: &[f32],
    closes: &[f32],
    period: usize,
) -> Vec<Option<f32>> {
    let items = build_data_items(highs, lows, closes, None);
    let Ok(mut atr) = TaAtr::new(period) else {
        return vec![None; items.len()];
    };
    items.iter().map(|item| Some(atr.next(item) as f32)).collect()
}
```

- [ ] **Step 2: Create desktop kind**

SubPane, single line, auto-scaled.
- `ID: "atr"`, `NAME: "ATR"`, `LIKES: 15432`
- Inputs: `[Highs, Lows, Closes]`
- Default color: `Rgba::from_rgb(205, 220, 57)` (lime)
- Description: `"Average True Range. Measures market volatility by decomposing the range of each bar. Higher ATR means more volatile."`

- [ ] **Step 3: Register, build, test, commit**

```bash
git commit -m "feat(indicators): add ATR (Average True Range)"
```

---

## Task 17: ADX / DMI

- [ ] **Step 1: Write compute function (custom)**

```rust
// crates/chart-core/src/indicators/adx.rs

/// Returns three series: (ADX, +DI, -DI).
pub fn compute_adx(
    highs: &[f32],
    lows: &[f32],
    closes: &[f32],
    period: usize,
) -> (Vec<Option<f32>>, Vec<Option<f32>>, Vec<Option<f32>>) {
    let n = highs.len().min(lows.len()).min(closes.len());
    if n < 2 || period == 0 {
        let empty = vec![None; n];
        return (empty.clone(), empty.clone(), empty);
    }

    // Step 1: Calculate True Range, +DM, -DM for each bar
    let mut tr = vec![0.0f64; n];
    let mut plus_dm = vec![0.0f64; n];
    let mut minus_dm = vec![0.0f64; n];

    for i in 1..n {
        let h = highs[i] as f64;
        let l = lows[i] as f64;
        let pc = closes[i - 1] as f64;
        tr[i] = (h - l).max((h - pc).abs()).max((l - pc).abs());

        let up = h - highs[i - 1] as f64;
        let down = lows[i - 1] as f64 - l;
        plus_dm[i] = if up > down && up > 0.0 { up } else { 0.0 };
        minus_dm[i] = if down > up && down > 0.0 { down } else { 0.0 };
    }

    // Step 2: Wilder smoothing (period-bar EMA with alpha = 1/period)
    let smooth = |raw: &[f64]| -> Vec<f64> {
        let mut s = vec![0.0; n];
        // Seed with sum of first `period` values
        let seed: f64 = raw[1..=(period.min(n - 1))].iter().sum();
        if period < n {
            s[period] = seed;
            for i in (period + 1)..n {
                s[i] = s[i - 1] - s[i - 1] / period as f64 + raw[i];
            }
        }
        s
    };

    let atr_s = smooth(&tr);
    let plus_dm_s = smooth(&plus_dm);
    let minus_dm_s = smooth(&minus_dm);

    // Step 3: +DI, -DI, DX
    let mut di_plus = vec![None; n];
    let mut di_minus = vec![None; n];
    let mut dx = vec![0.0f64; n];

    for i in period..n {
        if atr_s[i] > 0.0 {
            let pdv = (plus_dm_s[i] / atr_s[i] * 100.0) as f32;
            let mdv = (minus_dm_s[i] / atr_s[i] * 100.0) as f32;
            di_plus[i] = Some(pdv);
            di_minus[i] = Some(mdv);
            let sum = pdv + mdv;
            dx[i] = if sum > 0.0 {
                ((pdv - mdv).abs() / sum * 100.0) as f64
            } else {
                0.0
            };
        }
    }

    // Step 4: ADX = Wilder-smoothed DX
    let mut adx = vec![None; n];
    let start = period * 2;
    if start < n {
        let seed: f64 = dx[period..start].iter().sum::<f64>() / period as f64;
        adx[start] = Some(seed as f32);
        for i in (start + 1)..n {
            if let Some(prev) = adx[i - 1] {
                let v = (prev as f64 * (period as f64 - 1.0) + dx[i]) / period as f64;
                adx[i] = Some(v as f32);
            }
        }
    }

    (adx, di_plus, di_minus)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adx_produces_three_series() {
        let h: Vec<f32> = (1..=50).map(|i| i as f32 + 2.0).collect();
        let l: Vec<f32> = (1..=50).map(|i| i as f32 - 2.0).collect();
        let c: Vec<f32> = (1..=50).map(|i| i as f32).collect();
        let (adx, dip, dim) = compute_adx(&h, &l, &c, 14);
        assert_eq!(adx.len(), 50);
        assert_eq!(dip.len(), 50);
        assert_eq!(dim.len(), 50);
    }

    #[test]
    fn adx_values_in_0_100() {
        let h: Vec<f32> = (1..=100).map(|i| i as f32 + 3.0).collect();
        let l: Vec<f32> = (1..=100).map(|i| (i as f32 - 3.0).max(0.1)).collect();
        let c: Vec<f32> = (1..=100).map(|i| i as f32).collect();
        let (adx, _, _) = compute_adx(&h, &l, &c, 14);
        for v in adx.iter().flatten() {
            assert!(*v >= 0.0 && *v <= 100.0, "ADX out of range: {v}");
        }
    }
}
```

- [ ] **Step 2: Create desktop kind**

SubPane, three lines + guide line at 25.
- `ID: "adx"`, `NAME: "ADX"`, `LIKES: 11234`
- Inputs: `[Highs, Lows, Closes]`
- Params: period, adx_color, di_plus_color, di_minus_color
- Default colors: ADX white, +DI green `Rgba::from_rgb(76, 175, 80)`, -DI red `Rgba::from_rgb(244, 67, 54)`
- Description: `"Average Directional Index. Measures trend strength (not direction). ADX above 25 suggests a strong trend; +DI/-DI show direction."`

- [ ] **Step 3: Register, build, test, commit**

```bash
git commit -m "feat(indicators): add ADX/DMI (Average Directional Index)"
```

---

## Task 18: Supertrend

- [ ] **Step 1: Write compute function (custom)**

```rust
// crates/chart-core/src/indicators/supertrend.rs

/// Returns two series: (supertrend_value, direction). Direction is 1.0 for
/// uptrend, -1.0 for downtrend.
pub fn compute_supertrend(
    highs: &[f32],
    lows: &[f32],
    closes: &[f32],
    period: usize,
    multiplier: f32,
) -> (Vec<Option<f32>>, Vec<Option<f32>>) {
    // Uses ATR internally
    use super::util::build_data_items;
    use ta::Next;
    use ta::indicators::AverageTrueRange;

    let n = highs.len().min(lows.len()).min(closes.len());
    if n == 0 || period == 0 {
        return (vec![None; n], vec![None; n]);
    }

    let items = build_data_items(highs, lows, closes, None);
    let Ok(mut atr_ind) = AverageTrueRange::new(period) else {
        return (vec![None; n], vec![None; n]);
    };

    let mut atr_vals = Vec::with_capacity(n);
    for item in &items {
        atr_vals.push(atr_ind.next(item) as f32);
    }

    let mut st = vec![None; n];
    let mut dir = vec![None; n];
    let mut upper_band = vec![0.0f32; n];
    let mut lower_band = vec![0.0f32; n];

    for i in 0..n {
        let hl2 = (highs[i] + lows[i]) / 2.0;
        let basic_upper = hl2 + multiplier * atr_vals[i];
        let basic_lower = hl2 - multiplier * atr_vals[i];

        upper_band[i] = if i > 0 && basic_upper < upper_band[i - 1] || (i > 0 && closes[i - 1] > upper_band[i - 1]) {
            basic_upper
        } else if i > 0 {
            upper_band[i - 1]
        } else {
            basic_upper
        };

        lower_band[i] = if i > 0 && basic_lower > lower_band[i - 1] || (i > 0 && closes[i - 1] < lower_band[i - 1]) {
            basic_lower
        } else if i > 0 {
            lower_band[i - 1]
        } else {
            basic_lower
        };

        if i < period {
            continue;
        }

        if i == period {
            // Initialize direction based on close vs bands
            if closes[i] <= upper_band[i] {
                st[i] = Some(upper_band[i]);
                dir[i] = Some(-1.0);
            } else {
                st[i] = Some(lower_band[i]);
                dir[i] = Some(1.0);
            }
            continue;
        }

        let prev_dir = dir[i - 1].unwrap_or(-1.0);
        if prev_dir > 0.0 {
            // Was uptrend
            if closes[i] < lower_band[i] {
                st[i] = Some(upper_band[i]);
                dir[i] = Some(-1.0);
            } else {
                st[i] = Some(lower_band[i]);
                dir[i] = Some(1.0);
            }
        } else {
            // Was downtrend
            if closes[i] > upper_band[i] {
                st[i] = Some(lower_band[i]);
                dir[i] = Some(1.0);
            } else {
                st[i] = Some(upper_band[i]);
                dir[i] = Some(-1.0);
            }
        }
    }

    (st, dir)
}
```

- [ ] **Step 2: Create desktop kind**

MainOverlay, single line that changes color based on direction.
- `ID: "supertrend"`, `NAME: "Supertrend"`, `LIKES: 14321`
- Inputs: `[Highs, Lows, Closes]`
- Params: period (Int, default 10), multiplier (Float, default 3.0), up_color, down_color
- Default up: `Rgba::from_rgb(76, 175, 80)` (green), down: `Rgba::from_rgb(244, 67, 54)` (red)
- Description: `"Supertrend. ATR-based trend-following overlay. Green when price is in uptrend, red in downtrend. Useful for trailing stop levels."`
- Rendering: iterate series, draw segments using up_color when direction > 0, down_color when direction < 0.

- [ ] **Step 3: Register, build, test, commit**

```bash
git commit -m "feat(indicators): add Supertrend"
```

---

## Task 19: Ichimoku Cloud

- [ ] **Step 1: Write compute function (custom)**

```rust
// crates/chart-core/src/indicators/ichimoku.rs
use super::util::{rolling_high, rolling_low};

/// Returns five series: (tenkan, kijun, senkou_a, senkou_b, chikou).
/// senkou_a and senkou_b are displaced forward by `kijun_period` bars
/// (series is longer than input). chikou is displaced backward.
pub fn compute_ichimoku(
    highs: &[f32],
    lows: &[f32],
    closes: &[f32],
    tenkan_period: usize,
    kijun_period: usize,
    senkou_b_period: usize,
) -> (
    Vec<Option<f32>>,
    Vec<Option<f32>>,
    Vec<Option<f32>>,
    Vec<Option<f32>>,
    Vec<Option<f32>>,
) {
    let n = highs.len().min(lows.len()).min(closes.len());
    let displacement = kijun_period;

    // Tenkan-sen = (highest high + lowest low) / 2 over tenkan_period
    let th = rolling_high(highs, tenkan_period);
    let tl = rolling_low(lows, tenkan_period);
    let tenkan: Vec<Option<f32>> = th
        .iter()
        .zip(tl.iter())
        .map(|(h, l)| match (h, l) {
            (Some(hv), Some(lv)) => Some((hv + lv) / 2.0),
            _ => None,
        })
        .collect();

    // Kijun-sen = same formula over kijun_period
    let kh = rolling_high(highs, kijun_period);
    let kl = rolling_low(lows, kijun_period);
    let kijun: Vec<Option<f32>> = kh
        .iter()
        .zip(kl.iter())
        .map(|(h, l)| match (h, l) {
            (Some(hv), Some(lv)) => Some((hv + lv) / 2.0),
            _ => None,
        })
        .collect();

    // Senkou Span A = (tenkan + kijun) / 2, displaced forward by kijun_period
    let total_len = n + displacement;
    let mut senkou_a = vec![None; total_len];
    for i in 0..n {
        senkou_a[i + displacement] = match (tenkan.get(i), kijun.get(i)) {
            (Some(Some(t)), Some(Some(k))) => Some((t + k) / 2.0),
            _ => None,
        };
    }

    // Senkou Span B = (highest high + lowest low) / 2 over senkou_b_period, displaced forward
    let sh = rolling_high(highs, senkou_b_period);
    let sl = rolling_low(lows, senkou_b_period);
    let mut senkou_b = vec![None; total_len];
    for i in 0..n {
        senkou_b[i + displacement] = match (sh.get(i), sl.get(i)) {
            (Some(Some(hv)), Some(Some(lv))) => Some((hv + lv) / 2.0),
            _ => None,
        };
    }

    // Chikou Span = close displaced backward by kijun_period
    let mut chikou = vec![None; n];
    for i in displacement..n {
        chikou[i - displacement] = Some(closes[i]);
    }

    (tenkan, kijun, senkou_a, senkou_b, chikou)
}
```

- [ ] **Step 2: Create desktop kind**

MainOverlay with cloud fill between senkou_a/senkou_b + individual lines.
- `ID: "ichimoku"`, `NAME: "Ichimoku Cloud"`, `LIKES: 21345`
- Inputs: `[Highs, Lows, Closes]`
- Params: tenkan (9), kijun (26), senkou_b (52), tenkan_color, kijun_color, cloud_up_color, cloud_down_color, chikou_color
- Description: `"Ichimoku Cloud. All-in-one trend indicator showing support, resistance, momentum, and trend direction via five plotted lines and a shaded cloud."`
- Rendering note: senkou_a/senkou_b series are longer than the candle count (displaced forward). The draw code must handle indices beyond data length by mapping them to screen x positions using camera math. The cloud fill uses the same polygon approach as Bollinger.

- [ ] **Step 3: Register, build, test, commit**

```bash
git commit -m "feat(indicators): add Ichimoku Cloud"
```

---

## Task 20: Chandelier Exit

- [ ] **Step 1: Write compute function**

```rust
// crates/chart-core/src/indicators/chandelier.rs
use ta::Next;
use ta::indicators::ChandelierExit as TaChandelier;
use super::util::build_data_items;

/// Returns two series: (long_exit, short_exit).
pub fn compute_chandelier(
    highs: &[f32],
    lows: &[f32],
    closes: &[f32],
    period: usize,
    multiplier: f32,
) -> (Vec<Option<f32>>, Vec<Option<f32>>) {
    let items = build_data_items(highs, lows, closes, None);
    let Ok(mut ce) = TaChandelier::new(period, multiplier as f64) else {
        let empty = vec![None; items.len()];
        return (empty.clone(), empty);
    };
    let mut long_exit = Vec::with_capacity(items.len());
    let mut short_exit = Vec::with_capacity(items.len());
    for item in &items {
        let out = ce.next(item);
        long_exit.push(Some(out.long as f32));
        short_exit.push(Some(out.short as f32));
    }
    (long_exit, short_exit)
}
```

- [ ] **Step 2: Create desktop kind**

MainOverlay, two lines (long above price, short below).
- `ID: "chandelier"`, `NAME: "Chandelier Exit"`, `LIKES: 5678`
- Inputs: `[Highs, Lows, Closes]`
- Params: period (22), multiplier (3.0), long_color, short_color
- Default long: `Rgba::from_rgb(76, 175, 80)` (green), short: `Rgba::from_rgb(244, 67, 54)` (red)
- Description: `"Chandelier Exit. ATR-based trailing stop levels. Long exit below price for longs, short exit above for shorts."`

- [ ] **Step 3: Register, build, test, commit**

```bash
git commit -m "feat(indicators): add Chandelier Exit"
```

---

## Task 21: Parabolic SAR

- [ ] **Step 1: Write compute function (custom)**

```rust
// crates/chart-core/src/indicators/parabolic_sar.rs

pub fn compute_parabolic_sar(
    highs: &[f32],
    lows: &[f32],
    accel_start: f32,
    accel_max: f32,
) -> Vec<Option<f32>> {
    let n = highs.len().min(lows.len());
    if n < 2 {
        return vec![None; n];
    }

    let mut sar = vec![None; n];
    let mut is_long = highs[1] > highs[0]; // initial trend guess
    let mut af = accel_start;
    let mut ep = if is_long { highs[0] } else { lows[0] };
    let mut current_sar = if is_long { lows[0] } else { highs[0] };

    sar[0] = Some(current_sar);

    for i in 1..n {
        // Update SAR
        let prev_sar = current_sar;
        current_sar = prev_sar + af * (ep - prev_sar);

        if is_long {
            // Clamp SAR to not be above the prior two lows
            current_sar = current_sar.min(lows[i - 1]);
            if i >= 2 {
                current_sar = current_sar.min(lows[i - 2]);
            }

            if lows[i] < current_sar {
                // Flip to short
                is_long = false;
                current_sar = ep;
                ep = lows[i];
                af = accel_start;
            } else {
                if highs[i] > ep {
                    ep = highs[i];
                    af = (af + accel_start).min(accel_max);
                }
            }
        } else {
            // Clamp SAR to not be below the prior two highs
            current_sar = current_sar.max(highs[i - 1]);
            if i >= 2 {
                current_sar = current_sar.max(highs[i - 2]);
            }

            if highs[i] > current_sar {
                // Flip to long
                is_long = true;
                current_sar = ep;
                ep = highs[i];
                af = accel_start;
            } else {
                if lows[i] < ep {
                    ep = lows[i];
                    af = (af + accel_start).min(accel_max);
                }
            }
        }

        sar[i] = Some(current_sar);
    }

    sar
}
```

- [ ] **Step 2: Create desktop kind with dot rendering**

MainOverlay, dots at each bar position.
- `ID: "parabolic_sar"`, `NAME: "Parabolic SAR"`, `LIKES: 13456`
- Inputs: `[Highs, Lows]`
- Params: acceleration (Float 0.001–0.1, default 0.02), max_acceleration (Float 0.05–0.5, default 0.2), color
- Default color: `Rgba::from_rgb(171, 71, 188)` (purple)
- Description: `"Parabolic SAR. Trailing stop-and-reverse dots. Dots below price indicate uptrend, above indicate downtrend."`
- `draw_main()` uses `painter.circle_filled(pos, 2.5, color)` instead of line segments.

- [ ] **Step 3: Register, build, test, commit**

```bash
git commit -m "feat(indicators): add Parabolic SAR with dot rendering"
```

---

## Task 22: Pivot Points

- [ ] **Step 1: Write compute function (custom)**

```rust
// crates/chart-core/src/indicators/pivot_points.rs
use super::util::session_boundaries;

/// Pivot type: 0 = Standard, 1 = Fibonacci, 2 = Woodie.
/// Returns 7 series: (pivot, r1, r2, r3, s1, s2, s3).
pub fn compute_pivot_points(
    highs: &[f32],
    lows: &[f32],
    closes: &[f32],
    dates: &[String],
    pivot_type: i32,
) -> (
    Vec<Option<f32>>,
    Vec<Option<f32>>,
    Vec<Option<f32>>,
    Vec<Option<f32>>,
    Vec<Option<f32>>,
    Vec<Option<f32>>,
    Vec<Option<f32>>,
) {
    let n = highs.len().min(lows.len()).min(closes.len());
    let boundaries = session_boundaries(dates);
    let mut pivot = vec![None; n];
    let mut r1 = vec![None; n];
    let mut r2 = vec![None; n];
    let mut r3 = vec![None; n];
    let mut s1 = vec![None; n];
    let mut s2 = vec![None; n];
    let mut s3 = vec![None; n];

    // For each session (starting from the second), use previous session's HLC
    for sess_idx in 1..boundaries.len() {
        let prev_start = boundaries[sess_idx - 1];
        let prev_end = if sess_idx < boundaries.len() {
            boundaries[sess_idx]
        } else {
            n
        };

        // Find prev session's high, low, close
        let mut ph = f32::MIN;
        let mut pl = f32::MAX;
        for i in prev_start..prev_end {
            if highs[i] > ph { ph = highs[i]; }
            if lows[i] < pl { pl = lows[i]; }
        }
        let pc = closes[prev_end - 1];

        let (p, r1v, r2v, r3v, s1v, s2v, s3v) = match pivot_type {
            1 => {
                // Fibonacci
                let p = (ph + pl + pc) / 3.0;
                let range = ph - pl;
                (
                    p,
                    p + range * 0.382,
                    p + range * 0.618,
                    p + range * 1.0,
                    p - range * 0.382,
                    p - range * 0.618,
                    p - range * 1.0,
                )
            }
            2 => {
                // Woodie
                let p = (ph + pl + 2.0 * pc) / 4.0;
                (
                    p,
                    2.0 * p - pl,
                    p + (ph - pl),
                    ph + 2.0 * (p - pl),
                    2.0 * p - ph,
                    p - (ph - pl),
                    pl - 2.0 * (ph - p),
                )
            }
            _ => {
                // Standard
                let p = (ph + pl + pc) / 3.0;
                (
                    p,
                    2.0 * p - pl,
                    p + (ph - pl),
                    ph + 2.0 * (p - pl),
                    2.0 * p - ph,
                    p - (ph - pl),
                    pl - 2.0 * (ph - p),
                )
            }
        };

        // Apply to current session's bars
        let curr_end = if sess_idx + 1 < boundaries.len() {
            boundaries[sess_idx + 1]
        } else {
            n
        };
        for i in boundaries[sess_idx]..curr_end {
            pivot[i] = Some(p);
            r1[i] = Some(r1v);
            r2[i] = Some(r2v);
            r3[i] = Some(r3v);
            s1[i] = Some(s1v);
            s2[i] = Some(s2v);
            s3[i] = Some(s3v);
        }
    }

    (pivot, r1, r2, r3, s1, s2, s3)
}
```

- [ ] **Step 2: Create desktop kind**

MainOverlay, 7 horizontal lines per session. Uses same inline-compute approach as VWAP (needs dates from CandleData).
- `ID: "pivot_points"`, `NAME: "Pivot Points"`, `LIKES: 11987`
- Inputs: `[Highs, Lows, Closes]`
- Params: type (Int, 0/1/2, default 0), pivot_color, support_color, resistance_color
- Description: `"Pivot Points. Support and resistance levels calculated from the previous session's high, low, and close. Available in Standard, Fibonacci, and Woodie variants."`
- Rendering: draw horizontal line segments spanning each session for each level. Resistance lines use resistance_color, support lines use support_color, pivot uses pivot_color.

- [ ] **Step 3: Register, build, test, commit**

```bash
git commit -m "feat(indicators): add Pivot Points (Standard, Fibonacci, Woodie)"
```

---

## Task 23: Final Integration Check

- [ ] **Step 1: Run full test suite**

```bash
cd /Users/user/Zaned && cargo test -p zaned-chart-core 2>&1 | tail -20
```

Expected: all chart-core tests pass.

- [ ] **Step 2: Build desktop app**

```bash
cd /Users/user/Zaned && cargo check -p zaned-desktop 2>&1 | tail -20
```

Expected: clean build.

- [ ] **Step 3: Verify registry has all 25 indicators (3 existing + 22 new)**

```bash
cd /Users/user/Zaned && grep -c "IndicatorDef {" apps/desktop/src/ui_components/widgets/charts/indicators/registry.rs
```

Expected: 25

- [ ] **Step 4: Verify all compute functions are exported**

```bash
cd /Users/user/Zaned && grep "pub use" crates/chart-core/src/indicators/mod.rs | wc -l
```

Expected: 22+ lines (3 existing + 19 new compute functions, some indicators like VWAP/Pivot compute inline)

- [ ] **Step 5: Final commit if any loose changes**

```bash
git status && git add -A && git commit -m "feat(indicators): complete indicator expansion — 22 new indicators"
```
