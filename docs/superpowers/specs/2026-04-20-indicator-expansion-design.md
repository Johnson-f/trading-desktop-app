# Indicator Expansion Design

**Date:** 2026-04-20
**Status:** Draft
**Scope:** Add 22 new technical indicators to the charting engine

## Context

The charting engine currently has 3 indicators (EMA, RSI, Volume). To reach feature parity with platforms like TradingView and support all trader types (day, swing, position) across all asset classes (stocks, crypto, futures, forex, options), we need a comprehensive indicator library.

The `ta` crate (v0.5.0, already a dependency) provides 21 indicator implementations. 12 of the 22 new indicators can wrap `ta` structs directly. The remaining 10 require custom compute functions in `chart-core`.

## Architecture — No Breaking Changes Needed

The existing architecture supports all 22 indicators without structural changes:

- **Multi-input indicators** (ATR, Stochastic, etc.): Declare multiple `InputSpec` variants in `inputs()`. The manager already resolves each to a `&[f32]` slice and passes them as `&[&[f32]]` in order. An indicator needing HLC just returns `vec![InputSpec::Highs, InputSpec::Lows, InputSpec::Closes]`.

- **Multi-series output** (MACD, Bollinger, etc.): `ComputedSeries` is already `HashMap<&'static str, Vec<Option<f32>>>`. Indicators insert multiple named series ("macd", "signal", "histogram") and render each in `draw_pane()` / `draw_main()`.

- **Float params** (multipliers, sigma, acceleration): `ParamKind::Float { default, min, max }` already exists. `ParamValues::float()` accessor is available.

- **`ta` crate OHLCV**: Compute functions in `chart-core` receive individual slices and construct `ta::DataItem` structs internally when the `ta` indicator requires OHLCV traits. This keeps the indicator trait boundary clean.

## New Rendering Patterns

Three new visual patterns are needed beyond the existing single-line:

### 1. Band/Channel Overlay (MainOverlay)
Used by: Bollinger Bands, Keltner Channel, Ichimoku Cloud

- Render upper + lower lines with `Shape::line()`
- Optional filled region between bands using `Shape::convex_polygon()` or mesh with alpha
- Middle/base line rendered as standard line

### 2. Histogram + Lines (SubPane)
Used by: MACD, PPO

- Histogram bars: thin filled rects, colored green (positive) / red (negative)
- Signal + MACD lines overlaid on top using standard line rendering
- Y-axis centered on zero

### 3. Dot/Level Overlay (MainOverlay)
Used by: Parabolic SAR, Pivot Points

- Discrete dots rendered with `painter.circle_filled()` at each bar position
- Pivot Points: horizontal lines spanning the session/day at each level

## Indicator Specifications

### Tier 1 — `ta` Crate Wrappers (12 indicators)

#### 1. SMA (Simple Moving Average)
- **Target:** MainOverlay
- **Inputs:** `[Closes]`
- **Params:** period: Int (1–500, default 20), color: Color
- **Series:** `"sma"`
- **Compute:** `ta::SimpleMovingAverage` — already used in VMA, trivial
- **Render:** Single line (same pattern as EMA)
- **Note:** Stub exists at `crates/chart-core/src/indicators/sma.rs`

#### 2. Bollinger Bands
- **Target:** MainOverlay
- **Inputs:** `[Closes]`
- **Params:** period: Int (5–200, default 20), multiplier: Float (0.5–5.0, default 2.0), color: Color
- **Series:** `"upper"`, `"middle"`, `"lower"`
- **Compute:** `ta::BollingerBands` — returns `BollingerBandsOutput { upper, average, lower }`
- **Render:** Band overlay — 3 lines + shaded fill between upper/lower

#### 3. MACD (Moving Average Convergence Divergence)
- **Target:** SubPane
- **Inputs:** `[Closes]`
- **Params:** fast: Int (2–100, default 12), slow: Int (2–100, default 26), signal: Int (2–100, default 9), macd_color: Color, signal_color: Color, histogram_color: Color
- **Series:** `"macd"`, `"signal"`, `"histogram"`
- **Compute:** `ta::MovingAverageConvergenceDivergence` — returns `{ macd, signal, histogram }`
- **Render:** Histogram + lines pattern
- **Note:** Stub exists at `crates/chart-core/src/indicators/macd.rs`

#### 4. Slow Stochastic
- **Target:** SubPane
- **Inputs:** `[Highs, Lows, Closes]`
- **Params:** period: Int (3–100, default 14), k_color: Color, d_color: Color
- **Series:** `"k"`, `"d"`
- **Compute:** `ta::SlowStochastic` — needs `DataItem` (High+Low+Close traits)
- **Render:** Two lines + guide lines at 20/80, range 0–100

#### 5. ATR (Average True Range)
- **Target:** SubPane
- **Inputs:** `[Highs, Lows, Closes]`
- **Params:** period: Int (1–100, default 14), color: Color
- **Series:** `"atr"`
- **Compute:** `ta::AverageTrueRange` — needs `DataItem` (High+Low+Close)
- **Render:** Single line in SubPane, auto-scaled
- **Note:** Stub exists at `crates/chart-core/src/indicators/atr.rs`

#### 6. Keltner Channel
- **Target:** MainOverlay
- **Inputs:** `[Highs, Lows, Closes]`
- **Params:** period: Int (5–200, default 20), multiplier: Float (0.5–5.0, default 2.0), color: Color
- **Series:** `"upper"`, `"middle"`, `"lower"`
- **Compute:** `ta::KeltnerChannel` — returns `KeltnerChannelOutput { upper, average, lower }`
- **Render:** Band overlay (same pattern as Bollinger)

#### 7. Chandelier Exit
- **Target:** MainOverlay
- **Inputs:** `[Highs, Lows, Closes]`
- **Params:** period: Int (5–100, default 22), multiplier: Float (1.0–10.0, default 3.0), long_color: Color, short_color: Color
- **Series:** `"long"`, `"short"`
- **Compute:** `ta::ChandelierExit` — returns `ChandelierExitOutput { long, short }`
- **Render:** Two lines on MainOverlay (long above, short below)

#### 8. CCI (Commodity Channel Index)
- **Target:** SubPane
- **Inputs:** `[Highs, Lows, Closes]`
- **Params:** period: Int (5–200, default 20), color: Color
- **Series:** `"cci"`
- **Compute:** `ta::CommodityChannelIndex` — needs `DataItem`
- **Render:** Single line + guide lines at +100/-100, zero-centered

#### 9. PPO (Percentage Price Oscillator)
- **Target:** SubPane
- **Inputs:** `[Closes]`
- **Params:** fast: Int (2–100, default 12), slow: Int (2–100, default 26), signal: Int (2–100, default 9), ppo_color: Color, signal_color: Color, histogram_color: Color
- **Series:** `"ppo"`, `"signal"`, `"histogram"`
- **Compute:** `ta::PercentagePriceOscillator` — returns `{ ppo, signal, histogram }`
- **Render:** Histogram + lines (same pattern as MACD)

#### 10. MFI (Money Flow Index)
- **Target:** SubPane
- **Inputs:** `[Highs, Lows, Closes, Volumes]`
- **Params:** period: Int (2–100, default 14), color: Color
- **Series:** `"mfi"`
- **Compute:** `ta::MoneyFlowIndex` — needs `DataItem` with Volume
- **Render:** Single line + guide lines at 20/80, range 0–100 (like RSI)

#### 11. OBV (On Balance Volume)
- **Target:** SubPane
- **Inputs:** `[Closes, Volumes]`
- **Params:** color: Color
- **Series:** `"obv"`
- **Compute:** `ta::OnBalanceVolume` — takes Close+Volume
- **Render:** Single line, auto-scaled

#### 12. ROC (Rate of Change)
- **Target:** SubPane
- **Inputs:** `[Closes]`
- **Params:** period: Int (1–200, default 12), color: Color
- **Series:** `"roc"`
- **Compute:** `ta::RateOfChange` — takes `f64`
- **Render:** Single line + zero guide line

### Tier 2 — Custom Implementations (10 indicators)

#### 13. VWAP (Volume Weighted Average Price)
- **Target:** MainOverlay
- **Inputs:** `[Highs, Lows, Closes, Volumes]`
- **Params:** color: Color
- **Series:** `"vwap"`
- **Compute:** Custom — cumulative (typical_price * volume) / cumulative volume, resets each session
- **Render:** Single line on MainOverlay
- **Note:** Session reset logic needed (daily boundary detection from dates)

#### 14. Ichimoku Cloud
- **Target:** MainOverlay
- **Inputs:** `[Highs, Lows, Closes]`
- **Params:** tenkan: Int (1–100, default 9), kijun: Int (1–100, default 26), senkou_b: Int (1–200, default 52), tenkan_color: Color, kijun_color: Color, cloud_up_color: Color, cloud_down_color: Color, chikou_color: Color
- **Series:** `"tenkan"`, `"kijun"`, `"senkou_a"`, `"senkou_b"`, `"chikou"`
- **Compute:** Custom —
  - Tenkan-sen: (highest high + lowest low) / 2 over tenkan period
  - Kijun-sen: (highest high + lowest low) / 2 over kijun period
  - Senkou Span A: (tenkan + kijun) / 2, plotted 26 periods ahead
  - Senkou Span B: (highest high + lowest low) / 2 over senkou_b period, plotted 26 periods ahead
  - Chikou Span: close plotted 26 periods behind
- **Render:** Band overlay for cloud (shaded fill between senkou_a/senkou_b) + 3 individual lines
- **Note:** Series lengths differ due to forward/backward displacement. `ComputedSeries` vectors will have different logical offsets. Rendering must handle displacement.

#### 15. ADX / DMI (Average Directional Index)
- **Target:** SubPane
- **Inputs:** `[Highs, Lows, Closes]`
- **Params:** period: Int (2–100, default 14), adx_color: Color, di_plus_color: Color, di_minus_color: Color
- **Series:** `"adx"`, `"di_plus"`, `"di_minus"`
- **Compute:** Custom —
  - +DM / -DM from consecutive high/low differences
  - Smoothed +DI / -DI as percentage of ATR
  - ADX as smoothed average of |+DI - -DI| / (+DI + -DI) * 100
- **Render:** Three lines + optional guide line at 25 (trend strength threshold)

#### 16. Parabolic SAR
- **Target:** MainOverlay
- **Inputs:** `[Highs, Lows]`
- **Params:** acceleration: Float (0.001–0.1, default 0.02), max_acceleration: Float (0.05–0.5, default 0.2), color: Color
- **Series:** `"sar"`
- **Compute:** Custom — Welles Wilder's parabolic stop-and-reverse algorithm
  - Track EP (extreme point), AF (acceleration factor), trend direction
  - SAR flips when price crosses the SAR level
- **Render:** Dots at each bar position using `painter.circle_filled()`

#### 17. Williams %R
- **Target:** SubPane
- **Inputs:** `[Highs, Lows, Closes]`
- **Params:** period: Int (2–100, default 14), color: Color
- **Series:** `"williams_r"`
- **Compute:** Custom — (highest_high - close) / (highest_high - lowest_low) * -100
  - Rolling window of period bars for highest/lowest
- **Render:** Single line + guide lines at -20/-80, range -100 to 0

#### 18. Supertrend
- **Target:** MainOverlay
- **Inputs:** `[Highs, Lows, Closes]`
- **Params:** period: Int (1–100, default 10), multiplier: Float (0.5–10.0, default 3.0), up_color: Color, down_color: Color
- **Series:** `"supertrend"`, `"direction"` (1.0 = up, -1.0 = down)
- **Compute:** Custom —
  - ATR-based bands: upper = hl2 + multiplier * ATR, lower = hl2 - multiplier * ATR
  - Band locking: lower band never decreases in uptrend, upper never increases in downtrend
  - Direction flips when close crosses the active band
- **Render:** Single line that changes color based on direction series

#### 19. Pivot Points
- **Target:** MainOverlay
- **Inputs:** `[Highs, Lows, Closes]`
- **Params:** type: Int (0=Standard, 1=Fibonacci, 2=Woodie; default 0), pivot_color: Color, support_color: Color, resistance_color: Color
- **Series:** `"pivot"`, `"r1"`, `"r2"`, `"r3"`, `"s1"`, `"s2"`, `"s3"`
- **Compute:** Custom —
  - Standard: P=(H+L+C)/3, R1=2P-L, S1=2P-H, R2=P+(H-L), S2=P-(H-L), R3=H+2(P-L), S3=L-2(H-P)
  - Fibonacci: P=(H+L+C)/3, levels at P +/- (H-L) * fib ratios (0.382, 0.618, 1.0)
  - Woodie: P=(H+L+2C)/4, then standard formulas with Woodie pivot
  - Computed from previous session's HLC
- **Render:** Horizontal lines spanning the current session at each level
- **Note:** Requires session boundary detection (same as VWAP)

#### 20. ALMA (Arnaud Legoux Moving Average)
- **Target:** MainOverlay
- **Inputs:** `[Closes]`
- **Params:** period: Int (2–200, default 9), offset: Float (0.0–1.0, default 0.85), sigma: Float (1.0–20.0, default 6.0), color: Color
- **Series:** `"alma"`
- **Compute:** Custom — Gaussian-weighted moving average
  - m = offset * (period - 1), s = period / sigma
  - weight[i] = exp(-((i - m)^2) / (2 * s^2))
  - Normalize weights, apply to window
- **Render:** Single line (same pattern as EMA/SMA)

#### 21. WMA (Weighted Moving Average)
- **Target:** MainOverlay
- **Inputs:** `[Closes]`
- **Params:** period: Int (1–500, default 20), color: Color
- **Series:** `"wma"`
- **Compute:** Custom — linearly weighted: weight[i] = i+1, most recent gets highest weight
- **Render:** Single line (same pattern as EMA/SMA)

#### 22. Hull Moving Average
- **Target:** MainOverlay
- **Inputs:** `[Closes]`
- **Params:** period: Int (2–500, default 9), color: Color
- **Series:** `"hma"`
- **Compute:** Custom —
  - WMA1 = WMA(closes, period/2)
  - WMA2 = WMA(closes, period)
  - diff = 2 * WMA1 - WMA2
  - HMA = WMA(diff, sqrt(period))
- **Render:** Single line (same pattern as EMA/SMA)

## Implementation Phases

### Phase 1 — Moving Averages (4 indicators)
SMA, WMA, HMA, ALMA

All are MainOverlay single-line indicators taking Closes. Same rendering pattern as EMA. Validates that the pattern works at scale. SMA and ALMA have stubs.

### Phase 2 — Band/Channel Overlays (2 indicators)
Bollinger Bands, Keltner Channel

Introduces multi-series MainOverlay rendering with band fill. Both produce upper/middle/lower. Once the band rendering pattern is built for Bollinger, Keltner reuses it.

### Phase 3 — Histogram SubPane Indicators (2 indicators)
MACD, PPO

Introduces histogram + multi-line SubPane rendering. PPO is structurally identical to MACD. Both use `ta` crate directly.

### Phase 4 — Oscillators (4 indicators)
Stochastic, CCI, Williams %R, ROC

SubPane indicators with guide lines. Stochastic introduces `DataItem` construction in chart-core. Williams %R is custom but similar in shape. CCI and ROC are single-line.

### Phase 5 — Volume-Based (3 indicators)
MFI, OBV, VWAP

MFI and OBV wrap `ta` crate. VWAP is custom and introduces session-boundary detection logic (shared with Pivot Points later).

### Phase 6 — Volatility & Trend (3 indicators)
ATR, ADX/DMI, Supertrend

ATR wraps `ta`. ADX and Supertrend are custom with moderate complexity. Supertrend introduces direction-based color switching.

### Phase 7 — Complex Overlays (4 indicators)
Ichimoku Cloud, Chandelier Exit, Parabolic SAR, Pivot Points

Most complex phase. Ichimoku has displaced series and cloud fill. Parabolic SAR introduces dot rendering. Pivot Points reuses session boundaries from Phase 5. Chandelier wraps `ta`.

## Shared Utilities to Build

1. **`DataItem` construction helper** in chart-core: `fn build_data_items(highs: &[f32], lows: &[f32], closes: &[f32], volumes: Option<&[f32]>) -> Vec<DataItem>` — used by Stochastic, ATR, Keltner, CCI, MFI, Chandelier
2. **Session boundary detection**: `fn session_boundaries(dates: &[String]) -> Vec<usize>` — used by VWAP and Pivot Points
3. **Rolling window min/max**: `fn rolling_high(highs: &[f32], period: usize) -> Vec<Option<f32>>` and `rolling_low` — used by Ichimoku, Williams %R, Stochastic (custom variant)
4. **WMA compute**: `fn compute_wma(data: &[f32], period: usize) -> Vec<Option<f32>>` — used by WMA directly and as building block for HMA

## File Structure

```
crates/chart-core/src/indicators/
  mod.rs              (update: export all new compute_* functions)
  ema.rs              (existing)
  rsi.rs              (existing)
  vma.rs              (existing)
  sma.rs              (existing stub → implement)
  macd.rs             (existing stub → implement)
  atr.rs              (existing stub → implement)
  bollinger.rs        (new)
  stochastic.rs       (new)
  keltner.rs          (new)
  chandelier.rs       (new)
  cci.rs              (new)
  ppo.rs              (new)
  mfi.rs              (new)
  obv.rs              (new)
  roc.rs              (new)
  vwap.rs             (new)
  ichimoku.rs         (new)
  adx.rs              (new)
  parabolic_sar.rs    (new)
  williams_r.rs       (new)
  supertrend.rs       (new)
  pivot_points.rs     (new)
  alma.rs             (new)
  wma.rs              (new)
  hma.rs              (new)
  util.rs             (new — DataItem builder, session boundaries, rolling min/max, WMA)

apps/desktop/src/ui_components/widgets/charts/indicators/kinds/
  mod.rs              (update: export all new modules)
  ema.rs              (existing)
  rsi.rs              (existing)
  volume.rs           (existing)
  sma.rs              (new)
  bollinger.rs        (new)
  macd.rs             (new)
  stochastic.rs       (new)
  atr.rs              (new)
  keltner.rs          (new)
  chandelier.rs       (new)
  cci.rs              (new)
  ppo.rs              (new)
  mfi.rs              (new)
  obv.rs              (new)
  roc.rs              (new)
  vwap.rs             (new)
  ichimoku.rs         (new)
  adx.rs              (new)
  parabolic_sar.rs    (new)
  williams_r.rs       (new)
  supertrend.rs       (new)
  pivot_points.rs     (new)
  alma.rs             (new)
  wma.rs              (new)
  hma.rs              (new)
```

## Registry Updates

Each new indicator gets an `IndicatorDef` entry in `registry.rs` with:
- Unique `id` (lowercase snake_case)
- `name` (display name)
- `short_name` (toolbar abbreviation, 2-5 chars)
- `target` (MainOverlay or SubPane)
- `params` (static SCHEMA)
- `likes` (popularity score for ordering in the modal)
- `description` (1-2 sentence explanation for the indicator modal)
- `factory` function pointer

## What This Spec Does NOT Cover

- Drawing persistence (separate workstream)
- Price alerts on indicator levels
- Intraday timeframes (1m, 5m, etc.)
- Measurement tool
- Multi-chart layout
- Indicator-on-indicator (e.g. RSI of MACD)
