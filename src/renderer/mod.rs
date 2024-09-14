//====================================================================

use camera::{
    sys_resize_camera, sys_setup_camera, sys_update_camera, Camera, MainCamera, UiCamera,
};
use circle_pipeline::{sys_update_circle_pipeline, CirclePipeline};
use gif2d_pipeline::Gif2dPipeline;
use pollster::FutureExt;
use shipyard::{AllStoragesView, IntoIter, IntoWorkload, Unique, View};
use text_pipeline::{
    sys_prep_text, sys_resize_text_pipeline, sys_setup_text_pipeline, sys_trim_text_pipeline,
    TextPipeline,
};
use texture::{sys_resize_depth_texture, sys_setup_depth_texture, DepthTexture};
use texture2d_pipeline::Texture2dPipeline;

use crate::{
    app::Stages,
    images::{GifImage, ImageShown, StandardImage},
    shipyard_tools::{Plugin, Res, ResMut, UniqueTools},
    tools::Size,
    window::{ResizeEvent, WindowSize},
};

pub mod camera;
pub mod circle_pipeline;
pub mod gif2d_pipeline;
pub mod shared;
pub mod text_pipeline;
pub mod texture;
pub mod texture2d_pipeline;
pub mod tools;

//====================================================================

pub(crate) struct RendererPlugin;

impl Plugin<Stages> for RendererPlugin {
    fn build(&self, workload_builder: &mut crate::shipyard_tools::WorkloadBuilder<Stages>) {
        workload_builder
            .add_workload(
                Stages::PreSetup,
                (sys_setup_renderer_components, sys_setup_camera).into_sequential_workload(),
            )
            .add_workload(
                Stages::Setup,
                (
                    sys_setup_misc,
                    sys_setup_depth_texture,
                    sys_setup_pipelines,
                    sys_setup_text_pipeline,
                )
                    .into_workload(),
            )
            .add_workload(
                // FIX - text appears wobbly when scrolling due to updating the frame after the camera does
                Stages::PreUpdate,
                (
                    sys_update_camera::<MainCamera>,
                    sys_update_camera::<UiCamera>,
                )
                    .into_workload(),
            )
            .add_workload(
                Stages::PostUpdate,
                (
                    // sys_update_camera::<MainCamera>,
                    // sys_update_camera::<UiCamera>,
                    sys_prep_text,
                    sys_update_circle_pipeline,
                )
                    .into_workload(),
            )
            .add_workload(Stages::PreRender, (sys_setup_render_pass).into_workload())
            .add_workload(
                Stages::Render,
                (
                    sys_render_circles,
                    sys_render_textures,
                    sys_render_gifs,
                    sys_finish_pass,
                    sys_render_text,
                )
                    .into_sequential_workload(),
            )
            .add_workload(Stages::PostRender, (sys_trim_text_pipeline).into_workload())
            .add_event::<ResizeEvent>(
                (
                    sys_resize,
                    sys_resize_depth_texture,
                    sys_resize_text_pipeline,
                    sys_resize_camera::<UiCamera>,
                )
                    .into_workload(),
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
    camera: Res<Camera<MainCamera>>,
) {
    all_storages
        .insert(Texture2dPipeline::new(
            device.inner(),
            config.inner(),
            camera.bind_group_layout(),
        ))
        .insert(CirclePipeline::new(
            device.inner(),
            config.inner(),
            camera.bind_group_layout(),
        ))
        .insert(Gif2dPipeline::new(
            device.inner(),
            config.inner(),
            camera.bind_group_layout(),
        ));
}

fn sys_setup_misc(all_storages: AllStoragesView) {
    all_storages.add_unique(ClearColor::default());
}

fn sys_setup_render_pass(
    all_storages: AllStoragesView,
    mut tools: ResMut<RenderPassTools>,
    clear_color: Res<ClearColor>,
    depth: Res<DepthTexture>,
) {
    let pass = tools
        .render_pass_desc(RenderPassToolsDesc {
            use_depth: Some(&depth.main_texture().view),
            clear_color: Some(clear_color.to_array()),
        })
        .forget_lifetime();

    all_storages.add_unique(RenderPass { pass });
}

fn sys_render_circles(
    mut pass: ResMut<RenderPass>,
    circle_pipeline: Res<CirclePipeline>,
    main_camera: Res<Camera<MainCamera>>,
) {
    circle_pipeline.render(&mut pass.pass, &main_camera.bind_group());
}

fn sys_render_textures(
    mut pass: ResMut<RenderPass>,
    texture_pipeline: Res<Texture2dPipeline>,

    main_camera: Res<Camera<MainCamera>>,
    ui_camera: Res<Camera<UiCamera>>,

    v_images: View<StandardImage>,
    v_shown: View<ImageShown>,
) {
    let images = (&v_images, !&v_shown)
        .iter()
        .map(|(image, _)| &image.instance);

    texture_pipeline.render(
        &mut pass.pass,
        &main_camera.bind_group(),
        images.into_iter(),
        // Some(viewport.inner()), // BUG - fix viewport not working with world space
        None,
    );

    if !v_shown.is_empty() {
        let images = (&v_images, &v_shown)
            .iter()
            .map(|(image, _)| &image.instance);

        texture_pipeline.render(
            &mut pass.pass,
            &ui_camera.bind_group(),
            images.into_iter(),
            // Some(viewport.inner()), // BUG - fix viewport not working with world space
            None,
        );
    }
}

fn sys_render_gifs(
    mut pass: ResMut<RenderPass>,
    gif_pipeline: Res<Gif2dPipeline>,

    main_camera: Res<Camera<MainCamera>>,
    ui_camera: Res<Camera<UiCamera>>,

    v_gifs: View<GifImage>,
    v_shown: View<ImageShown>,
) {
    let images = (&v_gifs, !&v_shown)
        .iter()
        .map(|(image, _)| &image.instance);

    gif_pipeline.render(
        &mut pass.pass,
        &main_camera.bind_group(),
        images.into_iter(),
    );

    if !v_shown.is_empty() {
        let images = (&v_gifs, &v_shown).iter().map(|(image, _)| &image.instance);

        gif_pipeline.render(&mut pass.pass, &ui_camera.bind_group(), images.into_iter());
    }
}

fn sys_finish_pass(all_storages: AllStoragesView) {
    all_storages.remove_unique::<RenderPass>().ok();
}

fn sys_render_text(mut tools: ResMut<RenderPassTools>, text_pipeline: Res<TextPipeline>) {
    let mut pass = tools.render_pass_desc(RenderPassToolsDesc::none());
    text_pipeline.render(&mut pass);
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
pub struct RenderPass {
    pass: wgpu::RenderPass<'static>,
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
