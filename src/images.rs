//====================================================================

use shipyard::{
    AllStoragesViewMut, Borrow, BorrowInfo, Component, EntitiesViewMut, EntityId, IntoIter,
    IntoWithId, View, ViewMut,
};

use crate::renderer::texture_pipeline::TextureInstance;

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
pub struct Image {
    pub id: u64,
    pub instance: TextureInstance,
}

#[derive(Component)]
pub struct ImageIndex {
    pub index: u32,
}

#[derive(Component)]
pub struct ImageDirty;

#[derive(Component)]
pub struct ImageHighlighted;

#[derive(Component)]
pub struct ImageSelected;

//====================================================================

#[derive(Component)]
pub struct ToRemove;

//====================================================================

#[derive(Borrow, BorrowInfo)]
pub struct ImageDirtier<'v> {
    entities: EntitiesViewMut<'v>,
    index: View<'v, ImageIndex>,
    dirty: ViewMut<'v, ImageDirty>,
}

impl ImageDirtier<'_> {
    #[inline]
    pub fn mark_dirty(&mut self, id: EntityId) {
        self.entities.add_component(id, &mut self.dirty, ImageDirty);
    }

    pub fn mark_all_dirty(&mut self) {
        (&self.index)
            .iter()
            .with_id()
            .for_each(|(id, _)| self.entities.add_component(id, &mut self.dirty, ImageDirty));
    }
}

#[derive(Borrow, BorrowInfo)]
pub struct ImageCreator<'v> {
    entities: EntitiesViewMut<'v>,

    pos: ViewMut<'v, Pos>,
    size: ViewMut<'v, ImageSize>,
    color: ViewMut<'v, Color>,
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

    pub fn _remove_all(&mut self) {
        (&self.pos, &self.size, &self.color, &self.image)
            .iter()
            .with_id()
            .for_each(|(id, _)| self.entities.add_component(id, &mut self.remove, ToRemove));
    }

    pub fn spawn(&mut self, image: Image, index: u32) -> EntityId {
        self.spawn_config(
            Pos::default(),
            ImageSize::default(),
            Color::default(),
            image,
            index,
        )
    }

    pub fn spawn_config(
        &mut self,
        pos: Pos,
        size: ImageSize,
        color: Color,
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
