# Charting Engine Design

**Date:** 2026-04-01
**Location:** `apps/desktop/src/ui_components/widgets/charts/`
**Renderer:** Custom wgpu pipeline via egui `PaintCallback`
**Interaction:** TradingView-style pan/zoom

---

## Overview

A GPU-accelerated candlestick charting engine embedded inside the egui desktop app. Renders OHLCV data using instanced wgpu draw calls, supports pan/zoom with auto-scaling price axis, crosshair with tooltip, grid lines, and technical indicators. Handles 10k+ candles at 60fps.

## File Structure

```
apps/desktop/src/ui_components/widgets/charts/
├── mod.rs              — ChartWidget (egui Widget impl), re-exports
├── renderer.rs         — wgpu pipeline setup, CallbackTrait impl, shader loading
├── camera.rs           — 2D projection matrix, pan/zoom state, auto-scale
├── candle.rs           — CandleInstance GPU struct, buffer upload from Vec<Candle>
├── grid.rs             — Price/time grid lines, axis labels (egui text)
├── crosshair.rs        — Mouse-tracking crosshair lines + OHLCV tooltip
├── interaction.rs      — Input handling: chart drag, scroll zoom, axis drag
├── indicators/
│   ├── mod.rs          — Indicator trait, registry, line buffer
│   ├── sma.rs          — Simple moving average
│   ├── ema.rs          — Exponential moving average
│   ├── bollinger.rs    — Bollinger bands (filled region between upper/lower)
│   └── volume.rs       — Volume bars (rendered below main chart)
└── shaders/
    ├── candle.wgsl     — Candlestick vertex + fragment shader
    ├── line.wgsl       — Line rendering (indicators, grid, crosshair)
    └── volume.wgsl     — Volume bar vertex + fragment shader
```

## Dependencies

Add to `apps/desktop/Cargo.toml`:

```toml
wgpu = "24"
egui-wgpu = "0.34"
bytemuck = { version = "1", features = ["derive"] }
serde_json = "1"
```

## Data Model

### Input

Data comes from the `markets` crate `Candle` struct or from `AAPL.json`:

```rust
// From markets crate
pub struct Candle {
    pub timestamp: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: i64,
}
```

The `AAPL.json` file uses `date` (RFC 3339 string) instead of `timestamp`. The chart widget accepts `Vec<Candle>` — parsing is the caller's responsibility.

### GPU Instance Data

```rust
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CandleInstance {
    pub index: f32,       // x-position (candle index, not timestamp)
    pub open: f32,
    pub high: f32,
    pub low: f32,
    pub close: f32,
    pub volume: f32,
}
```

Using `f32` for GPU. The `index` field is the sequential position (0, 1, 2, ...) — the shader uses the camera matrix to map this to screen X. Timestamps are stored CPU-side for axis labels and tooltip display.

### Camera Uniform

```rust
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    pub x_offset: f32,     // horizontal pan (in candle-index space)
    pub x_scale: f32,      // horizontal zoom (pixels per candle)
    pub y_offset: f32,     // vertical pan (in price space)
    pub y_scale: f32,      // vertical zoom (pixels per price unit)
    pub width: f32,        // viewport width in pixels
    pub height: f32,       // viewport height in pixels
    pub _pad: [f32; 2],    // align to 32 bytes
}
```

## Rendering Pipeline

### Initialization (once, on first paint)

1. Create wgpu render pipelines: candle, line, volume
2. Load WGSL shaders from embedded strings (`include_str!`)
3. Create camera uniform buffer
4. Upload `CandleInstance` data to a GPU vertex buffer
5. Store all resources in `egui_wgpu::CallbackResources`

### Per Frame (inside `CallbackTrait::paint`)

1. **Update camera uniform** — write new pan/zoom values to the buffer (64 bytes)
2. **Draw grid** — line pipeline, ~50 instances for price/time grid lines
3. **Draw candles** — candle pipeline, 1 instanced draw call for ALL candles. Vertex shader computes body rect + wick lines from OHLC instance data + camera uniform. Fragment shader colors green (close > open) or red (close < open).
4. **Draw indicators** — line pipeline, 1 draw call per active indicator
5. **Draw volume** — volume pipeline, 1 instanced draw call
6. **Crosshair + labels** — drawn via egui painter (text labels need egui's font system)

**Total draw calls:** 5-8 regardless of candle count.

### Candle Shader Logic (candle.wgsl)

The vertex shader receives a unit quad (4 vertices) and a `CandleInstance`. It computes:

- **Body**: rect from `open` to `close` at x-position `index`, width based on `x_scale`
- **Upper wick**: line from `max(open, close)` to `high`
- **Lower wick**: line from `min(open, close)` to `low`

The camera uniform transforms candle-index space → clip space. The fragment shader checks `close > open` to choose green or red.

For **hollow candles**: a uniform flag switches the fragment shader to render the body as an outline (discard interior fragments for up-candles).

## Camera & Auto-Scale

### State

```rust
pub struct Camera {
    pub x_offset: f64,        // leftmost visible candle index
    pub x_scale: f64,         // pixels per candle
    pub y_offset: f64,        // bottom price
    pub y_scale: f64,         // pixels per price unit
    pub auto_scale_y: bool,   // true by default
    pub viewport: Vec2,       // pixel dimensions of chart area
}
```

### Auto-Scale (default behavior)

When `auto_scale_y` is true, every frame:

1. Determine visible candle range from `x_offset` and viewport width / `x_scale`
2. Find `min(low)` and `max(high)` among visible candles
3. Add 5% padding top and bottom
4. Set `y_offset` and `y_scale` to fit this range

This means the price axis always fits the visible data. Horizontal panning automatically adjusts the price range.

### Manual Scale

When user drags the price axis, set `auto_scale_y = false` and adjust `y_offset` / `y_scale` directly. Double-click price axis re-enables auto-scale.

## Interaction Model (TradingView-style)

### Chart Area

| Input | Action |
|-------|--------|
| Click + drag | Pan time axis (X only). Price auto-scales if enabled. |
| Scroll wheel | Zoom time axis. Zoom centers on cursor X position. |
| Mouse hover | Show crosshair (vertical + horizontal lines) + OHLCV tooltip at cursor position. |

### Price Axis (right edge, ~60px wide)

| Input | Action |
|-------|--------|
| Click + drag up/down | Manual price zoom. Disables auto-scale. |
| Double-click | Reset to auto-scale. |

### Time Axis (bottom edge, ~30px tall)

| Input | Action |
|-------|--------|
| Click + drag left/right | Zoom time axis. |

### Zoom Behavior

Zoom always centers on cursor position. When zooming in:
- Candles to the left of cursor move further left
- Candles to the right of cursor move further right
- The candle under the cursor stays in place

Implementation: `x_offset += cursor_index * (1.0 - zoom_factor)`

## Grid & Axis Labels

### Price Grid (horizontal lines)

- Compute "nice" price intervals (e.g., $1, $5, $10, $50) based on visible price range and viewport height
- Draw faint horizontal lines at each interval
- Labels on the right edge: white text, right-aligned, `12px` mono font
- Current price: highlighted label with accent background

### Time Grid (vertical lines)

- Compute intervals based on zoom level:
  - Zoomed in: show individual dates (Mon, Tue, ...)
  - Medium: show weeks or month starts
  - Zoomed out: show months or years
- Faint vertical lines at each interval
- Labels on the bottom edge: centered text

### Grid Color

Same as existing UI: `Color32::from_rgb(30, 30, 33)` — barely visible, doesn't compete with candles.

## Crosshair

- Thin vertical line (1px) tracking mouse X — snaps to nearest candle center
- Thin horizontal line (1px) tracking mouse Y — shows exact price
- **X label**: date/time shown on time axis at crosshair position
- **Y label**: price shown on price axis at crosshair position
- **OHLCV tooltip**: shown at top-left of chart area: `O: 175.48  H: 180.22  L: 174.10  C: 179.50  V: 11.2M`
- Color: `Color32::from_rgb(80, 80, 90)` — visible but subtle
- Crosshair lines drawn via egui painter (on top of wgpu content)
- Labels drawn via egui painter (needs font rendering)

## Indicators

### Trait

```rust
pub trait Indicator {
    fn name(&self) -> &str;
    fn compute(&self, candles: &[Candle]) -> Vec<f32>;  // one value per candle
    fn color(&self) -> Color32;
}
```

### Rendering

Indicator values are computed CPU-side and uploaded as a line strip vertex buffer. The line shader draws connected line segments transformed by the camera uniform. Each indicator is one draw call.

### Built-in Indicators

| Indicator | Parameters | Rendering |
|-----------|-----------|-----------|
| SMA | period (e.g., 20, 50, 200) | Colored line on main chart |
| EMA | period | Colored line on main chart |
| Bollinger | period, std_dev multiplier | Upper line + lower line + filled region between |
| Volume | none | Bar chart in separate panel below main chart |

Volume is special — it renders in its own sub-panel (bottom 20% of chart area) with its own Y-scale but shared X-axis.

## Volume Sub-Panel

- Occupies bottom ~20% of the chart widget area
- Shares X-axis (time) with the main chart — pan/zoom affects both
- Independent Y-axis (auto-scaled to visible volume range)
- Bars colored green/red matching their candle direction
- Thin horizontal separator line between main chart and volume panel
- Volume labels on right axis (abbreviated: 11.2M, 500K)

## egui Integration

### ChartWidget

```rust
pub struct ChartWidget {
    candles: Vec<Candle>,
    camera: Camera,
    interaction: InteractionState,
    indicators: Vec<Box<dyn Indicator>>,
    style: CandleStyle,          // Classic or Hollow
    show_volume: bool,
    show_crosshair: bool,
    gpu_initialized: bool,
}

impl ChartWidget {
    pub fn new(candles: Vec<Candle>) -> Self;
    pub fn show(&mut self, ui: &mut egui::Ui);
}
```

### How it connects to egui

`ChartWidget::show()` does:

1. Allocate a rect in the egui layout via `ui.allocate_rect()`
2. Handle input (mouse events within the rect) → update `Camera`
3. Register a `PaintCallback` with the allocated rect
4. The callback's `prepare()` uploads/updates GPU buffers
5. The callback's `paint()` issues wgpu draw calls
6. After the callback, use `ui.painter()` for crosshair labels and axis text (egui overlay on top of wgpu content)

### Wiring into the App

In `widgets_control.rs`, when `active_tab == 0` (Chart tab), render `ChartWidget` in the central panel area below the toolbar.

## Candle Styles

### Classic Filled (default)

- Up candle (close > open): body filled green, wicks green
- Down candle (close < open): body filled red, wicks red
- Body is a solid rectangle from open to close

### Hollow

- Up candle: body is an outline (hollow) in green, wicks green
- Down candle: body is filled red, wicks red
- Toggled via toolbar button or `CandleStyle` enum

Implementation: a uniform `u32` flag sent to the fragment shader. When hollow mode is on and the candle is "up", the fragment shader discards pixels that aren't on the body edge.

## Colors

| Element | Color | RGB |
|---------|-------|-----|
| Chart background | Match app bg | `(18, 18, 22)` |
| Candle up (body + wick) | Teal green | `(78, 205, 196)` |
| Candle down (body + wick) | Coral red | `(255, 107, 107)` |
| Grid lines | Subtle dark | `(30, 30, 33)` |
| Crosshair lines | Medium gray | `(80, 80, 90)` |
| Axis labels | Muted text | `(120, 120, 130)` |
| Current price label bg | Accent | `(78, 205, 196)` with black text |
| Volume up bars | Teal green 50% opacity | `(78, 205, 196, 128)` |
| Volume down bars | Coral red 50% opacity | `(255, 107, 107, 128)` |

## Out of Scope

- Drawing tools (lines, rectangles, fibonacci) — future feature
- Multi-chart layouts (split view) — future feature
- Real-time streaming updates — future feature (current: static data load)
- OHLC bar charts (non-candlestick) — future feature
- Heikin-Ashi candles — future feature
- Logarithmic price scale — future feature
- Order/position overlays — future feature
