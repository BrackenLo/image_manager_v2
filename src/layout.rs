//====================================================================

use shipyard::{IntoIter, Unique, View, ViewMut};
use winit::keyboard::KeyCode;

use crate::{
    images::{Image, ImageColor, ImageDirtier, ImageDirty, ImageIndex, ImagePos, ImageSize},
    renderer::{camera::MainCamera, texture_pipeline::RawTextureInstance, Queue},
    tools::{Input, MouseInput, Rect, Res, ResMut, Time},
    window::WindowSize,
};

//====================================================================

#[derive(Unique)]
pub(crate) struct LayoutManager {
    image_count: u32,

    columns: u32,
    tile_size: glam::Vec2,
    tile_spacing: glam::Vec2,

    max_tile_size: glam::Vec2,
    min_tile_size: glam::Vec2,
}

impl Default for LayoutManager {
    fn default() -> Self {
        Self {
            image_count: 0,
            columns: 1,
            tile_size: glam::vec2(200., 200.),
            tile_spacing: glam::vec2(10., 30.),

            max_tile_size: glam::vec2(500., 500.),
            min_tile_size: glam::vec2(80., 80.),
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

impl ImageViewport {
    #[inline]
    pub fn inner(&self) -> &Rect {
        &self.0
    }
}

#[derive(Unique)]
pub struct LayoutNavigation {
    scroll_speed: f32,
    scroll_mod: f32,
    zoom_speed: f32,
    zoom_mod: f32,
}

impl Default for LayoutNavigation {
    fn default() -> Self {
        Self {
            scroll_speed: 800.,
            scroll_mod: 3.,
            zoom_speed: 120.,
            zoom_mod: 2.1,
        }
    }
}

//====================================================================

pub(crate) fn sys_resize_layout(
    size: Res<WindowSize>,
    mut layout: ResMut<LayoutManager>,
    mut viewport: ResMut<ImageViewport>,
    mut image_dirtier: ImageDirtier,

    mut camera: ResMut<MainCamera>,
) {
    viewport.0 = Rect::from_size(size.width() as f32, (size.height() as f32 - 200.).max(1.));

    layout.columns =
        (viewport.0.width as u32 / (layout.tile_size.x + layout.tile_spacing.x) as u32).max(1);

    image_dirtier.mark_all_dirty();

    let row_width = layout.columns as f32 * (layout.tile_size.x + layout.tile_spacing.x);

    camera.raw.translation.x = -(row_width / 2.);
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

    let start_x = (layout.tile_size.x + layout.tile_spacing.x) / 2.;
    let start_y = -layout.tile_size.y / 2.;

    (&mut vm_pos, &mut vm_size, &v_index, &v_dirty)
        .iter()
        .for_each(|(pos, size, index, _)| {
            let x = start_x
                + (index.index % layout.columns) as f32
                    * (layout.tile_size.x + layout.tile_spacing.x);

            let y = start_y
                - (index.index / layout.columns) as f32
                    * (layout.tile_size.y + layout.tile_spacing.y);

            pos.x = x;
            pos.y = y;

            size.width = layout.tile_size.x;
            size.height = layout.tile_size.y;
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

pub(crate) fn sys_navigate_layout(
    mut layout: ResMut<LayoutManager>,
    navigation: Res<LayoutNavigation>,
    viewport: Res<ImageViewport>,
    mut camera: ResMut<MainCamera>,

    keys: Res<Input<KeyCode>>,
    mouse: Res<MouseInput>,
    time: Res<Time>,

    mut image_dirtier: ImageDirtier,
) {
    // Mods
    let shift = keys.pressed(KeyCode::ShiftLeft);
    let ctrl = keys.pressed(KeyCode::ControlLeft);

    // Move
    let w = keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::KeyK);
    let s = keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::KeyJ);
    let mut y = (s as i8 - w as i8) as f32;
    if !ctrl {
        y -= mouse.scroll().y * 1.4;
    }

    // Zooming in and out
    let r = keys.pressed(KeyCode::KeyR); // in
    let f = keys.pressed(KeyCode::KeyF); // out

    let mut zoom = (r as i8 - f as i8) as f32;
    if ctrl {
        zoom += mouse.scroll().y * 2.;
    }

    if zoom != 0. {
        let mut zoom_speed = zoom * navigation.zoom_speed;
        if shift {
            zoom_speed *= navigation.zoom_mod;
        }

        let speed = glam::vec2(zoom_speed, zoom_speed) * time.delta_seconds();

        layout.tile_size += speed;
        layout.tile_size = layout
            .tile_size
            .clamp(layout.min_tile_size, layout.max_tile_size);

        image_dirtier.mark_all_dirty();

        layout.columns =
            (viewport.0.width as u32 / (layout.tile_size.x + layout.tile_spacing.x) as u32).max(1);

        let row_width = layout.columns as f32 * (layout.tile_size.x + layout.tile_spacing.x);
        camera.raw.translation.x = -(row_width / 2.);

        // log::debug!("new tile size '{}'", layout.format.tile_max_size);
    }

    if y != 0. {
        let delta = time.delta_seconds();

        let mut speed = navigation.scroll_speed;
        if shift {
            speed *= navigation.scroll_mod;
        }

        camera.raw.translation.y += y * delta * speed;

        let last_column = (layout.image_count / layout.columns) as f32
            * (layout.tile_size.y + layout.tile_spacing.y);

        let max_y = last_column;
        let min_y = layout.tile_size.y * -0.8;

        camera.raw.translation.y = camera.raw.translation.y.clamp(min_y, max_y);
    }
}

//====================================================================
