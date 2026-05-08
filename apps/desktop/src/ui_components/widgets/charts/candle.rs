//! Desktop-side candle shim: re-exports the pure types from chart-core and adds
//! wgpu glue (vertex layout + buffer construction) that can't live in the core.

use wgpu::util::DeviceExt;

pub use zaned_chart_core::{CandleData, CandleInstance, JsonCandle, Timeframe};

/// Vertex buffer layout for `CandleInstance` as a per-instance vertex input.
pub fn candle_instance_desc() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<CandleInstance>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &[
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32,
            },
            wgpu::VertexAttribute {
                offset: 4,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32,
            },
            wgpu::VertexAttribute {
                offset: 8,
                shader_location: 2,
                format: wgpu::VertexFormat::Float32,
            },
            wgpu::VertexAttribute {
                offset: 12,
                shader_location: 3,
                format: wgpu::VertexFormat::Float32,
            },
            wgpu::VertexAttribute {
                offset: 16,
                shader_location: 4,
                format: wgpu::VertexFormat::Float32,
            },
            wgpu::VertexAttribute {
                offset: 20,
                shader_location: 5,
                format: wgpu::VertexFormat::Float32,
            },
        ],
    }
}

/// Build the candle vertex buffer from a `CandleData`.
///
/// wgpu refuses to allocate a 0-sized buffer, so when the chart has no
/// candles yet (cold boot, in-flight historical fetch) we materialize
/// a single zeroed instance. The renderer's `paint` skips the draw
/// when `num_candles == 0`, so this padding is never read.
pub fn create_candle_buffer(data: &CandleData, device: &wgpu::Device) -> wgpu::Buffer {
    let placeholder = [CandleInstance {
        index: 0.0,
        open: 0.0,
        high: 0.0,
        low: 0.0,
        close: 0.0,
        volume: 0.0,
    }];
    let bytes: &[u8] = if data.instances.is_empty() {
        bytemuck::cast_slice(&placeholder)
    } else {
        bytemuck::cast_slice(&data.instances)
    };
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("candle_instance_buffer"),
        contents: bytes,
        usage: wgpu::BufferUsages::VERTEX,
    })
}
