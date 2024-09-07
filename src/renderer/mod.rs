//====================================================================

use camera::{sys_setup_camera, sys_update_camera, MainCamera};
use circle_pipeline::{sys_update_circle_pipeline, CirclePipeline};
use pollster::FutureExt;
use shipyard::{AllStoragesView, IntoIter, IntoWorkload, Unique, View, Workload};
use text::{
    sys_prep_text, sys_resize_text_pipeline, sys_setup_text_pipeline, sys_trim_text_pipeline,
    TextPipeline,
};
use texture::{sys_resize_depth_texture, sys_setup_depth_texture, DepthTexture};
use texture_pipeline::TexturePipeline;

use crate::{
    images::StandardImage,
    shipyard_tools::{Plugin, Res, ResMut, Stages, UniqueTools},
    tools::Size,
    window::{ResizeEvent, WindowSize},
};

pub mod camera;
pub mod circle_pipeline;
pub mod text;
pub mod texture;
pub mod texture_pipeline;
pub mod tools;

//====================================================================

pub(crate) struct RendererPlugin;

impl Plugin for RendererPlugin {
    fn build(&self, workload_builder: &mut crate::shipyard_tools::WorkloadBuilder) {
        workload_builder
            .add_workload(
                Stages::PreSetup,
                Workload::new("")
                    .with_system(sys_setup_renderer_components)
                    .with_system(sys_setup_camera)
                    .into_sequential_workload(),
            )
            .add_workload(
                Stages::Setup,
                Workload::new("")
                    .with_system(sys_setup_misc)
                    .with_system(sys_setup_depth_texture)
                    .with_system(sys_setup_pipelines)
                    .with_system(sys_setup_text_pipeline),
            )
            .add_workload(
                Stages::PreRender,
                Workload::new("")
                    .with_system(sys_update_camera)
                    .with_system(sys_prep_text)
                    .with_system(sys_update_circle_pipeline),
            )
            .add_workload(Stages::Render, Workload::new("").with_system(sys_render))
            .add_workload(
                Stages::PostRender,
                Workload::new("").with_system(sys_trim_text_pipeline),
            )
            .add_event::<ResizeEvent>(
                Workload::new("")
                    .with_system(sys_resize)
                    .with_system(sys_resize_depth_texture)
                    .with_system(sys_resize_text_pipeline),
            );
    }
}

//====================================================================

pub trait Vertex: bytemuck::Pod {
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a>;
}

//====================================================================

#[derive(Unique)]
pub struct Device(wgpu::Device);
impl Device {
    #[inline]
    pub fn inner(&self) -> &wgpu::Device {
        &self.0
    }
}

#[derive(Unique)]
pub struct Queue(wgpu::Queue);
impl Queue {
    #[inline]
    pub fn inner(&self) -> &wgpu::Queue {
        &self.0
    }
}

#[derive(Unique)]
pub struct Surface(wgpu::Surface<'static>);
impl Surface {
    #[inline]
    pub fn inner(&self) -> &wgpu::Surface {
        &self.0
    }
}

#[derive(Unique)]
pub struct SurfaceConfig(wgpu::SurfaceConfiguration);
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

#[derive(Unique)]
pub struct ClearColor {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

impl Default for ClearColor {
    fn default() -> Self {
        Self {
            r: 0.2,
            g: 0.2,
            b: 0.2,
            a: 1.,
        }
    }
}

impl ClearColor {
    fn to_array(&self) -> [f64; 4] {
        [self.r, self.g, self.b, self.a]
    }
}

//====================================================================

fn sys_setup_renderer_components(
    all_storages: AllStoragesView,
    window: Res<crate::window::Window>,
) {
    log::info!("Creating renderer");

    let size = window.inner().inner_size();

    log::trace!("Creating core wgpu renderer components.");

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY,
        ..Default::default()
    });

    let surface = instance.create_surface(window.arc().clone()).unwrap();

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

fn sys_setup_pipelines(
    all_storages: AllStoragesView,
    device: Res<Device>,
    config: Res<SurfaceConfig>,
    camera: Res<MainCamera>,
) {
    let texture_pipeline = TexturePipeline::new(device.inner(), config.inner(), &camera);
    let circle_pipeline = CirclePipeline::new(device.inner(), config.inner(), &camera);

    all_storages
        .insert(texture_pipeline)
        .insert(circle_pipeline);
}

fn sys_setup_misc(all_storages: AllStoragesView) {
    all_storages.add_unique(ClearColor::default());
}

fn sys_render(
    mut tools: ResMut<RenderPassTools>,
    clear_color: Res<ClearColor>,
    depth: Res<DepthTexture>,

    text_pipeline: Res<TextPipeline>,
    texture_pipeline: Res<TexturePipeline>,
    circle_pipeline: Res<CirclePipeline>,

    camera: Res<MainCamera>,
    v_images: View<StandardImage>,
) {
    {
        let desc = RenderPassToolsDesc {
            use_depth: Some(&depth.main_texture().view),
            clear_color: Some(clear_color.to_array()),
        };

        let mut pass = tools.render_pass_desc(desc);

        let images = v_images.iter().map(|image| &image.instance);
        texture_pipeline.render(
            &mut pass,
            &camera,
            images.into_iter(),
            // Some(viewport.inner()), // BUG - fix viewport not working with world space
            None,
        );
        circle_pipeline.render(&mut pass, &camera);
    }

    {
        let mut pass = tools.render_pass_desc(RenderPassToolsDesc::none());
        text_pipeline.render(&mut pass);
    }
}

fn sys_resize(
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

impl RenderPassToolsDesc<'_> {
    pub fn none() -> Self {
        Self {
            use_depth: None,
            clear_color: None,
        }
    }
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

    pub fn render_pass_desc(&mut self, desc: RenderPassToolsDesc) -> wgpu::RenderPass {
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

        let render_pass = self.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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

        render_pass
    }
}

//====================================================================
