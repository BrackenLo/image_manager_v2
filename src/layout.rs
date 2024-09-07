//====================================================================

use shipyard::{
    AllStoragesView, EntitiesView, EntityId, Get, IntoIter, IntoWithId, IntoWorkload, Remove,
    Unique, View, ViewMut, Workload,
};
use winit::{event::MouseButton, keyboard::KeyCode};

use crate::{
    app::Stages,
    images::{
        Color, ImageCreator, ImageDirtier, ImageDirty, ImageHovered, ImageIndex, ImageMeta,
        ImageSelected, ImageShown, ImageSize, Pos, StandardImage, ToRemove,
    },
    renderer::{
        camera::{Camera, MainCamera},
        texture_pipeline::{RawTextureInstance, TextureInstance, TexturePipeline},
        Device, Queue,
    },
    shipyard_tools::{Event, EventHandler, Plugin, Res, ResMut, UniqueTools},
    storage::Storage,
    tools::{aabb_point, Input, MouseInput, Time},
    window::{ResizeEvent, WindowSize},
};

//====================================================================

pub(crate) struct LayoutPlugin;

impl Plugin<Stages> for LayoutPlugin {
    fn build(&self, workload_builder: &mut crate::shipyard_tools::WorkloadBuilder<Stages>) {
        workload_builder
            .add_workload(
                Stages::Setup,
                Workload::new("").with_system(sys_setup_layout),
            )
            .add_workload(
                Stages::Update,
                Workload::new("")
                    .with_system(sys_navigate_layout)
                    .with_system(sys_hover_images)
                    .into_sequential_workload()
                    .with_system(sys_select_images),
            )
            .add_workload(
                Stages::PreRender,
                Workload::new("")
                    .with_system(sys_order_images)
                    .with_system(sys_rebuild_images)
                    .into_sequential_workload(),
            )
            //
            .add_event::<ResizeEvent>(
                Workload::new("")
                    .with_system(sys_resize_layout)
                    .with_system(sys_resize_selected),
            )
            .add_event::<SelectedEvent>(
                Workload::new("")
                    .with_workload(
                        (sys_set_layout_selected, sys_resize_layout).into_sequential_workload(),
                    )
                    .with_workload(
                        (sys_process_selected, sys_resize_selected).into_sequential_workload(),
                    ),
            );
    }
}

//====================================================================

#[derive(Unique)]
pub struct LayoutManager {
    image_count: u32,

    width: f32,
    columns: u32,
    tile_size: glam::Vec2,
    tile_spacing: glam::Vec2,

    max_tile_size: glam::Vec2,
    min_tile_size: glam::Vec2,

    selected: bool,
}

impl Default for LayoutManager {
    fn default() -> Self {
        Self {
            image_count: 0,
            width: 1.,
            columns: 1,
            tile_size: glam::vec2(200., 200.),
            tile_spacing: glam::vec2(10., 30.),

            max_tile_size: glam::vec2(500., 500.),
            min_tile_size: glam::vec2(80., 80.),

            selected: false,
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

#[derive(Unique)]
pub struct LayoutNavigation {
    scroll_mod: f32,
    move_speed: f32,
    move_mod: f32,
    zoom_speed: f32,
    zoom_mod: f32,
}

impl Default for LayoutNavigation {
    fn default() -> Self {
        Self {
            scroll_mod: 4.,
            move_speed: 800.,
            move_mod: 3.,
            zoom_speed: 120.,
            zoom_mod: 2.1,
        }
    }
}

//====================================================================

#[derive(Event)]
struct SelectedEvent {
    selected: Option<EntityId>,
}

//====================================================================

fn sys_setup_layout(all_storages: AllStoragesView) {
    all_storages
        .insert(LayoutManager::default())
        .insert(LayoutNavigation::default());
}

fn sys_resize_layout(
    size: Res<WindowSize>,
    mut layout: ResMut<LayoutManager>,
    mut image_dirtier: ImageDirtier,

    mut camera: ResMut<Camera<MainCamera>>,
) {
    layout.width = match layout.selected {
        true => size.width_f32() / 2.,
        false => size.width_f32(),
    };

    layout.columns =
        (layout.width as u32 / (layout.tile_size.x + layout.tile_spacing.x) as u32).max(1);

    image_dirtier.mark_all_dirty();

    let row_width = layout.columns as f32 * (layout.tile_size.x + layout.tile_spacing.x);

    let half_width = size.width_f32() / 2.;
    let half_height = size.height_f32() / 2.;

    camera.raw.left = -half_width;
    camera.raw.right = half_width;
    camera.raw.top = half_height;
    camera.raw.bottom = -half_height;

    camera.raw.translation.x = row_width / 2.;
}

//====================================================================

fn sys_order_images(
    layout: Res<LayoutManager>,
    size: Res<WindowSize>,

    mut vm_pos: ViewMut<Pos>,
    mut vm_size: ViewMut<ImageSize>,
    v_index: View<ImageIndex>,
    v_meta: View<ImageMeta>,
    v_dirty: View<ImageDirty>,
) {
    if v_dirty.is_empty() {
        return;
    }

    let offset_x = match layout.selected {
        true => -layout.width / 2.,
        false => 0.,
    };

    let start_x = (layout.tile_size.x + layout.tile_spacing.x) / 2. + offset_x;
    let start_y = size.height_f32() / 2. - layout.tile_size.y / 2.;

    (&mut vm_pos, &mut vm_size, &v_index, &v_meta, &v_dirty)
        .iter()
        .for_each(|(pos, size, index, meta, _)| {
            let x = start_x
                + (index.index % layout.columns) as f32
                    * (layout.tile_size.x + layout.tile_spacing.x);

            let y = start_y
                - (index.index / layout.columns) as f32
                    * (layout.tile_size.y + layout.tile_spacing.y);

            pos.x = x;
            pos.y = y;

            match meta.aspect < 1. {
                true => {
                    size.width = layout.tile_size.x;
                    size.height = layout.tile_size.y * meta.aspect;
                }
                false => {
                    size.width = layout.tile_size.x / meta.aspect;
                    size.height = layout.tile_size.y;
                }
            }
        });
}

fn sys_rebuild_images(
    queue: Res<Queue>,

    v_pos: View<Pos>,
    v_size: View<ImageSize>,
    v_color: View<Color>,
    v_std_image: View<StandardImage>,
    v_dirty: View<ImageDirty>,
) {
    if v_dirty.is_empty() {
        return;
    }

    (&v_pos, &v_size, &v_color, &v_std_image, &v_dirty)
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
    mut camera: ResMut<Camera<MainCamera>>,

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
    let mut y = (w as i8 - s as i8) as f32;
    if !ctrl {
        y += mouse.scroll().y * navigation.scroll_mod;
    }

    // Zooming in and out
    let r = keys.pressed(KeyCode::KeyR); // in
    let f = keys.pressed(KeyCode::KeyF); // out

    let mut zoom = (r as i8 - f as i8) as f32;
    if ctrl {
        zoom += mouse.scroll().y * navigation.scroll_mod;
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
            (layout.width as u32 / (layout.tile_size.x + layout.tile_spacing.x) as u32).max(1);

        let row_width = layout.columns as f32 * (layout.tile_size.x + layout.tile_spacing.x);
        camera.raw.translation.x = row_width / 2.;
    }

    if y != 0. {
        let delta = time.delta_seconds();

        let mut speed = navigation.move_speed;
        if shift {
            speed *= navigation.move_mod;
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

fn sys_hover_images(
    layout: Res<LayoutManager>,
    camera: Res<Camera<MainCamera>>,
    mouse: Res<MouseInput>,

    v_pos: View<Pos>,
    mut vm_color: ViewMut<Color>,
    v_index: View<ImageIndex>,

    entities: EntitiesView,
    mut vm_dirty: ViewMut<ImageDirty>,
    mut vm_hovered: ViewMut<ImageHovered>,
) {
    let mouse_pos = camera.raw.screen_to_camera(mouse.screen_pos());

    // Check already hovered images
    let to_remove = (&v_pos, &vm_hovered)
        .iter()
        .with_id()
        .filter_map(|(id, (pos, _))| {
            match aabb_point(mouse_pos, glam::vec2(pos.x, pos.y), layout.tile_size) {
                true => None,
                false => Some(id),
            }
        })
        .collect::<Vec<_>>();

    to_remove.into_iter().for_each(|id| {
        vm_hovered.remove(id);
        (&mut vm_color).get(id).unwrap().r = 1.;

        entities.add_component(id, &mut vm_dirty, ImageDirty);
    });

    // Find newly hovered images - use v_index to only select images part of grid
    let image = (&v_pos, &v_index, !&vm_hovered)
        .iter()
        .with_id()
        .find(|(_, (pos, _, _))| aabb_point(mouse_pos, glam::vec2(pos.x, pos.y), layout.tile_size));

    let id = match image {
        Some((id, _)) => id,
        None => return,
    };

    let mut color = (&mut vm_color).get(id).unwrap();
    color.r = 0.;

    entities.add_component(id, &mut vm_dirty, ImageDirty);
    entities.add_component(id, &mut vm_hovered, ImageHovered);
}

fn sys_select_images(
    mut events: ResMut<EventHandler>,
    key_input: Res<Input<KeyCode>>,
    mouse_input: Res<Input<MouseButton>>,

    entities: EntitiesView,
    v_hovered: View<ImageHovered>,
    mut vm_selected: ViewMut<ImageSelected>,
) {
    match (
        mouse_input.just_pressed(MouseButton::Left),
        mouse_input.just_pressed(MouseButton::Right) | key_input.just_pressed(KeyCode::Escape),
    ) {
        (false, true) => {
            events.add_event(SelectedEvent { selected: None });
            return;
        }
        (false, false) => return,
        _ => {}
    }

    if !mouse_input.just_pressed(MouseButton::Left) {
        return;
    }

    let hovered = v_hovered.iter().with_id().next();

    let id = match hovered {
        Some((id, _)) => id,
        None => return,
    };

    log::debug!("New image selected with id '{:?}'", id);

    // TODO - Set color of selected image (or not idk)
    vm_selected.clear();
    entities.add_component(id, &mut vm_selected, ImageSelected);

    events.add_event(SelectedEvent { selected: Some(id) });
}

fn sys_process_selected(
    events: Res<EventHandler>,
    device: Res<Device>,
    pipeline: Res<TexturePipeline>,
    storage: Res<Storage>,

    mut image_creator: ImageCreator,
    mut vm_shown: ViewMut<ImageShown>,

    mut vm_remove: ViewMut<ToRemove>,
) {
    let event = events.get_event::<SelectedEvent>().unwrap();

    // Remove all existing shown images
    vm_shown.iter().with_id().for_each(|(id, _)| {
        image_creator
            .entities
            .add_component(id, &mut vm_remove, ToRemove)
    });

    let id = match event.selected {
        Some(id) => id,
        None => return,
    };

    let original_image = image_creator.std_image.get(id).unwrap();
    let texture = storage.get_texture(original_image.id).unwrap();

    let image = StandardImage {
        id: original_image.id,
        instance: TextureInstance::new(
            device.inner(),
            &pipeline,
            RawTextureInstance::default(),
            &texture.texture,
        ),
    };

    let meta = ImageMeta {
        texture_resolution: texture.resolution,
        aspect: texture.resolution.height as f32 / texture.resolution.width as f32,
    };

    let entity_id = image_creator.spawn_image(image, meta);
    image_creator
        .entities
        .add_component(entity_id, &mut vm_shown, ImageShown);
}

fn sys_set_layout_selected(events: Res<EventHandler>, mut layout: ResMut<LayoutManager>) {
    let event = events.get_event::<SelectedEvent>().unwrap();

    match event.selected {
        Some(_) => layout.selected = true,
        None => layout.selected = false,
    }
}

fn sys_resize_selected(
    window_size: Res<WindowSize>,
    v_shown: View<ImageShown>,
    mut vm_pos: ViewMut<Pos>,
    mut vm_size: ViewMut<ImageSize>,
    v_meta: View<ImageMeta>,
) {
    (&v_shown, &mut vm_pos, &mut vm_size, &v_meta)
        .iter()
        .for_each(|(_, pos, size, meta)| {
            let half_width = window_size.width_f32() / 2.;

            pos.x = half_width / 2.;
            pos.y = 0.;

            match meta.aspect < 1. {
                true => {
                    size.width = half_width;
                    size.height = window_size.height_f32() * meta.aspect;
                }
                false => {
                    size.width = half_width / meta.aspect;
                    size.height = window_size.height_f32();
                }
            }
        });
}

//====================================================================
