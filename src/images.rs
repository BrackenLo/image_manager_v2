//====================================================================

use shipyard::{
    AllStoragesViewMut, Borrow, BorrowInfo, Component, EntitiesViewMut, EntityId, IntoIter,
    IntoWithId, View, ViewMut,
};

use crate::renderer::texture_pipeline::TextureInstance;

//====================================================================

#[derive(Component, Default)]
pub(crate) struct ImagePos {
    pub x: f32,
    pub y: f32,
}

impl ImagePos {
    #[inline]
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
    #[inline]
    pub fn to_array(&self) -> [f32; 2] {
        [self.x, self.y]
    }
}

//--------------------------------------------------

#[derive(Component)]
pub(crate) struct ImageSize {
    pub width: f32,
    pub height: f32,
}

impl ImageSize {
    #[inline]
    pub fn new(width: f32, height: f32) -> Self {
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

//--------------------------------------------------

#[derive(Component)]
pub(crate) struct ImageColor {
    r: f32,
    g: f32,
    b: f32,
    a: f32,
}

impl ImageColor {
    #[inline]
    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }
    #[inline]
    pub fn to_array(&self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }
}

impl Default for ImageColor {
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
pub(crate) struct Image {
    pub id: u64,
    pub instance: TextureInstance,
}

#[derive(Component)]
pub(crate) struct ImageIndex {
    pub index: u32,
}

#[derive(Component)]
pub(crate) struct ImageDirty;

//====================================================================

#[derive(Component)]
pub(crate) struct ToRemove;

//====================================================================

#[derive(Borrow, BorrowInfo)]
pub(crate) struct ImageCreator<'v> {
    entities: EntitiesViewMut<'v>,

    pos: ViewMut<'v, ImagePos>,
    size: ViewMut<'v, ImageSize>,
    color: ViewMut<'v, ImageColor>,
    image: ViewMut<'v, Image>,
    index: ViewMut<'v, ImageIndex>,
    dirty: ViewMut<'v, ImageDirty>,

    remove: ViewMut<'v, ToRemove>,
}

impl ImageCreator<'_> {
    #[inline]
    pub fn _remove(&mut self, id: EntityId) {
        self.entities.add_component(id, &mut self.remove, ToRemove);
    }

    pub fn remove_all(&mut self) {
        (&self.pos, &self.size, &self.color, &self.image)
            .iter()
            .with_id()
            .for_each(|(id, _)| self.entities.add_component(id, &mut self.remove, ToRemove));
    }

    pub fn mark_all_dirty(&mut self) {
        log::debug!("Marking all images as dirty");
        (&self.image, &self.pos, &self.size, &self.color)
            .iter()
            .with_id()
            .for_each(|(id, _)| self.entities.add_component(id, &mut self.dirty, ImageDirty));
    }

    pub fn spawn(&mut self, image: Image, index: u32) -> EntityId {
        self.spawn_config(
            ImagePos::default(),
            ImageSize::default(),
            ImageColor::default(),
            image,
            index,
        )
    }

    pub fn spawn_config(
        &mut self,
        pos: ImagePos,
        size: ImageSize,
        color: ImageColor,
        image: Image,
        index: u32,
    ) -> EntityId {
        self.entities.add_entity(
            (
                &mut self.pos,
                &mut self.size,
                &mut self.color,
                &mut self.image,
                &mut self.index,
                &mut self.dirty,
            ),
            (pos, size, color, image, ImageIndex { index }, ImageDirty),
        )
    }
}

//====================================================================

pub(crate) fn sys_remove_pending(mut all_storages: AllStoragesViewMut) {
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

pub(crate) fn sys_clear_dirty(mut vm_dirty: ViewMut<ImageDirty>) {
    vm_dirty.clear();
}

//====================================================================
