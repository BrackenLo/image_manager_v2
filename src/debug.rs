//====================================================================

use std::time::Duration;

use cabat::{
    renderer::text2d_pipeline::{TextBuffer, TextBufferDescriptor, TextPipeline},
    runner::tools::{MouseInput, Time},
    shipyard_tools::prelude::*,
};
use shipyard::{
    AllStoragesView, AllStoragesViewMut, EntitiesViewMut, EntityId, Get, IntoWorkload, Unique,
    ViewMut,
};

use crate::{
    images::Pos,
    renderer::{camera::MainCamera, circle_pipeline::Circle},
};

//====================================================================

pub(crate) struct DebugPlugin;

impl Plugin for DebugPlugin {
    fn build(self, workload_builder: WorkloadBuilder) -> WorkloadBuilder {
        workload_builder
            .add_workload(
                Stages::Setup,
                (
                    sys_setup_debug,
                    sys_setup_mouse_tracker,
                    sys_setup_debug_circles,
                )
                    .into_workload(),
            )
            .add_workload_sub(
                Stages::Setup,
                SubStages::Post,
                (sys_display_memory_usage).into_workload(),
            )
            .add_workload_sub(
                Stages::Update,
                SubStages::Pre,
                (
                    sys_tick_upkeep,
                    sys_update_mouse_tracker,
                    sys_spawn_debug_circles,
                    sys_despawn_debug_circles,
                )
                    .into_workload(),
            )
    }
}

fn sys_setup_debug(all_storages: AllStoragesView) {
    all_storages.add_unique(Upkeep::new());
}

fn sys_display_memory_usage(all_storages: AllStoragesView) {
    log::debug!("Memory Usage:\n{:#?}", all_storages.memory_usage());
}

//====================================================================

#[derive(Unique)]
pub struct Upkeep {
    second_tracker: f32,
    frame_count_this_second: u16,

    fps_list: [u16; Self::FPS_RECORD_SIZE],
    fps_instance_counter: usize,
    fps_sum: u32,
}

impl Upkeep {
    const FPS_RECORD_SIZE: usize = 6;

    pub fn new() -> Self {
        Self {
            second_tracker: 0.,
            frame_count_this_second: 0,

            fps_list: [0; 6],
            fps_instance_counter: 0,
            fps_sum: 0,
        }
    }

    fn tick(&mut self, delta: f32, output: bool) {
        self.frame_count_this_second += 1;

        self.second_tracker += delta;

        if self.second_tracker > 1. {
            self.fps_sum -= self.fps_list[self.fps_instance_counter] as u32;
            self.fps_sum += self.frame_count_this_second as u32;
            self.fps_list[self.fps_instance_counter] = self.frame_count_this_second;
            self.fps_instance_counter = (self.fps_instance_counter + 1) % Self::FPS_RECORD_SIZE;

            self.frame_count_this_second = 0;
            self.second_tracker -= 1.;

            if output {
                let avg = self.fps_sum / Self::FPS_RECORD_SIZE as u32;
                println!("Avg fps: {}", avg);
            }
        }
    }
}

fn sys_tick_upkeep(mut upkeep: ResMut<Upkeep>, time: Res<Time>) {
    upkeep.tick(time.delta_seconds(), false);
}

//====================================================================

#[derive(Unique)]
pub struct MouseTracker {
    text_id: EntityId,
    circle_id: EntityId,
}

fn sys_setup_mouse_tracker(
    all_storages: AllStoragesView,
    mut entities: EntitiesViewMut,

    mut text: ResMut<TextPipeline>,
    mut vm_text_buffer: ViewMut<TextBuffer>,

    mut vm_circles: ViewMut<Circle>,
    mut vm_pos: ViewMut<Pos>,
) {
    let text_id = entities.add_entity(
        &mut vm_text_buffer,
        TextBuffer::new(
            &mut text,
            &TextBufferDescriptor {
                // bounds: todo!(),
                // width: todo!(),
                // height: todo!(),
                ..Default::default()
            },
        ),
    );

    let circle_id = entities.add_entity(
        (&mut vm_circles, &mut vm_pos),
        (
            Circle {
                radius: 5.,
                color: [0., 0., 0., 1.],
            },
            Pos { x: 0., y: 0. },
        ),
    );

    let tracker = MouseTracker { text_id, circle_id };

    all_storages.add_unique(tracker);
}

fn sys_update_mouse_tracker(
    tracker: ResMut<MouseTracker>,
    camera: Res<MainCamera>,
    mouse: Res<MouseInput>,

    mut text_pipeline: ResMut<TextPipeline>,
    mut vm_text_buffer: ViewMut<TextBuffer>,
    mut vm_pos: ViewMut<Pos>,
) {
    let mouse_pos = camera.raw.screen_to_camera(mouse.screen_pos());

    let text = format!(
        "mouse_pos = {}, screen_pos = {}\n camera_pos = {}, final_pos = {}",
        mouse._pos().trunc(),
        mouse.screen_pos().trunc(),
        camera.raw.translation.truncate().trunc(),
        mouse_pos.trunc(),
    );

    let mut buffer = (&mut vm_text_buffer).get(tracker.text_id).unwrap();
    buffer.set_text(&mut text_pipeline, &text);

    let mut pos = (&mut vm_pos).get(tracker.circle_id).unwrap();
    pos.x = mouse_pos.x;
    pos.y = mouse_pos.y;
}

//====================================================================

#[derive(Unique, Default)]
pub struct DebugCircles {
    pub to_spawn: Vec<(f32, f32, [f32; 4], Duration)>,

    points: Vec<(EntityId, Duration, Duration)>,
}

fn sys_setup_debug_circles(all_storages: AllStoragesView) {
    all_storages.add_unique(DebugCircles::default());
}

fn sys_spawn_debug_circles(
    mut debug_circles: ResMut<DebugCircles>,
    mut entities: EntitiesViewMut,

    mut vm_circles: ViewMut<Circle>,
    mut vm_pos: ViewMut<Pos>,
) {
    let ids = debug_circles
        .to_spawn
        .drain(..)
        .map(|(x, y, color, timeout)| {
            (
                entities.add_entity(
                    (&mut vm_circles, &mut vm_pos),
                    (Circle { radius: 20., color }, Pos { x, y }),
                ),
                timeout,
            )
        })
        .collect::<Vec<_>>();

    ids.into_iter()
        .for_each(|(id, timeout)| debug_circles.points.push((id, Duration::ZERO, timeout)));
}

fn sys_despawn_debug_circles(mut all_storages: AllStoragesViewMut) {
    let time = all_storages.borrow::<Res<Time>>().unwrap();
    let mut debug_circles = all_storages.borrow::<ResMut<DebugCircles>>().unwrap();

    let to_despawn = debug_circles
        .points
        .iter_mut()
        .enumerate()
        .filter_map(|(index, (_, duration, timeout))| {
            *duration += *time.delta();

            match duration > timeout {
                true => Some(index),
                false => None,
            }
        })
        .collect::<Vec<_>>();

    if to_despawn.is_empty() {
        return;
    }

    let ids = to_despawn
        .into_iter()
        .rev()
        .map(|index| debug_circles.points.remove(index).0)
        .collect::<Vec<_>>();

    std::mem::drop(time);
    std::mem::drop(debug_circles);

    ids.into_iter().for_each(|id| {
        all_storages.delete_entity(id);
    });
}

//====================================================================
