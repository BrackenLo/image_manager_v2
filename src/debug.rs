//====================================================================

use shipyard::{AllStoragesView, EntitiesViewMut, EntityId, Get, IntoWorkload, Unique, ViewMut};

use crate::{
    app::Stages,
    images::Pos,
    renderer::{
        camera::{Camera, MainCamera},
        circle_pipeline::Circle,
        text_pipeline::{TextBuffer, TextBufferDescriptor, TextPipeline},
    },
    shipyard_tools::{Plugin, Res, ResMut},
    tools::{MouseInput, Time},
};

//====================================================================

pub(crate) struct DebugPlugin;

impl Plugin<Stages> for DebugPlugin {
    fn build(&self, workload_builder: &mut crate::shipyard_tools::WorkloadBuilder<Stages>) {
        workload_builder
            .add_workload(
                Stages::Setup,
                (sys_setup_debug, sys_setup_mouse_tracker).into_workload(),
            )
            .add_workload(
                Stages::PostSetup,
                (sys_display_memory_usage).into_workload(),
            )
            .add_workload(
                Stages::PreUpdate,
                (sys_tick_upkeep, sys_update_mouse_tracker).into_workload(),
            );
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
        (Circle { radius: 5. }, Pos { x: 0., y: 0. }),
    );

    let tracker = MouseTracker { text_id, circle_id };

    all_storages.add_unique(tracker);
}

fn sys_update_mouse_tracker(
    tracker: ResMut<MouseTracker>,
    camera: Res<Camera<MainCamera>>,
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
