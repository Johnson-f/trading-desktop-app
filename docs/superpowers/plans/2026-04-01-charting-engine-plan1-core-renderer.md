# Charting Engine Plan 1: Core Renderer + Static Candles

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Render AAPL candlestick data on the GPU using a custom wgpu pipeline embedded inside the egui desktop app, with basic pan/zoom interaction.

**Architecture:** A `ChartWidget` in `apps/desktop/src/ui_components/widgets/charts/` uses egui's `PaintCallback` to inject custom wgpu draw calls. Candle OHLCV data is uploaded to a GPU instance buffer once. A camera uniform controls pan/zoom. The candle vertex shader computes body + wick geometry from instance data. This plan covers: dependencies, data types, shader, renderer, camera with auto-scale, basic scroll-to-zoom and drag-to-pan, and wiring into the app.

**Tech Stack:** Rust, egui 0.34, eframe 0.34, wgpu (via egui-wgpu 0.34), bytemuck, WGSL shaders

**Spec:** `docs/superpowers/specs/2026-04-01-charting-engine-design.md`

**Subsequent plans:** Plan 2 (grid + axis labels), Plan 3 (crosshair + indicators), Plan 4 (volume + hollow candles)

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `apps/desktop/Cargo.toml` | Modify | Add wgpu, egui-wgpu, bytemuck, serde, serde_json, chrono deps |
| `apps/desktop/src/ui_components/mod.rs` | Create (overwrite empty) | Module declaration for widgets |
| `apps/desktop/src/ui_components/widgets/mod.rs` | Create (overwrite empty) | Module declaration for charts |
| `apps/desktop/src/ui_components/widgets/charts/mod.rs` | Create (overwrite empty) | ChartWidget struct, `show()`, PaintCallback registration |
| `apps/desktop/src/ui_components/widgets/charts/candle.rs` | Create | CandleInstance GPU struct, buffer creation from data |
| `apps/desktop/src/ui_components/widgets/charts/camera.rs` | Create | Camera struct, auto-scale, uniform buffer |
| `apps/desktop/src/ui_components/widgets/charts/renderer.rs` | Create | wgpu pipeline creation, CallbackTrait impl |
| `apps/desktop/src/ui_components/widgets/charts/interaction.rs` | Create | Pan/zoom input handling |
| `apps/desktop/src/ui_components/widgets/charts/shaders/candle.wgsl` | Create | Candlestick vertex + fragment shader |
| `apps/desktop/src/main.rs` | Modify | Add ui_components mod, load AAPL.json, wire ChartWidget |
| `apps/desktop/src/components/widgets_control.rs` | Modify | Accept and render ChartWidget when Chart tab active |

---

### Task 1: Add Dependencies

**Files:**
- Modify: `apps/desktop/Cargo.toml`

- [ ] **Step 1: Add wgpu, egui-wgpu, bytemuck, serde, serde_json, chrono to Cargo.toml**

Add these lines under `[dependencies]` in `apps/desktop/Cargo.toml`:

```toml
wgpu = "24"
egui-wgpu = "0.34"
bytemuck = { version = "1", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = "0.4"
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check --manifest-path /Users/user/Zaned/apps/desktop/Cargo.toml`
Expected: Compiles (new deps downloaded, warnings OK).

- [ ] **Step 3: Commit**

```bash
git add apps/desktop/Cargo.toml Cargo.lock
git commit -m "chore(desktop): add wgpu, egui-wgpu, bytemuck, serde, chrono deps for charting engine"
```

---

### Task 2: Define CandleInstance GPU Data Type

**Files:**
- Create: `apps/desktop/src/ui_components/widgets/charts/candle.rs`

- [ ] **Step 1: Create candle.rs with CandleInstance struct and buffer creation**

```rust
use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct CandleInstance {
    pub index: f32,
    pub open: f32,
    pub high: f32,
    pub low: f32,
    pub close: f32,
    pub volume: f32,
}

impl CandleInstance {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<CandleInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // index
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32,
                },
                // open
                wgpu::VertexAttribute {
                    offset: 4,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32,
                },
                // high
                wgpu::VertexAttribute {
                    offset: 8,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32,
                },
                // low
                wgpu::VertexAttribute {
                    offset: 12,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32,
                },
                // close
                wgpu::VertexAttribute {
                    offset: 16,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32,
                },
                // volume
                wgpu::VertexAttribute {
                    offset: 20,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32,
                },
            ],
        }
    }
}

/// JSON shape from AAPL.json
#[derive(serde::Deserialize)]
pub struct JsonCandle {
    pub date: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: i64,
}

pub struct CandleData {
    pub instances: Vec<CandleInstance>,
    pub dates: Vec<String>,
    pub price_min: f32,
    pub price_max: f32,
}

impl CandleData {
    pub fn from_json(json_candles: &[JsonCandle]) -> Self {
        let mut price_min = f32::MAX;
        let mut price_max = f32::MIN;

        let instances: Vec<CandleInstance> = json_candles
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let high = c.high as f32;
                let low = c.low as f32;
                if high > price_max { price_max = high; }
                if low < price_min { price_min = low; }
                CandleInstance {
                    index: i as f32,
                    open: c.open as f32,
                    high,
                    low,
                    close: c.close as f32,
                    volume: c.volume as f32,
                }
            })
            .collect();

        let dates = json_candles.iter().map(|c| c.date.clone()).collect();

        Self { instances, dates, price_min, price_max }
    }

    pub fn create_buffer(&self, device: &wgpu::Device) -> wgpu::Buffer {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("candle_instance_buffer"),
            contents: bytemuck::cast_slice(&self.instances),
            usage: wgpu::BufferUsages::VERTEX,
        })
    }

    pub fn len(&self) -> usize {
        self.instances.len()
    }
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check --manifest-path /Users/user/Zaned/apps/desktop/Cargo.toml`
Expected: Warning about unused module (not wired up yet), but no errors.

Note: This file won't compile standalone — it needs to be part of the module tree. We'll wire it up in Task 5.

- [ ] **Step 3: Commit**

```bash
git add apps/desktop/src/ui_components/widgets/charts/candle.rs
git commit -m "feat(charting): add CandleInstance GPU data type and JSON loader"
```

---

### Task 3: Define Camera Uniform and Auto-Scale

**Files:**
- Create: `apps/desktop/src/ui_components/widgets/charts/camera.rs`

- [ ] **Step 1: Create camera.rs with Camera struct, uniform, and auto-scale**

```rust
use bytemuck::{Pod, Zeroable};
use egui::Vec2;
use wgpu::util::DeviceExt;

use super::candle::CandleData;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct CameraUniform {
    pub x_offset: f32,
    pub x_scale: f32,
    pub y_offset: f32,
    pub y_scale: f32,
    pub width: f32,
    pub height: f32,
    pub _pad: [f32; 2],
}

pub struct Camera {
    pub x_offset: f64,
    pub x_scale: f64,
    pub y_offset: f64,
    pub y_scale: f64,
    pub auto_scale_y: bool,
    pub viewport: Vec2,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            x_offset: 0.0,
            x_scale: 8.0,  // 8 pixels per candle initially
            y_offset: 0.0,
            y_scale: 1.0,
            auto_scale_y: true,
            viewport: Vec2::new(800.0, 600.0),
        }
    }
}

impl Camera {
    /// Fit camera to show the last N candles
    pub fn fit_to_data(&mut self, data: &CandleData) {
        let total = data.len() as f64;
        let visible = (self.viewport.x as f64 / self.x_scale).min(total);
        self.x_offset = (total - visible).max(0.0);
        self.auto_scale_y(data);
    }

    /// Auto-scale Y to fit visible candles
    pub fn auto_scale_y(&mut self, data: &CandleData) {
        if !self.auto_scale_y || data.len() == 0 {
            return;
        }

        let start = (self.x_offset as usize).max(0);
        let visible_count = (self.viewport.x as f64 / self.x_scale).ceil() as usize;
        let end = (start + visible_count + 1).min(data.len());

        if start >= end {
            return;
        }

        let mut min_price = f32::MAX;
        let mut max_price = f32::MIN;
        for candle in &data.instances[start..end] {
            if candle.low < min_price { min_price = candle.low; }
            if candle.high > max_price { max_price = candle.high; }
        }

        let range = (max_price - min_price) as f64;
        let padding = range * 0.05;
        self.y_offset = min_price as f64 - padding;
        let total_range = range + padding * 2.0;
        if total_range > 0.0 {
            self.y_scale = self.viewport.y as f64 / total_range;
        }
    }

    pub fn to_uniform(&self) -> CameraUniform {
        CameraUniform {
            x_offset: self.x_offset as f32,
            x_scale: self.x_scale as f32,
            y_offset: self.y_offset as f32,
            y_scale: self.y_scale as f32,
            width: self.viewport.x,
            height: self.viewport.y,
            _pad: [0.0; 2],
        }
    }

    pub fn create_buffer(&self, device: &wgpu::Device) -> wgpu::Buffer {
        let uniform = self.to_uniform();
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("camera_uniform_buffer"),
            contents: bytemuck::bytes_of(&uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        })
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add apps/desktop/src/ui_components/widgets/charts/camera.rs
git commit -m "feat(charting): add Camera with auto-scale and GPU uniform"
```

---

### Task 4: Write the Candle WGSL Shader

**Files:**
- Create: `apps/desktop/src/ui_components/widgets/charts/shaders/candle.wgsl`

- [ ] **Step 1: Create the shaders directory and candle.wgsl**

```bash
mkdir -p /Users/user/Zaned/apps/desktop/src/ui_components/widgets/charts/shaders
```

- [ ] **Step 2: Write candle.wgsl**

```wgsl
// Camera uniform: transforms candle-index/price space → clip space
struct Camera {
    x_offset: f32,
    x_scale: f32,
    y_offset: f32,
    y_scale: f32,
    width: f32,
    height: f32,
    _pad: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

struct VertexInput {
    // Per-vertex: unit quad corner (0..1 for body, special values for wicks)
    @builtin(vertex_index) vertex_index: u32,
    // Per-instance: candle data
    @location(0) candle_index: f32,
    @location(1) open: f32,
    @location(2) high: f32,
    @location(3) low: f32,
    @location(4) close: f32,
    @location(5) volume: f32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec3<f32>,
};

// Convert from data space (candle index, price) to clip space (-1..1)
fn to_clip(x_data: f32, y_data: f32) -> vec2<f32> {
    let x_pixel = (x_data - camera.x_offset) * camera.x_scale;
    let y_pixel = (y_data - camera.y_offset) * camera.y_scale;
    let x_clip = (x_pixel / camera.width) * 2.0 - 1.0;
    let y_clip = (y_pixel / camera.height) * 2.0 - 1.0;
    return vec2<f32>(x_clip, y_clip);
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let is_up = in.close >= in.open;
    let body_top = max(in.open, in.close);
    let body_bottom = min(in.open, in.close);

    // Colors: green for up, red for down
    if is_up {
        out.color = vec3<f32>(0.306, 0.804, 0.769); // (78, 205, 196) / 255
    } else {
        out.color = vec3<f32>(1.0, 0.420, 0.420);   // (255, 107, 107) / 255
    }

    // Candle width: 70% of x_scale (leaving 30% gap between candles)
    let candle_half_width = camera.x_scale * 0.35 / camera.width;
    let wick_half_width = camera.x_scale * 0.04 / camera.width;

    let center_x = in.candle_index;

    // 12 vertices per candle: 6 for body (2 triangles), 6 for wick (2 thin rects)
    // Vertices 0-5: body quad
    // Vertices 6-8: upper wick
    // Vertices 9-11: lower wick
    let vi = in.vertex_index;

    var pos: vec2<f32>;

    if vi < 6u {
        // Body quad (2 triangles)
        let body_positions = array<vec2<f32>, 6>(
            vec2<f32>(-1.0, 0.0), // bottom-left
            vec2<f32>( 1.0, 0.0), // bottom-right
            vec2<f32>( 1.0, 1.0), // top-right
            vec2<f32>(-1.0, 0.0), // bottom-left
            vec2<f32>( 1.0, 1.0), // top-right
            vec2<f32>(-1.0, 1.0), // top-left
        );
        let corner = body_positions[vi];
        let x_data = center_x + corner.x * 0.35;
        let y_data = body_bottom + corner.y * (body_top - body_bottom);
        pos = to_clip(x_data, y_data);
    } else if vi < 9u {
        // Upper wick (thin rect from body_top to high)
        let wick_positions = array<vec2<f32>, 3>(
            vec2<f32>(-1.0, 0.0),
            vec2<f32>( 1.0, 0.0),
            vec2<f32>( 1.0, 1.0),
        );
        let corner = wick_positions[vi - 6u];
        let x_data = center_x + corner.x * 0.04;
        let y_data = body_top + corner.y * (in.high - body_top);
        pos = to_clip(x_data, y_data);
    } else {
        // Lower wick (thin rect from low to body_bottom)
        let wick_positions = array<vec2<f32>, 3>(
            vec2<f32>(-1.0, 0.0),
            vec2<f32>( 1.0, 1.0),
            vec2<f32>(-1.0, 1.0),
        );
        let corner = wick_positions[vi - 9u];
        let x_data = center_x + corner.x * 0.04;
        let y_data = in.low + corner.y * (body_bottom - in.low);
        pos = to_clip(x_data, y_data);
    }

    out.position = vec4<f32>(pos.x, pos.y, 0.0, 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color, 1.0);
}
```

- [ ] **Step 3: Commit**

```bash
git add apps/desktop/src/ui_components/widgets/charts/shaders/candle.wgsl
git commit -m "feat(charting): add candlestick WGSL shader with body + wick rendering"
```

---

### Task 5: Implement the wgpu Renderer (CallbackTrait)

**Files:**
- Create: `apps/desktop/src/ui_components/widgets/charts/renderer.rs`

- [ ] **Step 1: Create renderer.rs with pipeline setup and CallbackTrait impl**

```rust
use std::sync::Arc;
use egui::mutex::Mutex;
use egui_wgpu::CallbackTrait;
use wgpu::util::DeviceExt;

use super::camera::{Camera, CameraUniform};
use super::candle::{CandleData, CandleInstance};

pub struct ChartResources {
    pub pipeline: wgpu::RenderPipeline,
    pub candle_buffer: wgpu::Buffer,
    pub camera_buffer: wgpu::Buffer,
    pub camera_bind_group: wgpu::BindGroup,
    pub num_candles: u32,
}

impl ChartResources {
    pub fn new(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        data: &CandleData,
        camera: &Camera,
    ) -> Self {
        // Shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("candle_shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shaders/candle.wgsl").into(),
            ),
        });

        // Camera uniform buffer
        let camera_buffer = camera.create_buffer(device);

        // Camera bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("camera_bind_group_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera_bind_group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        // Pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("candle_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Render pipeline
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("candle_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[CandleInstance::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Candle instance buffer
        let candle_buffer = data.create_buffer(device);

        Self {
            pipeline,
            candle_buffer,
            camera_buffer,
            camera_bind_group,
            num_candles: data.len() as u32,
        }
    }
}

pub struct ChartCallback {
    pub camera: Arc<Mutex<Camera>>,
    pub data: Arc<CandleData>,
    pub initialized: Arc<Mutex<bool>>,
}

impl CallbackTrait for ChartCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let mut initialized = self.initialized.lock();

        if !*initialized {
            let camera = self.camera.lock();
            // Detect texture format from the render state
            let format = wgpu::TextureFormat::Bgra8UnormSrgb;
            let chart_resources = ChartResources::new(device, format, &self.data, &camera);
            resources.insert(chart_resources);
            *initialized = true;
        } else {
            // Update camera uniform
            if let Some(res) = resources.get::<ChartResources>() {
                let camera = self.camera.lock();
                let uniform = camera.to_uniform();
                queue.write_buffer(&res.camera_buffer, 0, bytemuck::bytes_of(&uniform));
            }
        }

        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        if let Some(res) = resources.get::<ChartResources>() {
            render_pass.set_pipeline(&res.pipeline);
            render_pass.set_bind_group(0, &res.camera_bind_group, &[]);
            render_pass.set_vertex_buffer(0, res.candle_buffer.slice(..));
            // 12 vertices per candle (body: 6, upper wick: 3, lower wick: 3)
            render_pass.draw(0..12, 0..res.num_candles);
        }
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add apps/desktop/src/ui_components/widgets/charts/renderer.rs
git commit -m "feat(charting): add wgpu renderer with CallbackTrait for candle pipeline"
```

---

### Task 6: Implement Basic Pan/Zoom Interaction

**Files:**
- Create: `apps/desktop/src/ui_components/widgets/charts/interaction.rs`

- [ ] **Step 1: Create interaction.rs**

```rust
use egui::{Pos2, Rect, Vec2};

use super::camera::Camera;
use super::candle::CandleData;

pub struct InteractionState {
    pub is_dragging: bool,
    pub drag_start: Pos2,
    pub last_drag_pos: Pos2,
}

impl Default for InteractionState {
    fn default() -> Self {
        Self {
            is_dragging: false,
            drag_start: Pos2::ZERO,
            last_drag_pos: Pos2::ZERO,
        }
    }
}

impl InteractionState {
    pub fn handle_input(
        &mut self,
        ui: &egui::Ui,
        chart_rect: Rect,
        camera: &mut Camera,
        data: &CandleData,
    ) {
        let response = ui.interact(chart_rect, ui.id().with("chart_interaction"), egui::Sense::click_and_drag());

        // Drag to pan (X only)
        if response.dragged() {
            let delta = response.drag_delta();
            // Convert pixel delta to candle-index delta
            let dx_candles = delta.x as f64 / camera.x_scale;
            camera.x_offset -= dx_candles;

            // Clamp so we don't scroll past the data
            let max_offset = (data.len() as f64 - 1.0).max(0.0);
            camera.x_offset = camera.x_offset.clamp(0.0, max_offset);

            // Auto-scale Y after panning
            camera.auto_scale_y(data);
        }

        // Scroll to zoom (X axis, centered on cursor)
        let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll_delta != 0.0 && chart_rect.contains(ui.input(|i| i.pointer.latest_pos().unwrap_or(Pos2::ZERO))) {
            let zoom_factor = if scroll_delta > 0.0 { 1.1 } else { 1.0 / 1.1 };

            // Get cursor position in candle-index space
            if let Some(cursor_pos) = ui.input(|i| i.pointer.latest_pos()) {
                let cursor_x_pixel = cursor_pos.x - chart_rect.left();
                let cursor_index = camera.x_offset + cursor_x_pixel as f64 / camera.x_scale;

                // Apply zoom
                camera.x_scale *= zoom_factor;
                camera.x_scale = camera.x_scale.clamp(2.0, 50.0); // min 2px, max 50px per candle

                // Adjust offset so cursor position stays in place
                camera.x_offset = cursor_index - cursor_x_pixel as f64 / camera.x_scale;

                let max_offset = (data.len() as f64 - 1.0).max(0.0);
                camera.x_offset = camera.x_offset.clamp(0.0, max_offset);

                camera.auto_scale_y(data);
            }
        }
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add apps/desktop/src/ui_components/widgets/charts/interaction.rs
git commit -m "feat(charting): add pan/zoom interaction with cursor-centered zoom"
```

---

### Task 7: Create ChartWidget and Wire Module Tree

**Files:**
- Create: `apps/desktop/src/ui_components/widgets/charts/mod.rs`
- Create: `apps/desktop/src/ui_components/widgets/mod.rs`
- Create: `apps/desktop/src/ui_components/mod.rs`

- [ ] **Step 1: Write charts/mod.rs — the ChartWidget**

```rust
mod camera;
mod candle;
mod interaction;
mod renderer;

use std::sync::Arc;
use egui::mutex::Mutex;

pub use candle::{CandleData, JsonCandle};
use camera::Camera;
use interaction::InteractionState;
use renderer::ChartCallback;

pub struct ChartWidget {
    data: Arc<CandleData>,
    camera: Arc<Mutex<Camera>>,
    interaction: InteractionState,
    initialized: Arc<Mutex<bool>>,
}

impl ChartWidget {
    pub fn new(data: CandleData) -> Self {
        let mut camera = Camera::default();
        let data = Arc::new(data);
        camera.fit_to_data(&data);

        Self {
            data,
            camera: Arc::new(Mutex::new(camera)),
            interaction: InteractionState::default(),
            initialized: Arc::new(Mutex::new(false)),
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        let available = ui.available_size();
        let (rect, _response) = ui.allocate_exact_size(available, egui::Sense::hover());

        // Update viewport
        {
            let mut camera = self.camera.lock();
            camera.viewport = rect.size();
        }

        // Handle interaction
        {
            let mut camera = self.camera.lock();
            self.interaction.handle_input(ui, rect, &mut camera, &self.data);
        }

        // Register wgpu paint callback
        let callback = ChartCallback {
            camera: self.camera.clone(),
            data: self.data.clone(),
            initialized: self.initialized.clone(),
        };

        ui.painter().add(egui::PaintCallback {
            rect,
            callback: Arc::new(egui_wgpu::Callback::new_paint_callback(rect, callback)),
        });
    }
}
```

- [ ] **Step 2: Write widgets/mod.rs**

```rust
pub mod charts;
```

- [ ] **Step 3: Write ui_components/mod.rs**

```rust
pub mod widgets;
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo check --manifest-path /Users/user/Zaned/apps/desktop/Cargo.toml`
Expected: May have warnings about unused ui_components module in main.rs. Fix any compile errors.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src/ui_components/
git commit -m "feat(charting): add ChartWidget with wgpu PaintCallback and module tree"
```

---

### Task 8: Wire ChartWidget into the App

**Files:**
- Modify: `apps/desktop/src/main.rs`
- Modify: `apps/desktop/src/components/widgets_control.rs`

- [ ] **Step 1: Add ui_components module and load AAPL data in main.rs**

Add `mod ui_components;` after `mod components;` at the top of `main.rs`.

Update the `MyApp` struct to include the chart widget:

```rust
mod components;
mod ui_components;

use components::{MainSidebar, MiniSidebar, TopHeader, WidgetsControl};
use ui_components::widgets::charts::{ChartWidget, CandleData, JsonCandle};
use eframe::egui;

// ... (main fn stays the same)

#[derive(Default)]
struct MyApp {
    top_header: TopHeader,
    widgets_control: WidgetsControl,
    main_sidebar: MainSidebar,
    mini_sidebar: MiniSidebar,
    chart: Option<ChartWidget>,
}
```

Add data loading in the `eframe::run_native` closure, after setting visuals:

```rust
// Inside the Box::new(|cc| { ... }) closure, after cc.egui_ctx.set_visuals(visuals):
let mut app = MyApp::default();

// Load AAPL.json
let json_str = std::fs::read_to_string("AAPL.json").unwrap_or_default();
if !json_str.is_empty() {
    let candles: Vec<JsonCandle> = serde_json::from_str(&json_str).unwrap_or_default();
    if !candles.is_empty() {
        let data = CandleData::from_json(&candles);
        app.chart = Some(ChartWidget::new(data));
    }
}

Ok(Box::new(app))
```

Remove `Ok(Box::new(MyApp::default()))` since we're creating the app manually now.

- [ ] **Step 2: Update WidgetsControl to accept a mutable ChartWidget reference**

Change `WidgetsControl::show` to accept an optional chart widget. In `widgets_control.rs`, update the `show` method signature:

```rust
pub fn show(&mut self, ui: &mut egui::Ui, chart: Option<&mut ChartWidget>) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;

        // ── Tab Bar ──
        self.paint_tab_bar(ui);

        // ── Toolbar (only show for Chart tab) ──
        if self.active_tab == 0 {
            self.paint_toolbar(ui);
        }

        // ── Content area ──
        if self.active_tab == 0 {
            if let Some(chart) = chart {
                chart.show(ui);
            }
        }
    });
}
```

Add the import at the top of `widgets_control.rs`:

```rust
use crate::ui_components::widgets::charts::ChartWidget;
```

- [ ] **Step 3: Update main.rs central panel to pass chart to widgets_control**

In the `ui` method, change the central panel to:

```rust
egui::CentralPanel::default()
    .frame(egui::Frame::new().fill(bg))
    .show_inside(ui, |ui| {
        self.widgets_control.show(ui, self.chart.as_mut());
    });
```

- [ ] **Step 4: Verify it compiles and runs**

Run: `cargo check --manifest-path /Users/user/Zaned/apps/desktop/Cargo.toml`
Then: `cargo run --manifest-path /Users/user/Zaned/apps/desktop/Cargo.toml`

Expected: App launches with candlestick data visible in the chart area. You can scroll to zoom and drag to pan.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src/main.rs apps/desktop/src/components/widgets_control.rs
git commit -m "feat(charting): wire ChartWidget into app with AAPL.json data loading"
```

---

### Task 9: Visual Verification and Fix-up

**Files:**
- May modify any file from previous tasks

- [ ] **Step 1: Run the app and verify**

Run: `cargo run --manifest-path /Users/user/Zaned/apps/desktop/Cargo.toml`

Check:
- Candles appear in the central panel below the tab bar / toolbar
- Green candles for up days (close > open), red for down days
- Scrolling zooms in/out (candles get wider/narrower)
- Dragging pans left/right through time
- Price axis auto-scales as you pan
- No panics, no GPU errors in console
- Zoom centers on cursor position

- [ ] **Step 2: Fix any issues found**

Common issues to check:
- Shader compilation errors (check console for wgpu errors)
- Candles rendering upside down (Y-axis flip — adjust `to_clip` in shader)
- Candles not visible (camera offset wrong — check `fit_to_data`)
- Zoom too fast/slow (adjust 1.1 factor in interaction.rs)
- PaintCallback rect not matching the available area

- [ ] **Step 3: Final compile check**

Run: `cargo check --manifest-path /Users/user/Zaned/apps/desktop/Cargo.toml`
Expected: Clean compile, minimal warnings.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "fix(charting): visual polish and fix-ups for core renderer"
```
