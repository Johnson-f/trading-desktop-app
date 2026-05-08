use super::camera::Camera;
use super::candle::{CandleData, candle_instance_desc, create_candle_buffer};
use egui_wgpu::CallbackTrait;
use std::collections::HashMap;
use std::sync::Arc;

/// Per-pane GPU resources, keyed by chart id. egui_wgpu stores callback
/// resources by type, so without this map every pane would share the same
/// camera/vertex buffers and the last-prepared pane's state would visually
/// drive every pane.
#[derive(Default)]
pub struct ChartResourcesMap(pub HashMap<u64, ChartResources>);

pub struct ChartResources {
    pub pipeline: wgpu::RenderPipeline,
    pub candle_buffer: wgpu::Buffer,
    pub camera_buffer: wgpu::Buffer,
    pub camera_bind_group: wgpu::BindGroup,
    pub num_candles: u32,
    /// Tracks which `CandleData` produced the current candle_buffer. When
    /// `ChartCallback` hands over a different Arc (bucket size changed), the
    /// buffer is rebuilt in `prepare`.
    pub data: Arc<CandleData>,
}

impl ChartResources {
    pub fn new(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        data: Arc<CandleData>,
        camera: &Camera,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("candle_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/candle.wgsl").into()),
        });

        let camera_buffer = camera.create_buffer(device);

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

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("candle_pipeline_layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("candle_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[candle_instance_desc()],
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
            multiview_mask: None,
            cache: None,
        });

        let candle_buffer = create_candle_buffer(&data, device);
        let num_candles = data.len() as u32;

        Self {
            pipeline,
            candle_buffer,
            camera_buffer,
            camera_bind_group,
            num_candles,
            data,
        }
    }

    /// Replace the candle buffer when the active `CandleData` changes (e.g.
    /// bucket size flipped). Keeps pipeline/camera resources intact.
    pub fn rebuild_candle_buffer(&mut self, device: &wgpu::Device, data: Arc<CandleData>) {
        self.candle_buffer = create_candle_buffer(&data, device);
        self.num_candles = data.len() as u32;
        self.data = data;
    }
}

pub struct ChartCallback {
    pub id: u64,
    pub camera: Arc<egui::mutex::Mutex<Camera>>,
    pub data: Arc<CandleData>,
    pub target_format: wgpu::TextureFormat,
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
        if resources.get::<ChartResourcesMap>().is_none() {
            resources.insert(ChartResourcesMap::default());
        }
        let map = resources
            .get_mut::<ChartResourcesMap>()
            .expect("inserted above");
        match map.0.entry(self.id) {
            std::collections::hash_map::Entry::Vacant(v) => {
                let camera = self.camera.lock();
                let res =
                    ChartResources::new(device, self.target_format, self.data.clone(), &camera);
                v.insert(res);
            }
            std::collections::hash_map::Entry::Occupied(mut o) => {
                let res = o.get_mut();
                if !Arc::ptr_eq(&res.data, &self.data) {
                    res.rebuild_candle_buffer(device, self.data.clone());
                }
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
        let Some(map) = resources.get::<ChartResourcesMap>() else {
            return;
        };
        let Some(res) = map.0.get(&self.id) else {
            return;
        };
        // Empty data ⇒ zero-sized vertex buffer; `slice(..)` panics on
        // those, so just skip the candle pass and let the egui overlay
        // (axis chrome, watermark) render alone.
        if res.num_candles == 0 {
            return;
        }
        render_pass.set_pipeline(&res.pipeline);
        render_pass.set_bind_group(0, &res.camera_bind_group, &[]);
        render_pass.set_vertex_buffer(0, res.candle_buffer.slice(..));
        render_pass.draw(0..18, 0..res.num_candles);
    }
}
