//====================================================================

use cabat::renderer::{
    render_tools,
    shared::{
        TextureRectVertex, TEXTURE_RECT_INDEX_COUNT, TEXTURE_RECT_INDICES, TEXTURE_RECT_VERTICES,
    },
    texture, Vertex,
};
use shipyard::Unique;
use wgpu::util::DeviceExt;

use crate::tools::Rect;

//====================================================================

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Zeroable, bytemuck::Pod, Default)]
pub struct Texture2dInstanceRaw {
    pub pos: [f32; 2],
    pub size: [f32; 2],
    pub color: [f32; 4],
}

pub struct Texture2dInstance {
    bind_group: wgpu::BindGroup,
    buffer: wgpu::Buffer,

    texture_bind_group: wgpu::BindGroup,
}

impl Texture2dInstance {
    pub fn new(
        device: &wgpu::Device,
        pipeline: &Texture2dPipeline,
        data: Texture2dInstanceRaw,
        texture: &texture::RawTexture,
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
    pub fn update(&self, queue: &wgpu::Queue, data: Texture2dInstanceRaw) {
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[data]));
    }
}

//====================================================================

#[derive(Unique)]
pub struct Texture2dPipeline {
    pipeline: wgpu::RenderPipeline,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    pub texture_instance_bind_group_layout: wgpu::BindGroupLayout,

    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
}

impl Texture2dPipeline {
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
                    render_tools::bgl_texture_entry(0),
                    render_tools::bgl_sampler_entry(1),
                ],
            });

        let texture_instance_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Texture Instance Bind Group Layout"),
                entries: &[render_tools::bgl_uniform_entry(
                    0,
                    wgpu::ShaderStages::VERTEX_FRAGMENT,
                )],
            });

        let pipeline = render_tools::create_pipeline(
            &device,
            &config,
            "Texture Pipeline",
            &[
                camera_bind_group_layout,
                &texture_bind_group_layout,
                &texture_instance_bind_group_layout,
            ],
            &[TextureRectVertex::desc()],
            include_str!("texture_shader.wgsl"),
            render_tools::RenderPipelineDescriptor::default().with_depth_stencil(),
        );

        let vertex_buffer =
            render_tools::vertex_buffer(&device, "Texture Pipeline", &TEXTURE_RECT_VERTICES);

        let index_buffer =
            render_tools::index_buffer(&device, "Texture Pipeline", &TEXTURE_RECT_INDICES);
        let index_count = TEXTURE_RECT_INDEX_COUNT;

        Self {
            pipeline,
            texture_bind_group_layout,
            texture_instance_bind_group_layout,
            vertex_buffer,
            index_buffer,
            index_count,
        }
    }

    pub fn load_texture(
        &self,
        device: &wgpu::Device,
        data: &texture::RawTexture,
    ) -> wgpu::BindGroup {
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

    pub fn render<'a, I: Iterator<Item = &'a Texture2dInstance>>(
        &self,
        pass: &mut wgpu::RenderPass,
        camera_bind_goup: &wgpu::BindGroup,
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

        pass.set_bind_group(0, camera_bind_goup, &[]);

        to_render.for_each(|to_render| {
            pass.set_bind_group(1, &to_render.texture_bind_group, &[]);
            pass.set_bind_group(2, &to_render.bind_group, &[]);

            pass.draw_indexed(0..self.index_count, 0, 0..1);
        });
    }
}

//====================================================================
