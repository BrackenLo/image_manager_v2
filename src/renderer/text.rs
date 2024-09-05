//====================================================================

use glyphon::{
    Attrs, Buffer, Cache, FontSystem, Metrics, Resolution, Shaping, SwashCache, TextArea,
    TextAtlas, TextBounds, TextRenderer, Viewport,
};
use shipyard::{AllStoragesView, Component, IntoIter, Unique, View};

use crate::{
    tools::{Res, ResMut},
    window::WindowSize,
};

use super::{Device, Queue, SurfaceConfig};

//====================================================================

#[derive(Unique)]
pub struct TextPipeline {
    renderer: TextRenderer,
    font_system: FontSystem,
    swash_cache: SwashCache,
    atlas: TextAtlas,
    viewport: Viewport,
}

impl TextPipeline {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, config: &wgpu::SurfaceConfiguration) -> Self
    where
        Self: Sized,
    {
        let cache = Cache::new(device);
        let font_system = FontSystem::new();
        let swash_cache = SwashCache::new();
        let mut atlas = TextAtlas::new(device, queue, &cache, config.format);
        let viewport = Viewport::new(device, &cache);

        let renderer =
            TextRenderer::new(&mut atlas, device, wgpu::MultisampleState::default(), None);

        Self {
            renderer,
            font_system,
            swash_cache,
            atlas,
            viewport,
        }
    }

    fn resize(&mut self, queue: &wgpu::Queue, width: u32, height: u32) {
        self.viewport.update(queue, Resolution { width, height });
    }

    fn prep(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        data: Vec<TextArea>,
    ) -> Result<(), glyphon::PrepareError> {
        self.renderer.prepare(
            device,
            queue,
            &mut self.font_system,
            &mut self.atlas,
            &self.viewport,
            data,
            &mut self.swash_cache,
        )
    }

    pub fn render<'a: 'b, 'b>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        self.renderer
            .render(&self.atlas, &self.viewport, pass)
            .unwrap();
    }

    pub fn trim(&mut self) {
        self.atlas.trim();
    }
}

pub(crate) fn sys_setup_text_pipeline(
    all_storages: AllStoragesView,
    device: Res<Device>,
    queue: Res<Queue>,
    config: Res<SurfaceConfig>,
) {
    let pipeline = TextPipeline::new(device.inner(), queue.inner(), config.inner());
    all_storages.add_unique(pipeline);
}

pub(crate) fn sys_resize_text_pipeline(
    queue: Res<Queue>,
    size: Res<WindowSize>,

    mut text_pipeline: ResMut<TextPipeline>,
) {
    text_pipeline.resize(queue.inner(), size.width(), size.height());
}

pub(crate) fn sys_prep_text(
    device: Res<Device>,
    queue: Res<Queue>,

    mut text_pipeline: ResMut<TextPipeline>,
    v_buffers: View<TextBuffer>,
) {
    let data = v_buffers
        .iter()
        .map(|buffer| TextArea {
            buffer: &buffer.buffer,
            left: buffer.pos.0,
            top: buffer.pos.1,
            scale: 1.,
            bounds: buffer.bounds,
            default_color: buffer.color,
        })
        .collect::<Vec<_>>();

    text_pipeline
        .prep(device.inner(), queue.inner(), data)
        .unwrap();
}

pub(crate) fn sys_trim_text_pipeline(mut text_pipeline: ResMut<TextPipeline>) {
    text_pipeline.trim();
}

//====================================================================

pub struct TextBufferDescriptor<'a> {
    pub font_size: f32,
    pub line_height: f32,
    pub bounds: TextBounds,

    pub text: &'a str,
    pub pos: (f32, f32),
}

impl Default for TextBufferDescriptor<'_> {
    fn default() -> Self {
        Self {
            font_size: 30.,
            line_height: 42.,
            bounds: TextBounds {
                left: 0,
                top: 0,
                right: 800,
                bottom: 160,
            },
            text: "",
            pos: (0., 0.),
        }
    }
}

#[derive(Component)]
pub struct TextBuffer {
    pub buffer: Buffer,
    pub bounds: TextBounds,
    pub pos: (f32, f32),
    pub color: glyphon::Color,
}

impl TextBuffer {
    pub fn new(text_pipeline: &mut TextPipeline, desc: &TextBufferDescriptor) -> Self {
        let mut buffer = Buffer::new(
            &mut text_pipeline.font_system,
            Metrics::new(desc.font_size, desc.line_height),
        );

        buffer.set_text(
            &mut text_pipeline.font_system,
            desc.text,
            Attrs::new(),
            Shaping::Advanced,
        );

        Self {
            buffer,
            bounds: desc.bounds,
            pos: desc.pos,
            color: glyphon::Color::rgb(0, 0, 0),
        }
    }

    #[inline]
    pub fn set_text(&mut self, text_pipeline: &mut TextPipeline, text: &str) {
        self.buffer.set_text(
            &mut text_pipeline.font_system,
            text,
            Attrs::new(),
            Shaping::Advanced,
        );
    }
}

//====================================================================
