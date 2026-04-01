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
                wgpu::VertexAttribute { offset: 0, shader_location: 0, format: wgpu::VertexFormat::Float32 },
                wgpu::VertexAttribute { offset: 4, shader_location: 1, format: wgpu::VertexFormat::Float32 },
                wgpu::VertexAttribute { offset: 8, shader_location: 2, format: wgpu::VertexFormat::Float32 },
                wgpu::VertexAttribute { offset: 12, shader_location: 3, format: wgpu::VertexFormat::Float32 },
                wgpu::VertexAttribute { offset: 16, shader_location: 4, format: wgpu::VertexFormat::Float32 },
                wgpu::VertexAttribute { offset: 20, shader_location: 5, format: wgpu::VertexFormat::Float32 },
            ],
        }
    }
}

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
}

impl CandleData {
    pub fn from_json(json_candles: &[JsonCandle]) -> Self {
        let instances: Vec<CandleInstance> = json_candles.iter().enumerate().map(|(i, c)| {
            CandleInstance {
                index: i as f32,
                open: c.open as f32,
                high: c.high as f32,
                low: c.low as f32,
                close: c.close as f32,
                volume: c.volume as f32,
            }
        }).collect();
        let dates = json_candles.iter().map(|c| c.date.clone()).collect();
        Self { instances, dates }
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
