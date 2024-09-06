//====================================================================

use shipyard::{Component, IntoIter, Unique, View};
use wgpu::util::DeviceExt;

use crate::{
    images::Pos,
    tools::{Res, ResMut},
};

use super::{
    camera::MainCamera,
    tools::{self},
    Device, Queue, Vertex,
};

//====================================================================

#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable, Clone, Copy)]
pub struct RawVertex {
    pos: [f32; 2],
}

impl Vertex for RawVertex {
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        const VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![
            0 => Float32x2
        ];

        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<RawVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &VERTEX_ATTRIBUTES,
        }
    }
}

const VERTICES: [RawVertex; 4] = [
    RawVertex { pos: [-0.5, 0.5] },
    RawVertex { pos: [-0.5, -0.5] },
    RawVertex { pos: [0.5, 0.5] },
    RawVertex { pos: [0.5, -0.5] },
];

pub const INDICES: [u16; 6] = [0, 1, 3, 0, 3, 2];

#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable, Clone, Copy)]
pub struct RawInstance {
    pub pos: [f32; 2],
    pub radius: f32,
    pub border_radius: f32,
    pub color: [f32; 4],
    pub border_color: [f32; 4],
    // hollow: bool, // TODO
}

impl Vertex for RawInstance {
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        const VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
            1 => Float32x2, 2 => Float32, 3 => Float32, 4 => Float32x4, 5 => Float32x4,
        ];

        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<RawInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &VERTEX_ATTRIBUTES,
        }
    }
}

impl RawInstance {
    pub fn new(pos: [f32; 2], radius: f32) -> Self {
        Self {
            pos,
            radius,
            border_radius: 6.,
            color: [1., 1., 1., 1.],
            border_color: [0., 0., 0., 1.],
        }
    }
    pub fn with_color(mut self, color: [f32; 4]) -> Self {
        self.color = color;
        self
    }
    pub fn hollow(mut self) -> Self {
        self.color = [0., 0., 0., 0.];
        self
    }
    pub fn with_border(mut self, radius: f32, color: [f32; 4]) -> Self {
        self.border_radius = radius;
        self.border_color = color;
        self
    }
}

//====================================================================

#[derive(Unique)]
pub struct CirclePipeline {
    pipeline: wgpu::RenderPipeline,

    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,

    instance_buffer: wgpu::Buffer,
    instance_count: u32,
}

impl CirclePipeline {
    pub fn new(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        camera: &MainCamera,
    ) -> Self {
        let pipeline = tools::create_pipeline(
            device,
            config,
            "Circle Pipeline",
            &[&camera.bind_group_layout()],
            &[RawVertex::desc(), RawInstance::desc()],
            include_str!("circle_shader.wgsl").into(),
            // tools::RenderPipelineDescriptor {
            //     fragment_targets: Some(&[Some(wgpu::ColorTargetState {
            //         format: core.config.format,
            //         blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            //         write_mask: wgpu::ColorWrites::all(),
            //     })]),
            //     ..Default::default()
            // },
            tools::RenderPipelineDescriptor::default().with_depth_stencil(),
        );

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Circle Pipeline Vertex Buffer"),
            contents: bytemuck::cast_slice(&VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Circle Pipeline Index Buffer"),
            contents: bytemuck::cast_slice(&INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });
        let index_count = INDICES.len() as u32;

        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Circle Pipeline Instance Buffer"),
            size: 0,
            usage: wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        });
        let instance_count = 0 as u32;

        Self {
            pipeline,
            vertex_buffer,
            index_buffer,
            index_count,
            instance_buffer,
            instance_count,
        }
    }

    pub fn render(&self, pass: &mut wgpu::RenderPass, camera: &MainCamera) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &camera.bind_group(), &[]);

        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        pass.set_vertex_buffer(1, self.instance_buffer.slice(..));

        pass.draw_indexed(0..self.index_count, 0, 0..self.instance_count);
    }

    fn update(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, instances: &[RawInstance]) {
        tools::update_instance_buffer(
            device,
            queue,
            "Circle Pipeline Instance Buffer",
            &mut self.instance_buffer,
            &mut self.instance_count,
            instances,
        );
    }
}

//====================================================================

#[derive(Component)]
pub struct Circle {
    pub radius: f32,
}

pub(crate) fn sys_update_circle_pipeline(
    device: Res<Device>,
    queue: Res<Queue>,
    mut pipeline: ResMut<CirclePipeline>,

    v_circle: View<Circle>,
    v_pos: View<Pos>,
) {
    let instances = (&v_circle, &v_pos)
        .iter()
        .map(|(circle, pos)| RawInstance::new([pos.x, pos.y], circle.radius).hollow())
        .collect::<Vec<_>>();

    pipeline.update(device.inner(), queue.inner(), &instances);
}

//====================================================================
