//====================================================================

use shipyard::Unique;
use wgpu::util::DeviceExt;

// use crate::app::entities::Image;

use crate::tools::Rect;

use super::{camera::MainCamera, texture::Texture, tools, Vertex};

//====================================================================

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Zeroable, bytemuck::Pod)]
pub struct RawTextureVertex {
    pos: [f32; 2],
    uv: [f32; 2],
}

impl Vertex for RawTextureVertex {
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        const VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 2] = wgpu::vertex_attr_array![
                0 => Float32x2, 1 => Float32x2
        ];

        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<RawTextureVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &VERTEX_ATTRIBUTES,
        }
    }
}

pub const TEXTURE_VERTICES: [RawTextureVertex; 4] = [
    RawTextureVertex {
        pos: [-0.5, 0.5],
        uv: [0., 0.],
    },
    RawTextureVertex {
        pos: [-0.5, -0.5],
        uv: [0., 1.],
    },
    RawTextureVertex {
        pos: [0.5, 0.5],
        uv: [1., 0.],
    },
    RawTextureVertex {
        pos: [0.5, -0.5],
        uv: [1., 1.],
    },
];

pub const TEXTURE_INDICES: [u16; 6] = [0, 1, 3, 0, 3, 2];

//====================================================================

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Zeroable, bytemuck::Pod)]
pub struct RawTextureInstance {
    pub pos: [f32; 2],
    pub size: [f32; 2],
    pub color: [f32; 4],
}

pub struct TextureInstance {
    bind_group: wgpu::BindGroup,
    buffer: wgpu::Buffer,

    texture_bind_group: wgpu::BindGroup,
}

impl TextureInstance {
    pub fn new(
        device: &wgpu::Device,
        pipeline: &TexturePipeline,
        data: RawTextureInstance,
        texture: &Texture,
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

        let texture_bind_group = pipeline.load_texture(&device, texture);

        Self {
            bind_group,
            buffer,
            texture_bind_group,
        }
    }

    #[inline]
    pub fn update(&self, queue: &wgpu::Queue, data: RawTextureInstance) {
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[data]));
    }
}

//====================================================================

#[derive(Unique)]
pub struct TexturePipeline {
    pipeline: wgpu::RenderPipeline,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    pub texture_instance_bind_group_layout: wgpu::BindGroupLayout,

    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
}

impl TexturePipeline {
    pub fn new(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        camera: &MainCamera,
    ) -> Self
    where
        Self: Sized,
    {
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Texture Bind Group Layout"),
                entries: &[tools::bgl_texture_entry(0), tools::bgl_sampler_entry(1)],
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
                camera.bind_group_layout(),
                &texture_bind_group_layout,
                &texture_instance_bind_group_layout,
            ],
            &[RawTextureVertex::desc()],
            include_str!("texture_shader.wgsl"),
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

    pub fn render<'a, I: Iterator<Item = &'a TextureInstance>>(
        &self,
        pass: &mut wgpu::RenderPass,
        camera: &MainCamera,
        to_render: I,
        viewport: Option<&Rect>,
    ) {
        if let Some(viewport) = viewport {
            pass.set_viewport(
                viewport.x,
                viewport.y,
                viewport.width,
                viewport.height,
                0.,
                1.,
            );
        }

        pass.set_pipeline(&self.pipeline);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);

        pass.set_bind_group(0, camera.bind_group(), &[]);

        to_render.for_each(|to_render| {
            pass.set_bind_group(1, &to_render.texture_bind_group, &[]);
            pass.set_bind_group(2, &to_render.bind_group, &[]);

            pass.draw_indexed(0..self.index_count, 0, 0..1);
        });
    }
}

//====================================================================
