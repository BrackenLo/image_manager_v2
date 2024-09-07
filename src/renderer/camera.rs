//====================================================================

use std::marker::PhantomData;

use shipyard::{AllStoragesView, Unique};
use wgpu::util::DeviceExt;

use crate::{
    shipyard_tools::{Res, ResMut, UniqueTools},
    tools::Size,
    window::WindowSize,
};

use super::{Device, Queue};

//====================================================================

pub struct MainCamera;
pub struct UiCamera;

#[derive(Unique)]
pub struct Camera<T: 'static + Send + Sync> {
    phantom: PhantomData<T>,

    camera_buffer: wgpu::Buffer,
    camera_bind_group_layout: wgpu::BindGroupLayout,
    camera_bind_group: wgpu::BindGroup,

    pub raw: OrthographicCamera,
}

impl<T> Camera<T>
where
    T: 'static + Send + Sync,
{
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
            phantom: PhantomData,

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

pub(super) fn sys_setup_camera(
    all_storages: AllStoragesView,
    device: Res<Device>,
    size: Res<WindowSize>,
) {
    let main_camera = Camera::<MainCamera>::new(device.inner(), size.inner());
    let ui_camera = Camera::<UiCamera>::new(device.inner(), size.inner());

    all_storages.insert(main_camera).insert(ui_camera);
}

pub(super) fn sys_resize_camera<T: 'static + Send + Sync>(
    size: Res<WindowSize>,
    mut camera: ResMut<Camera<T>>,
) {
    camera.raw.set_size(size.width_f32(), size.height_f32());
}

pub(super) fn sys_update_camera<T: 'static + Send + Sync>(
    queue: Res<Queue>,
    camera: ResMut<Camera<T>>,
) {
    if camera.is_modified() {
        camera.update_camera(queue.inner());
    }
}

//====================================================================

pub trait CameraType {
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

impl CameraType for OrthographicCamera {
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

        // BUG  - find out why camera axis is wrong way around
        let transform_matrix =
            glam::Mat4::from_rotation_translation(self.rotation, -self.translation);

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

    pub fn _new_centered(half_width: f32, half_height: f32) -> Self {
        Self {
            left: -half_width,
            right: half_width,
            bottom: -half_height,
            top: half_height,
            ..Default::default()
        }
    }

    pub fn set_size(&mut self, width: f32, height: f32) {
        let half_width = width / 2.;
        let half_height = height / 2.;

        self.left = -half_width;
        self.right = half_width;
        self.top = half_height;
        self.bottom = -half_height;
    }

    pub fn screen_to_camera(&self, screen_pos: glam::Vec2) -> glam::Vec2 {
        // TODO/FIX - Test this function with different ratios
        screen_pos + self.translation.truncate()
            - glam::vec2((self.right - self.left) / 2., (self.top - self.bottom) / 2.)
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

impl CameraType for PerspectiveCamera {
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

//====================================================================
