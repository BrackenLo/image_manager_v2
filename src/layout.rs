//====================================================================

use shipyard::{IntoIter, Unique, View, ViewMut};

use crate::{
    images::{Image, ImageColor, ImageCreator, ImageDirty, ImageIndex, ImagePos, ImageSize},
    renderer::{texture_pipeline::RawTextureInstance, Queue},
    tools::{Rect, Res, ResMut},
    window::WindowSize,
};

//====================================================================

#[derive(Unique)]
pub(crate) struct LayoutManager {
    image_count: u32,

    columns: u32,
    tile_max_size: glam::Vec2,
    tile_spacing: glam::Vec2,
}

impl Default for LayoutManager {
    fn default() -> Self {
        Self {
            image_count: 0,
            columns: 1,
            tile_max_size: glam::vec2(200., 200.),
            tile_spacing: glam::vec2(10., 30.),
        }
    }
}

impl LayoutManager {
    pub fn next(&mut self) -> u32 {
        let next = self.image_count;
        self.image_count += 1;
        next
    }
}

#[derive(Unique, Default)]
pub struct ImageViewport(Rect);

//====================================================================

pub(crate) fn sys_resize_layout(
    size: Res<WindowSize>,
    mut layout: ResMut<LayoutManager>,
    mut viewport: ResMut<ImageViewport>,
    mut image_creator: ImageCreator,
) {
    viewport.0 = Rect::from_size(size.width() as f32, (size.height() as f32 - 200.).max(1.));

    layout.columns =
        (viewport.0.width as u32 / (layout.tile_max_size.x + layout.tile_spacing.x) as u32).max(1);

    image_creator.mark_all_dirty();
}

pub(crate) fn sys_order_images(
    layout: Res<LayoutManager>,

    mut vm_pos: ViewMut<ImagePos>,
    mut vm_size: ViewMut<ImageSize>,
    v_index: View<ImageIndex>,
    v_dirty: View<ImageDirty>,
) {
    if v_dirty.is_empty() {
        return;
    }

    let start_x = 0.;
    let start_y = -layout.tile_max_size.y / 2.;

    (&mut vm_pos, &mut vm_size, &v_index, &v_dirty)
        .iter()
        .for_each(|(pos, size, index, _)| {
            let x = start_x
                + (index.index % layout.columns) as f32
                    * (layout.tile_max_size.x + layout.tile_spacing.x);

            let y = start_y
                - (index.index / layout.columns) as f32
                    * (layout.tile_max_size.y + layout.tile_spacing.y);

            pos.x = x;
            pos.y = y;

            size.width = layout.tile_max_size.x;
            size.height = layout.tile_max_size.y;
        });
}

pub(crate) fn sys_rebuild_images(
    queue: Res<Queue>,

    v_pos: View<ImagePos>,
    v_size: View<ImageSize>,
    v_color: View<ImageColor>,
    v_image: View<Image>,
    v_dirty: View<ImageDirty>,
) {
    if v_dirty.is_empty() {
        return;
    }

    (&v_pos, &v_size, &v_color, &v_image, &v_dirty)
        .iter()
        .for_each(|(pos, size, color, image, _)| {
            image.instance.update(
                queue.inner(),
                RawTextureInstance {
                    pos: pos.to_array(),
                    size: size.to_array(),
                    color: color.to_array(),
                },
            )
        });
}

//====================================================================
