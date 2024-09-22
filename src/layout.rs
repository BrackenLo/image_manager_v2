//====================================================================

use cabat::{
    common::{WindowResizeEvent, WindowSize},
    renderer::{
        text2d_pipeline::{Metrics, TextBuffer, TextPipeline},
        Device, Queue,
    },
    runner::tools::{Input, KeyCode, MouseButton, MouseInput, Time},
    shipyard_tools::{prelude::*, UniqueTools},
};
use shipyard::{
    AllStoragesView, EntitiesView, EntityId, Get, IntoIter, IntoWithId, IntoWorkload, Remove,
    Unique, View, ViewMut,
};

use crate::{
    images::{
        Color, GifImage, GifTimer, ImageCreator, ImageDirtier, ImageDirty, ImageHovered,
        ImageIndex, ImageMeta, ImageSelected, ImageShown, ImageSize, Pos, StandardImage, ToRemove,
    },
    renderer::{
        camera::MainCamera,
        gif2d_pipeline::{Gif2dInstance, Gif2dInstanceRaw, Gif2dPipeline},
        texture2d_pipeline::{Texture2dInstance, Texture2dInstanceRaw, Texture2dPipeline},
    },
    storage::Storage,
    tools::aabb_point,
};

//====================================================================

pub(crate) struct LayoutPlugin;

impl Plugin for LayoutPlugin {
    fn build(self, workload_builder: WorkloadBuilder) -> WorkloadBuilder {
        workload_builder
            .add_workload(Stages::Setup, (sys_setup_layout).into_workload())
            .add_workload(
                Stages::Update,
                (
                    (sys_navigate_layout, sys_hover_images).into_sequential_workload(),
                    sys_select_images,
                )
                    .into_workload(),
            )
            .add_workload_sub(
                Stages::Update,
                SubStages::Post,
                (
                    sys_order_images,
                    sys_rebuild_images,
                    sys_tick_gifs,
                    sys_rebuild_gifs,
                    sys_reposition_text_dirty,
                    // sys_debug_layout,
                )
                    .into_sequential_workload(),
            )
            //
            .add_event::<WindowResizeEvent>(
                (sys_resize_layout, sys_resize_selected, sys_reposition_text)
                    .into_sequential_workload(),
            )
            .add_event::<SelectedEvent>(
                (
                    (sys_set_layout_selected, sys_resize_layout).into_sequential_workload(),
                    (sys_process_selected, sys_resize_selected).into_sequential_workload(),
                )
                    .into_workload(),
            )
            .add_event::<ScrollEvent>((sys_reposition_text).into_workload())
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
            tile_spacing: glam::vec2(10., 60.),

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

#[derive(Event)]
struct ScrollEvent;

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

    mut camera: ResMut<MainCamera>,
) {
    layout.width = match layout.selected {
        true => size.width_f32() / 2.,
        false => size.width_f32(),
    };

    layout.columns =
        (layout.width as u32 / (layout.tile_size.x + layout.tile_spacing.x) as u32).max(1);

    image_dirtier.mark_all_dirty();

    let half_width = size.width_f32() / 2.;
    let half_height = size.height_f32() / 2.;

    camera.raw.left = -half_width;
    camera.raw.right = half_width;
    camera.raw.top = half_height;
    camera.raw.bottom = -half_height;

    // camera.raw.translation.x = row_width / 2.;
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

    let row_width = layout.columns as f32 * (layout.tile_size.x + layout.tile_spacing.x);

    let start_x = (layout.tile_size.x + layout.tile_spacing.x) / 2. + offset_x + -row_width / 2.;
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

            let wratio = layout.tile_size.x / meta.texture_resolution.width as f32;
            let hratio = layout.tile_size.y / meta.texture_resolution.height as f32;
            let ratio = f32::min(wratio, hratio);

            size.width = meta.texture_resolution.width as f32 * ratio;
            size.height = meta.texture_resolution.height as f32 * ratio;
        });
}

fn sys_rebuild_images(
    queue: Res<Queue>,

    v_pos: View<Pos>,
    v_size: View<ImageSize>,
    v_color: View<Color>,
    v_image: View<StandardImage>,
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
                Texture2dInstanceRaw {
                    pos: pos.to_array(),
                    size: size.to_array(),
                    color: color.to_array(),
                },
            )
        });
}

fn sys_rebuild_gifs(
    queue: Res<Queue>,

    v_pos: View<Pos>,
    v_size: View<ImageSize>,
    v_color: View<Color>,
    v_gif: View<GifImage>,
    v_dirty: View<ImageDirty>,
) {
    if v_dirty.is_empty() {
        return;
    }

    (&v_pos, &v_size, &v_color, &v_gif, &v_dirty)
        .iter()
        .for_each(|(pos, size, color, gif, _)| {
            gif.instance.update(
                queue.inner(),
                Gif2dInstanceRaw {
                    pos: pos.to_array(),
                    size: size.to_array(),
                    color: color.to_array(),
                    frame_x: (gif.frame % gif.frames_per_row) as f32,
                    frame_y: (gif.frame / gif.frames_per_row) as f32,
                    ..Default::default()
                },
            )
        });
}

fn sys_tick_gifs(
    entities: EntitiesView,
    time: Res<Time>,
    mut vm_gif: ViewMut<GifImage>,
    mut vm_gif_timer: ViewMut<GifTimer>,
    mut vm_dirty: ViewMut<ImageDirty>,
) {
    (&mut vm_gif, &mut vm_gif_timer)
        .iter()
        .with_id()
        .for_each(|(id, (gif, timer))| {
            timer.acc += *time.delta();

            // let delay = timer.delay[gif.frame as usize];
            let delay = timer.delay.get_delay(&gif.frame);

            if timer.acc > delay {
                timer.acc = std::time::Duration::ZERO;
                gif.frame = gif.frame + 1;
                if gif.frame >= gif.total_frames {
                    gif.frame = 0;
                }

                entities.add_component(id, &mut vm_dirty, ImageDirty);
            }
        });
}

fn sys_reposition_text_dirty(
    layout: Res<LayoutManager>,
    size: Res<WindowSize>,
    camera: Res<MainCamera>,
    mut pipeline: ResMut<TextPipeline>,

    v_pos: View<Pos>,
    v_index: View<ImageIndex>,
    mut vm_text: ViewMut<TextBuffer>,
    v_dirty: View<ImageDirty>,
) {
    if v_dirty.is_empty() {
        return;
    }

    let top = 0;
    let bottom = size.height() as i32;
    let left = 0;
    let right = size.width() as i32;

    let start_x = camera.raw.translation.x + size.width_f32() / 2. - layout.tile_size.x / 2.;
    let start_y = camera.raw.translation.y + size.height_f32() / 2. + layout.tile_size.y / 2.;

    let font_scale = (layout.tile_size.x / layout.max_tile_size.x) * 30. + 2.;

    (&v_pos, &v_index, &mut vm_text, &v_dirty)
        .iter()
        .for_each(|(pos, _, text, _)| {
            text.pos.0 = start_x + pos.x;
            text.pos.1 = start_y - pos.y;

            text.bounds.top = top;
            text.bounds.bottom = bottom;
            text.bounds.left = left;
            text.bounds.right = right;

            text.set_metrics_and_size(
                &mut pipeline,
                Metrics::relative(font_scale, 1.2),
                Some(layout.tile_size.x),
                Some(layout.tile_spacing.y),
            );
        });
}

fn sys_reposition_text(
    layout: Res<LayoutManager>,
    size: Res<WindowSize>,
    camera: Res<MainCamera>,
    mut pipeline: ResMut<TextPipeline>,

    v_pos: View<Pos>,
    v_index: View<ImageIndex>,
    mut vm_text: ViewMut<TextBuffer>,
) {
    let top = 0;
    let bottom = size.height() as i32;
    let left = 0;
    let right = size.width() as i32;

    let start_x = camera.raw.translation.x + size.width_f32() / 2. - layout.tile_size.x / 2.;
    let start_y = camera.raw.translation.y + size.height_f32() / 2. + layout.tile_size.y / 2.;

    let font_scale = (layout.tile_size.x / layout.max_tile_size.x) * 30. + 2.;

    (&v_pos, &v_index, &mut vm_text)
        .iter()
        .for_each(|(pos, _, text)| {
            text.pos.0 = start_x + pos.x;
            text.pos.1 = start_y - pos.y;

            text.bounds.top = top;
            text.bounds.bottom = bottom;
            text.bounds.left = left;
            text.bounds.right = right;

            text.set_metrics_and_size(
                &mut pipeline,
                Metrics::relative(font_scale, 1.2),
                Some(layout.tile_size.x),
                Some(layout.tile_spacing.y),
            );
        });
}

//====================================================================

// TODO / OPTIMIZE - Only render text and images that are visible
// fn sys_set_visiblity(
//     layout: Res<LayoutManager>,
//     camera: Res<Camera<MainCamera>>,
//     v_index: View<ImageIndex>,
//     vm_visible: ViewMut<ImageVisible>,
// ) {
//     let top = camera.raw.translation.y + camera.raw.top;
//     let bottom = camera.raw.translation.y + camera.raw.bottom;

// }

//====================================================================

// fn sys_debug_layout(
//     window_size: Res<WindowSize>,
//     layout: Res<LayoutManager>,
//     camera: Res<Camera<MainCamera>>,
//     mut debug_circles: ResMut<DebugCircles>,
// ) {
//     let cam_top = camera.raw.translation.y + camera.raw.top;
//     let top_row = f32::floor(cam_top / (layout.tile_size.y + layout.tile_spacing.y)) - 2.;
//     let top_row_start_index = layout.columns as f32 * top_row;

//     let top_row_debug_pos = top_row * (layout.tile_size.y + layout.tile_spacing.y);

//     // let new_top_row = top_row_start_index / layout.columns;
//     // let top_row_pos = (new_top_row as f32 * -1.) * (layout.tile_size.y + layout.tile_spacing.y);

//     let start_y = window_size.height_f32() / 2. - layout.tile_size.y / 2.;

//     println!(
//         "Top row = {}, top row start index = {}, top row debug pos = {}",
//         top_row, top_row_start_index, top_row_debug_pos
//     );

//     debug_circles.to_spawn.push((
//         0.,
//         // top_row_pos,
//         // start_y - top_row,
//         start_y + top_row_debug_pos,
//         // cam_top,
//         [1., 0., 0., 1.],
//         Duration::from_secs_f32(0.1),
//     ));

//     debug_circles.to_spawn.push((
//         camera.raw.translation.x,
//         camera.raw.translation.y,
//         [0., 0., 1., 1.],
//         Duration::from_secs_f32(0.1),
//     ));
// }

fn sys_navigate_layout(
    mut events: ResMut<EventHandler>,

    window_size: Res<WindowSize>,
    mut layout: ResMut<LayoutManager>,
    navigation: Res<LayoutNavigation>,
    mut camera: ResMut<MainCamera>,

    keys: Res<Input<KeyCode>>,
    mouse: Res<MouseInput>,
    time: Res<Time>,

    mut image_dirtier: ImageDirtier,
) {
    // // DEBUG
    // let a = keys.pressed(KeyCode::KeyA);
    // let d = keys.pressed(KeyCode::KeyD);
    // let x = (a as i8 - d as i8) as f32 * 40.;
    // camera.raw.translation.x += x;

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

        // Store the current top row index
        let cam_top = camera.raw.translation.y + camera.raw.top;
        let top_row = f32::floor(cam_top / (layout.tile_size.y + layout.tile_spacing.y)) - 2.;
        let top_row_start_index = layout.columns as f32 * top_row;

        //

        let speed = glam::vec2(zoom_speed, zoom_speed) * time.delta_seconds();

        layout.tile_size += speed;
        layout.tile_size = layout
            .tile_size
            .clamp(layout.min_tile_size, layout.max_tile_size);

        image_dirtier.mark_all_dirty();

        //

        let prev_columns = layout.columns;

        layout.columns =
            (layout.width as u32 / (layout.tile_size.x + layout.tile_spacing.x) as u32).max(1);

        if prev_columns != layout.columns {
            let start_y = window_size.height_f32() / 2. - layout.tile_size.y / 2.;

            let sub = match prev_columns > layout.columns {
                true => 1.,
                false => 0.,
            };

            let new_top_row = f32::ceil(top_row_start_index / layout.columns as f32) + sub;
            let new_top_row_pos = new_top_row * (layout.tile_size.y + layout.tile_spacing.y);

            camera.raw.translation.y = start_y + new_top_row_pos - camera.raw.top;
        }
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

        events.add_event(ScrollEvent);
    }
}

//====================================================================

fn sys_hover_images(
    layout: Res<LayoutManager>,
    camera: Res<MainCamera>,
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
    texture_pipeline: Res<Texture2dPipeline>,
    gif_pipeline: Res<Gif2dPipeline>,
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

    // let original_image = image_creator.std_image.get(id).unwrap();
    // let texture = storage.get_texture(original_image.id).unwrap();

    let id = if let Ok(img) = image_creator.std_image.get(id) {
        img.id
    } else if let Ok(gif) = image_creator.gif_image.get(id) {
        gif.id
    } else {
        log::warn!("Selected image doesn't have an assosiated texture id");
        return;
    };

    let texture = storage.get_texture(id).unwrap();

    let meta = ImageMeta {
        texture_resolution: texture.resolution,
    };

    let entity_id = match &texture.texture {
        crate::storage::TextureType::Texture(texture) => {
            let image = StandardImage {
                id,
                instance: Texture2dInstance::new(
                    device.inner(),
                    &texture_pipeline,
                    Texture2dInstanceRaw::default(),
                    &texture,
                ),
            };

            image_creator.spawn_image(image, meta)
        }
        crate::storage::TextureType::Gif { gif, frames } => {
            let gif = GifImage {
                id,
                frame: 0,
                total_frames: gif.total_frames,
                frames_per_row: gif.frames_per_row,
                instance: Gif2dInstance::new(
                    device.inner(),
                    &gif_pipeline,
                    Gif2dInstanceRaw::default(),
                    gif,
                ),
            };

            image_creator.spawn_gif(gif, frames, meta)
        }
    };

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
            let selected_viewport_width = window_size.width_f32() / 2.;
            let selected_viewport_height = window_size.height_f32();

            let wratio = selected_viewport_width / meta.texture_resolution.width as f32;
            let hratio = selected_viewport_height / meta.texture_resolution.height as f32;
            let ratio = f32::min(wratio, hratio);

            size.width = meta.texture_resolution.width as f32 * ratio;
            size.height = meta.texture_resolution.height as f32 * ratio;

            pos.x = selected_viewport_width / 2.;
            pos.y = 0.;
        });
}

//====================================================================
