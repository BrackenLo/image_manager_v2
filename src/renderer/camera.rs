//====================================================================

use shipyard::{AllStoragesView, Unique};
use wgpu::util::DeviceExt;

use crate::{
    tools::{Res, ResMut, Size, UniqueTools},
    window::WindowSize,
};

use super::{Device, Queue};

//====================================================================

#[derive(Unique)]
pub(crate) struct MainCamera {
    camera_buffer: wgpu::Buffer,
    camera_bind_group_layout: wgpu::BindGroupLayout,
    camera_bind_group: wgpu::BindGroup,

    pub(crate) raw: OrthographicCamera,
}

impl MainCamera {
    pub fn new(device: &wgpu::Device, size: Size<u32>) -> Self {
        let camera = OrthographicCamera::new_sized(size.width as f32, size.height as f32);

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera buffer"),
            contents: bytemuck::cast_slice(&[camera.into_uniform()]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Camera Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera Bind Group"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(camera_buffer.as_entire_buffer_binding()),
            }],
        });

        Self {
            camera_buffer,
            camera_bind_group_layout,
            camera_bind_group,
            raw: camera,
        }
    }

    #[inline]
    pub fn update_camera(&self, queue: &wgpu::Queue) {
        queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[self.raw.into_uniform()]),
        );
    }

    #[inline]
    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.camera_bind_group_layout
    }

    #[inline]
    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.camera_bind_group
    }
}

pub(crate) fn sys_setup_camera(
    all_storages: AllStoragesView,
    device: Res<Device>,
    size: Res<WindowSize>,
) {
    let camera = MainCamera::new(device.inner(), size.inner());

    all_storages.insert(camera);
}

pub(crate) fn sys_resize_camera(
    queue: Res<Queue>,
    mut camera: ResMut<MainCamera>,
    size: Res<WindowSize>,
) {
    let half_width = size.width() as f32 / 2.;

    // camera.raw.left = -half_width;
    // camera.raw.right = half_width;
    camera.raw.left = 0.;
    camera.raw.right = size.width() as f32;
    camera.raw.top = 0.;
    camera.raw.bottom = -(size.height() as f32);

    camera.update_camera(queue.inner());
}

//====================================================================

pub trait Camera {
    fn into_uniform(&self) -> CameraUniform;
}

#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable, Clone, Copy)]
pub struct CameraUniform {
    view_projection: [f32; 16],
    camera_position: [f32; 3],
    _padding: u32,
}
impl CameraUniform {
    pub fn new(view_projection: [f32; 16], camera_position: [f32; 3]) -> Self {
        Self {
            view_projection,
            camera_position,
            _padding: 0,
        }
    }
}

//--------------------------------------------------

pub struct PerspectiveCamera {
    pub up: glam::Vec3,
    pub aspect: f32,
    pub fovy: f32,
    pub z_near: f32,
    pub z_far: f32,

    pub translation: glam::Vec3,
    pub rotation: glam::Quat,
}
impl Default for PerspectiveCamera {
    fn default() -> Self {
        Self {
            up: glam::Vec3::Y,
            aspect: 1.7777777778,
            fovy: 45.,
            z_near: 0.1,
            z_far: 1000000.,

            translation: glam::Vec3::ZERO,
            rotation: glam::Quat::IDENTITY,
        }
    }
}

impl Camera for PerspectiveCamera {
    fn into_uniform(&self) -> CameraUniform {
        CameraUniform::new(self.get_projection(), self.translation.into())
    }
}

impl PerspectiveCamera {
    fn get_projection(&self) -> [f32; 16] {
        let forward = (self.rotation * glam::Vec3::Z).normalize();

        let projection_matrix =
            glam::Mat4::perspective_lh(self.fovy, self.aspect, self.z_near, self.z_far);

        let view_matrix =
            glam::Mat4::look_at_lh(self.translation, self.translation + forward, self.up);

        (projection_matrix * view_matrix).to_cols_array()
    }
}

//--------------------------------------------------

#[derive(Debug)]
pub struct OrthographicCamera {
    pub left: f32,
    pub right: f32,
    pub bottom: f32,
    pub top: f32,
    pub z_near: f32,
    pub z_far: f32,

    pub translation: glam::Vec3,
    pub rotation: glam::Quat,
}

impl Default for OrthographicCamera {
    fn default() -> Self {
        Self {
            left: 0.,
            right: 1920.,
            bottom: 0.,
            top: 1080.,
            z_near: 0.,
            z_far: 1000000.,

            translation: glam::Vec3::ZERO,
            rotation: glam::Quat::IDENTITY,
        }
    }
}

impl Camera for OrthographicCamera {
    fn into_uniform(&self) -> CameraUniform {
        CameraUniform::new(self.get_projection(), self.translation.into())
    }
}

impl OrthographicCamera {
    fn get_projection(&self) -> [f32; 16] {
        let projection_matrix = glam::Mat4::orthographic_lh(
            self.left,
            self.right,
            self.bottom,
            self.top,
            self.z_near,
            self.z_far,
        );

        let transform_matrix =
            glam::Mat4::from_rotation_translation(self.rotation, self.translation);

        (projection_matrix * transform_matrix).to_cols_array()
    }

    pub fn new_sized(width: f32, height: f32) -> Self {
        Self {
            left: 0.,
            right: width,
            bottom: 0.,
            top: height,
            ..Default::default()
        }
    }

    pub fn new_centered(half_width: f32, half_height: f32, x: f32, y: f32) -> Self {
        Self {
            left: -half_width,
            right: half_width,
            bottom: -half_height,
            top: half_height,
            translation: glam::Vec3::new(x, y, 0.),
            ..Default::default()
        }
    }

    pub fn set_size(&mut self, width: f32, height: f32) {
        let half_width = width / 2.;
        let half_height = height / 2.;

        self.left = -half_width;
        self.right = half_width;
        self.bottom = -half_height;
        self.top = half_height;
    }
}

//====================================================================