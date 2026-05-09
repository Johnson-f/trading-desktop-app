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
            // Pixels per candle slot. Body renders at 85% of this width
            // (see candle.wgsl), so 16 → ~13 px body + ~3 px gap, which
            // matches Webull's daily-chart density at a comfortable size
            // (~50 visible bars on a typical chart pane). The wheel-zoom
            // handler scales this freely; it's the boot value only.
            x_scale: 16.0,
            y_offset: 0.0,
            y_scale: 1.0,
            auto_scale_y: true,
            viewport: Vec2::new(800.0, 600.0),
        }
    }
}

impl Camera {
    pub fn fit_to_data(&mut self, data: &CandleData) {
        let total = data.len() as f64;
        let visible = (self.viewport.x as f64 / self.x_scale).min(total);
        self.x_offset = (total - visible).max(0.0);
        self.auto_scale_y(data);
    }

    /// Fit the viewport to the last `n` candles of `data` (or all of them
    /// if `n` exceeds the dataset). Used by the Range bar to show a fixed
    /// trailing window without changing the x_scale (zoom level).
    pub fn fit_to_trailing(&mut self, data: &CandleData, n: usize) {
        if data.is_empty() {
            return;
        }
        let total = data.len();
        let start = total.saturating_sub(n);
        // Compute how many candles actually fit at the current zoom level.
        // We want the last `n` candles centred/right-aligned in the viewport,
        // so we set x_offset to `start` (the first visible bar index).
        let visible = (self.viewport.x as f64 / self.x_scale).min(n as f64);
        self.x_offset = (total as f64 - visible).max(start as f64);
        self.auto_scale_y(data);
    }

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
            if candle.low < min_price {
                min_price = candle.low;
            }
            if candle.high > max_price {
                max_price = candle.high;
            }
        }

        let range = (max_price - min_price) as f64;
        // Asymmetric padding: more headroom at the top so the highest candle
        // wick doesn't reach the OHLC / ticker label band painted at the
        // chart's top-left. Bottom keeps a tighter 5% pad so the chart still
        // sits low and uses the available canvas height.
        let bottom_padding = range * 0.09;
        let top_padding = range * 0.18;
        self.y_offset = min_price as f64 - bottom_padding;
        let total_range = range + bottom_padding + top_padding;
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
