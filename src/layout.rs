//====================================================================

use shipyard::{
    AllStoragesView, EntitiesView, Get, IntoIter, IntoWithId, IntoWorkload, Remove, Unique, View,
    ViewMut, Workload,
};
use winit::keyboard::KeyCode;

use crate::{
    images::{Color, Image, ImageDirtier, ImageDirty, ImageIndex, ImageSelected, ImageSize, Pos},
    renderer::{camera::MainCamera, texture_pipeline::RawTextureInstance, Queue},
    shipyard_tools::{Plugin, Res, ResMut, Stages, UniqueTools},
    tools::{aabb_point, Input, MouseInput, Rect, Time},
    window::WindowSize,
};

//====================================================================

pub(crate) struct LayoutPlugin;

impl Plugin for LayoutPlugin {
    fn build(&self, workload_builder: &mut crate::shipyard_tools::WorkloadBuilder) {
        workload_builder
            .add_workload(
                Stages::Setup,
                Workload::new("").with_system(sys_setup_layout),
            )
            .add_workload(
                Stages::Update,
                Workload::new("")
                    .with_system(sys_navigate_layout)
                    .with_system(sys_select_images)
                    .into_sequential_workload(),
            )
            .add_workload(
                Stages::PreRender,
                Workload::new("")
                    .with_system(sys_order_images)
                    .with_system(sys_rebuild_images)
                    .into_sequential_workload(),
            )
            .add_workload(
                Stages::Resize,
                Workload::new("").with_system(sys_resize_layout),
            );
    }
}

//====================================================================

#[derive(Unique)]
pub struct LayoutManager {
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

fn sys_setup_layout(all_storages: AllStoragesView) {
    all_storages
        .insert(LayoutManager::default())
        .insert(LayoutNavigation::default())
        .insert(ImageViewport::default());
}

fn sys_resize_layout(
    size: Res<WindowSize>,
    mut layout: ResMut<LayoutManager>,
    mut viewport: ResMut<ImageViewport>,
    mut image_dirtier: ImageDirtier,

    mut camera: ResMut<MainCamera>,
) {
    viewport.0 = Rect::from_size(size.width() as f32, (size.height() as f32 - 200.).max(1.));
    // viewport.0 = Rect::new(
    //     0.,
    //     100.,
    //     // 0.,
    //     size.width() as f32,
    //     (size.height() as f32 - 300.).max(1.),
    // );

    layout.columns =
        (viewport.0.width as u32 / (layout.tile_size.x + layout.tile_spacing.x) as u32).max(1);

    image_dirtier.mark_all_dirty();

    let row_width = layout.columns as f32 * (layout.tile_size.x + layout.tile_spacing.x);

    let half_width = size.width() as f32 / 2.;
    let half_height = size.height() as f32 / 2.;

    camera.raw.left = -half_width;
    camera.raw.right = half_width;
    // camera.raw.top = 0.;
    // camera.raw.bottom = -(size.height() as f32);
    // camera.raw.top = half_height;
    // camera.raw.bottom = -half_height + 300.;
    camera.raw.top = half_height;
    camera.raw.bottom = -half_height;

    camera.raw.translation.x = row_width / 2.;
}

fn sys_order_images(
    layout: Res<LayoutManager>,

    mut vm_pos: ViewMut<Pos>,
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

            // println!("Position at {}, {}", pos.x, pos.y);
        });
}

fn sys_rebuild_images(
    queue: Res<Queue>,

    v_pos: View<Pos>,
    v_size: View<ImageSize>,
    v_color: View<Color>,
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

fn sys_navigate_layout(
    mut layout: ResMut<LayoutManager>,
    navigation: Res<LayoutNavigation>,
    viewport: Res<ImageViewport>,
    mut camera: ResMut<MainCamera>,

    keys: Res<Input<KeyCode>>,
    mouse: Res<MouseInput>,
    time: Res<Time>,

    mut image_dirtier: ImageDirtier,
) {
    // DEBUG
    // let a = keys.pressed(KeyCode::KeyA);
    // let d = keys.pressed(KeyCode::KeyD);
    // let x = (a as i8 - d as i8) as f32;

    // if x != 0. {
    //     camera.raw.translation.x += x * 600. * time.delta_seconds();
    // }

    // Mods
    let shift = keys.pressed(KeyCode::ShiftLeft);
    let ctrl = keys.pressed(KeyCode::ControlLeft);

    // Move
    let w = keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::KeyK);
    let s = keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::KeyJ);
    let mut y = (w as i8 - s as i8) as f32;
    if !ctrl {
        y += mouse.scroll().y * 1.4;
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
        camera.raw.translation.x = row_width / 2.;

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
            * (layout.tile_size.y + layout.tile_spacing.y)
            * -1.;

        let min_y = last_column;
        let max_y = layout.tile_size.y * 0.8;

        camera.raw.translation.y = camera.raw.translation.y.clamp(min_y, max_y);
    }
}

//====================================================================

fn sys_select_images(
    layout: Res<LayoutManager>,
    camera: Res<MainCamera>,
    mouse: Res<MouseInput>,

    v_pos: View<Pos>,
    mut vm_color: ViewMut<Color>,
    v_index: View<ImageIndex>,

    entities: EntitiesView,
    mut vm_dirty: ViewMut<ImageDirty>,
    mut vm_selected: ViewMut<ImageSelected>,
) {
    let mouse_pos = camera.raw.screen_to_camera(mouse.screen_pos());

    // Check already selected images
    let to_remove = (&v_pos, &vm_selected)
        .iter()
        .with_id()
        .filter_map(|(id, (pos, _))| {
            match aabb_point(
                // mouse.screen_pos(),
                mouse_pos,
                glam::vec2(pos.x, pos.y),
                layout.tile_size,
            ) {
                true => None,
                false => Some(id),
            }
        })
        .collect::<Vec<_>>();

    to_remove.into_iter().for_each(|id| {
        vm_selected.remove(id);
        (&mut vm_color).get(id).unwrap().r = 1.;

        entities.add_component(id, &mut vm_dirty, ImageDirty);
    });

    // Find newly selected images
    let image = (&v_pos, &v_index, !&vm_selected)
        .iter()
        .with_id()
        .find(|(_, (pos, _, _))| {
            aabb_point(
                // mouse.screen_pos(),
                mouse_pos,
                glam::vec2(pos.x, pos.y),
                layout.tile_size,
            )
        });

    let id = match image {
        Some((id, _)) => id,
        None => return,
    };

    // println!("Found at {:?}", v_pos.get(id).unwrap().to_array());

    let mut color = (&mut vm_color).get(id).unwrap();
    color.r = 0.;

    entities.add_component(id, &mut vm_dirty, ImageDirty);
    entities.add_component(id, &mut vm_selected, ImageSelected);
}

//====================================================================
