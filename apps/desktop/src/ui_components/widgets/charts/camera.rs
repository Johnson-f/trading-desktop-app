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
            x_scale: 1.0,
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
