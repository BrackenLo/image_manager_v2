//====================================================================

use std::sync::Arc;

use camera::MainCamera;
use pollster::FutureExt;
use shipyard::{AllStoragesView, Unique};
use texture_pipeline::TexturePipeline;

use crate::{
    tools::{Res, ResMut, Size, UniqueTools},
    window::WindowSize,
};

pub mod camera;
pub mod texture;
pub mod texture_pipeline;
pub mod tools;

//====================================================================

pub(crate) trait Vertex: bytemuck::Pod {
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a>;
}

//====================================================================

#[derive(Unique)]
pub(crate) struct Device(wgpu::Device);
impl Device {
    #[inline]
    pub fn inner(&self) -> &wgpu::Device {
        &self.0
    }
}

#[derive(Unique)]
pub(crate) struct Queue(wgpu::Queue);
impl Queue {
    #[inline]
    pub fn inner(&self) -> &wgpu::Queue {
        &self.0
    }
}

#[derive(Unique)]
pub(crate) struct Surface(wgpu::Surface<'static>);
impl Surface {
    #[inline]
    pub fn inner(&self) -> &wgpu::Surface {
        &self.0
    }
}

#[derive(Unique)]
pub(crate) struct SurfaceConfig(wgpu::SurfaceConfiguration);
impl SurfaceConfig {
    #[inline]
    pub fn inner(&self) -> &wgpu::SurfaceConfiguration {
        &self.0
    }

    fn resize(&mut self, size: Size<u32>) {
        self.0.width = size.width;
        self.0.height = size.height;
    }
}

//====================================================================

pub(crate) fn sys_setup_renderer_components(
    window: Arc<winit::window::Window>,
    all_storages: AllStoragesView,
) {
    log::info!("Creating renderer");

    let size = window.inner_size();

    log::trace!("Creating core wgpu renderer components.");

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY,
        ..Default::default()
    });
    let surface = instance.create_surface(window.clone()).unwrap();

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        })
        .block_on()
        .unwrap();

    log::debug!("Chosen device adapter: {:#?}", adapter.get_info());

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor::default(), None)
        .block_on()
        .unwrap();

    let surface_capabilities = surface.get_capabilities(&adapter);

    let surface_format = surface_capabilities
        .formats
        .iter()
        .find(|format| format.is_srgb())
        .copied()
        .unwrap_or(surface_capabilities.formats[0]);

    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: surface_format,
        width: size.width,
        height: size.height,
        present_mode: wgpu::PresentMode::AutoNoVsync,
        desired_maximum_frame_latency: 2,
        alpha_mode: surface_capabilities.alpha_modes[0],
        view_formats: vec![],
    };

    surface.configure(&device, &config);

    all_storages
        .insert(Device(device))
        .insert(Queue(queue))
        .insert(Surface(surface))
        .insert(SurfaceConfig(config));
}

pub(crate) fn sys_setup_pipelines(
    all_storages: AllStoragesView,
    device: Res<Device>,
    config: Res<SurfaceConfig>,
    camera: Res<MainCamera>,
) {
    let texture_pipeline = TexturePipeline::new(device.inner(), config.inner(), &camera);

    all_storages.insert(texture_pipeline);
}

pub(crate) fn sys_resize(
    device: Res<Device>,
    surface: Res<Surface>,
    mut config: ResMut<SurfaceConfig>,
    size: Res<WindowSize>,
) {
    config.resize(size.inner());
    surface.inner().configure(device.inner(), config.inner());
}

//====================================================================

pub struct RenderPassToolsDesc<'a> {
    pub use_depth: Option<&'a wgpu::TextureView>,
    pub clear_color: Option<[f64; 4]>,
}

impl Default for RenderPassToolsDesc<'_> {
    fn default() -> Self {
        Self {
            use_depth: None,
            clear_color: Some([0.2, 0.2, 0.2, 1.]),
        }
    }
}

#[derive(Unique)]
pub struct RenderPassTools {
    surface_texture: wgpu::SurfaceTexture,
    surface_view: wgpu::TextureView,
    encoder: wgpu::CommandEncoder,
}

impl RenderPassTools {
    pub(crate) fn new(
        device: &wgpu::Device,
        surface: &wgpu::Surface,
    ) -> Result<Self, wgpu::SurfaceError> {
        let (surface_texture, surface_view) = match surface.get_current_texture() {
            Ok(texture) => {
                let view = texture
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                (texture, view)
            }
            Err(e) => return Err(e),
        };

        let encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Main Command Encoder"),
        });

        Ok(RenderPassTools {
            surface_texture,
            surface_view,
            encoder,
        })
    }

    pub(crate) fn finish(self, queue: &wgpu::Queue) {
        queue.submit(Some(self.encoder.finish()));
        self.surface_texture.present();
    }

    #[inline]
    pub fn render_pass<'b, F>(&'b mut self, f: F)
    where
        F: for<'encoder> FnOnce(&mut wgpu::RenderPass),
    {
        self.render_pass_desc(RenderPassToolsDesc::default(), f);
    }

    pub fn render_pass_desc<'b, F>(&'b mut self, desc: RenderPassToolsDesc, f: F)
    where
        F: for<'encoder> FnOnce(&mut wgpu::RenderPass),
    {
        // Clear the current depth buffer and use it.
        let depth_stencil_attachment = match desc.use_depth {
            Some(view) => Some(wgpu::RenderPassDepthStencilAttachment {
                view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            None => None,
        };

        let load = match desc.clear_color {
            Some(color) => wgpu::LoadOp::Clear(wgpu::Color {
                r: color[0],
                g: color[1],
                b: color[2],
                a: color[3],
            }),
            None => wgpu::LoadOp::Load,
        };

        let mut render_pass = self.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Render Tools Basic Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.surface_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        f(&mut render_pass);
    }
}

//====================================================================
