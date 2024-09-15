//====================================================================

use shipyard::Unique;
use wgpu::util::DeviceExt;

use super::{
    shared::{RawTextureVertex, TEXTURE_INDICES, TEXTURE_VERTICES},
    texture::Gif,
    tools, Vertex,
};

//====================================================================

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Zeroable, bytemuck::Pod, Default)]
pub struct Gif2dInstanceRaw {
    pub pos: [f32; 2],
    pub size: [f32; 2],
    pub color: [f32; 4],
    pub frame: f32,
    pub padding: [u32; 3],
}

pub struct Gif2dInstance {
    bind_group: wgpu::BindGroup,
    buffer: wgpu::Buffer,

    texture_bind_group: wgpu::BindGroup,
}

impl Gif2dInstance {
    pub fn new(
        device: &wgpu::Device,
        pipeline: &Gif2dPipeline,
        data: Gif2dInstanceRaw,
        gif: &Gif,
    ) -> Self {
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Texture Instance"),
            contents: bytemuck::cast_slice(&[data]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.texture_instance_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(buffer.as_entire_buffer_binding()),
            }],
        });

        let texture_bind_group = pipeline.load_texture(&device, gif);

        Self {
            texture_bind_group,
            bind_group,
            buffer,
        }
    }

    #[inline]
    pub fn update(&self, queue: &wgpu::Queue, data: Gif2dInstanceRaw) {
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[data]));
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
                label: Some("Gif2d Bind Group Layout"),
                entries: &[
                    tools::bgl_texture_entry(0),
                    tools::bgl_sampler_entry(1),
                    tools::bgl_uniform_entry(2, wgpu::ShaderStages::FRAGMENT),
                ],
            });

        let texture_instance_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Gif2d Instance Bind Group Layout"),
                entries: &[tools::bgl_uniform_entry(
                    0,
                    wgpu::ShaderStages::VERTEX_FRAGMENT,
                )],
            });

        let pipeline = tools::create_pipeline(
            &device,
            &config,
            "Gif2d Pipeline",
            &[
                camera_bind_group_layout,
                &texture_bind_group_layout,
                &texture_instance_bind_group_layout,
            ],
            &[RawTextureVertex::desc()],
            include_str!("gif2d_shader.wgsl"),
            tools::RenderPipelineDescriptor::default().with_depth_stencil(),
        );

        let vertex_buffer = tools::vertex_buffer(&device, "Gif2d Pipeline", &TEXTURE_VERTICES);

        let index_buffer = tools::index_buffer(&device, "Gif2d Pipeline", &TEXTURE_INDICES);
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

    pub fn load_texture(&self, device: &wgpu::Device, data: &Gif) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Gif2dBindGroup"),
            layout: &self.texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&data.texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&data.texture.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Buffer(data.buffer.as_entire_buffer_binding()),
                },
            ],
        })
    }

    pub fn render<'a, I: Iterator<Item = &'a Gif2dInstance>>(
        &self,
        pass: &mut wgpu::RenderPass,
        camera_bind_goup: &wgpu::BindGroup,
        to_render: I,
    ) {
        pass.set_pipeline(&self.pipeline);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);

        pass.set_bind_group(0, camera_bind_goup, &[]);

        to_render.for_each(|to_render| {
            pass.set_bind_group(1, &to_render.texture_bind_group, &[]);
            pass.set_bind_group(2, &to_render.bind_group, &[]);

            pass.draw_indexed(0..self.index_count, 0, 0..1);
        });
    }
}

//====================================================================
