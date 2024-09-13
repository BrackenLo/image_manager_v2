//====================================================================

use std::collections::HashMap;

use shipyard::Unique;

use crate::storage::TextureID;

use super::{
    shared::{RawTextureVertex, TEXTURE_INDICES, TEXTURE_VERTICES},
    texture::Texture,
    tools, Vertex,
};

//====================================================================

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Zeroable, bytemuck::Pod, Default)]
pub struct RawGif2dInstance {
    pub pos: [f32; 2],
    pub size: [f32; 2],
    pub color: [f32; 4],
    pub frame: u32,
}

impl Vertex for RawGif2dInstance {
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        const VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
            2 => Float32x2,
            3 => Float32x2,
            4 => Float32x4,
            5 => Uint32,
        ];

        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<RawGif2dInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &VERTEX_ATTRIBUTES,
        }
    }
}

pub struct Gif2dInstance {
    instance_buffer: wgpu::Buffer,
    instance_count: u32,

    texture_bind_group: wgpu::BindGroup,
}

impl Gif2dInstance {
    pub fn new(device: &wgpu::Device, pipeline: &Gif2dPipeline, texture: &Texture) -> Self {
        let texture_bind_group = pipeline.load_texture(&device, texture);

        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Gif2d Instance Buffer"),
            size: 0,
            usage: wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        });

        Self {
            texture_bind_group,
            instance_buffer,
            instance_count: 0,
        }
    }

    #[inline]
    pub fn update(&self, queue: &wgpu::Queue, data: RawGif2dInstance) {
        queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&[data]));
    }
}

//====================================================================

#[derive(Unique)]
pub struct Gif2dPipeline {
    pipeline: wgpu::RenderPipeline,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    pub texture_instance_bind_group_layout: wgpu::BindGroupLayout,

    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
}

impl Gif2dPipeline {
    pub fn new(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self
    where
        Self: Sized,
    {
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Texture Bind Group Layout"),
                entries: &[
                    tools::bgl_texture_entry(0),
                    tools::bgl_sampler_entry(1),
                    tools::bgl_uniform_entry(2, wgpu::ShaderStages::FRAGMENT),
                ],
            });

        let texture_instance_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Texture Instance Bind Group Layout"),
                entries: &[tools::bgl_uniform_entry(
                    0,
                    wgpu::ShaderStages::VERTEX_FRAGMENT,
                )],
            });

        let pipeline = tools::create_pipeline(
            &device,
            &config,
            "Texture Pipeline",
            &[
                camera_bind_group_layout,
                &texture_bind_group_layout,
                &texture_instance_bind_group_layout,
            ],
            &[RawTextureVertex::desc()],
            include_str!("gif2d_shader.wgsl"),
            tools::RenderPipelineDescriptor::default().with_depth_stencil(),
        );

        let vertex_buffer = tools::vertex_buffer(&device, "Texture Pipeline", &TEXTURE_VERTICES);

        let index_buffer = tools::index_buffer(&device, "Texture Pipeline", &TEXTURE_INDICES);
        let index_count = TEXTURE_INDICES.len() as u32;

        Self {
            pipeline,
            texture_bind_group_layout,
            texture_instance_bind_group_layout,
            vertex_buffer,
            index_buffer,
            index_count,
        }
    }

    pub fn load_texture(&self, device: &wgpu::Device, data: &Texture) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("TextureBindGroup"),
            layout: &self.texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&data.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&data.sampler),
                },
            ],
        })
    }

    pub fn render<'a, I: Iterator<Item = &'a Gif2dInstance>>(
        &self,
        pass: &mut wgpu::RenderPass,
        camera_bind_goup: &wgpu::BindGroup,
        instances: I,
    ) {
        pass.set_pipeline(&self.pipeline);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);

        pass.set_bind_group(0, camera_bind_goup, &[]);

        instances.for_each(|instance| {
            pass.set_vertex_buffer(1, instance.instance_buffer.slice(..));
            pass.set_bind_group(1, &instance.texture_bind_group, &[]);

            pass.draw_indexed(0..self.index_count, 0, 0..instance.instance_count);
        });
    }
}

//====================================================================

pub struct Gif2dManager {
    instances: HashMap<TextureID, Gif2dInstance>,
}

impl Gif2dManager {}

//====================================================================
