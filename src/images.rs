//====================================================================

use std::time::Duration;

use shipyard::{
    AllStoragesViewMut, Borrow, BorrowInfo, Component, EntitiesViewMut, EntityId, IntoIter,
    IntoWithId, IntoWorkload, View, ViewMut,
};

use crate::{
    app::Stages,
    renderer::{
        gif2d_pipeline::Gif2dInstance, texture::GifFrameDelay,
        texture2d_pipeline::Texture2dInstance,
    },
    shipyard_tools::Plugin,
    tools::Size,
};

//====================================================================

pub(crate) struct ImagePlugin;

impl Plugin<Stages> for ImagePlugin {
    fn build(&self, workload_builder: &mut crate::shipyard_tools::WorkloadBuilder<Stages>) {
        workload_builder.add_workload(
            Stages::Last,
            (sys_remove_pending, sys_clear_dirty).into_workload(),
        );
    }
}

//====================================================================

#[derive(Component, Default, Debug)]
pub struct Pos {
    pub x: f32,
    pub y: f32,
}

impl Pos {
    #[inline]
    pub fn _new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
    #[inline]
    pub fn to_array(&self) -> [f32; 2] {
        [self.x, self.y]
    }
}

//--------------------------------------------------

#[derive(Component)]
pub struct ImageSize {
    pub width: f32,
    pub height: f32,
}

impl ImageSize {
    #[inline]
    pub fn _new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
    #[inline]
    pub fn to_array(&self) -> [f32; 2] {
        [self.width, self.height]
    }
}

impl Default for ImageSize {
    fn default() -> Self {
        Self {
            width: 1.,
            height: 1.,
        }
    }
}

#[derive(Component)]
pub struct ImageMeta {
    pub _texture_resolution: Size<u32>,
    pub aspect: f32,
}

//--------------------------------------------------

#[derive(Component)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    #[inline]
    pub fn _new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }
    #[inline]
    pub fn to_array(&self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }
}

impl Default for Color {
    fn default() -> Self {
        Self {
            r: 1.,
            g: 1.,
            b: 1.,
            a: 1.,
        }
    }
}

//--------------------------------------------------

#[derive(Component)]
pub struct Image;

#[derive(Component)]
pub struct StandardImage {
    pub id: u64,
    pub instance: Texture2dInstance,
}

#[derive(Component)]
pub struct GifImage {
    pub id: u64,
    pub frame: u32,
    pub total_frames: u32,
    pub frames_per_row: u32,
    pub instance: Gif2dInstance,
}

#[derive(Component)]
pub struct GifTimer {
    pub acc: Duration,
    pub delay: GifFrameDelay,
}

#[derive(Component)]
pub struct ImageIndex {
    pub index: u32,
}

#[derive(Component)]
pub struct ImageDirty;

// TODO / OPTIMIZE
// #[derive(Component)]
// pub struct ImageVisible;

#[derive(Component)]
pub struct ImageHovered;

#[derive(Component)]
pub struct ImageSelected;

// TODO - Find better name for this (and other above components)
#[derive(Component)]
pub struct ImageShown;

//====================================================================

#[derive(Component)]
pub struct ToRemove;

//====================================================================

#[derive(Borrow, BorrowInfo)]
pub struct ImageDirtier<'v> {
    entities: EntitiesViewMut<'v>,
    image: View<'v, Image>,
    dirty: ViewMut<'v, ImageDirty>,
}

impl ImageDirtier<'_> {
    #[inline]
    pub fn _mark_dirty(&mut self, id: EntityId) {
        self.entities.add_component(id, &mut self.dirty, ImageDirty);
    }

    pub fn mark_all_dirty(&mut self) {
        (&self.image)
            .iter()
            .with_id()
            .for_each(|(id, _)| self.entities.add_component(id, &mut self.dirty, ImageDirty));
    }
}

#[derive(Borrow, BorrowInfo)]
pub struct ImageCreator<'v> {
    pub entities: EntitiesViewMut<'v>,

    pub image: ViewMut<'v, Image>,
    pub pos: ViewMut<'v, Pos>,
    pub size: ViewMut<'v, ImageSize>,
    pub color: ViewMut<'v, Color>,
    pub std_image: ViewMut<'v, StandardImage>,
    pub gif_image: ViewMut<'v, GifImage>,
    pub meta: ViewMut<'v, ImageMeta>,

    pub gif_timer: ViewMut<'v, GifTimer>,
    pub dirty: ViewMut<'v, ImageDirty>,
}

impl ImageCreator<'_> {
    pub fn spawn_image(&mut self, image: StandardImage, meta: ImageMeta) -> EntityId {
        self.spawn_image_config(
            Pos::default(),
            ImageSize::default(),
            Color::default(),
            image,
            meta,
        )
    }

    pub fn spawn_image_config(
        &mut self,
        pos: Pos,
        size: ImageSize,
        color: Color,
        std_image: StandardImage,
        meta: ImageMeta,
    ) -> EntityId {
        self.entities.add_entity(
            (
                &mut self.image,
                &mut self.pos,
                &mut self.size,
                &mut self.color,
                &mut self.std_image,
                &mut self.meta,
                &mut self.dirty,
            ),
            (Image, pos, size, color, std_image, meta, ImageDirty),
        )
    }

    pub fn spawn_gif(
        &mut self,
        gif: GifImage,
        frame_delay: &Vec<Duration>,
        meta: ImageMeta,
    ) -> EntityId {
        self.entities.add_entity(
            (
                &mut self.image,
                &mut self.pos,
                &mut self.size,
                &mut self.color,
                &mut self.gif_image,
                &mut self.gif_timer,
                &mut self.meta,
                &mut self.dirty,
            ),
            (
                Image,
                Pos::default(),
                ImageSize::default(),
                Color::default(),
                gif,
                GifTimer {
                    acc: Duration::default(),
                    delay: GifFrameDelay::from_durations(frame_delay),
                },
                meta,
                ImageDirty,
            ),
        )
    }
}

//====================================================================

fn sys_remove_pending(mut all_storages: AllStoragesViewMut) {
    let ids = all_storages
        .borrow::<View<ToRemove>>()
        .unwrap()
        .iter()
        .with_id()
        .map(|(id, _)| id)
        .collect::<Vec<_>>();

    ids.into_iter().for_each(|id| {
        all_storages.delete_entity(id);
    });
}

fn sys_clear_dirty(mut vm_dirty: ViewMut<ImageDirty>) {
    vm_dirty.clear();
}

//====================================================================
