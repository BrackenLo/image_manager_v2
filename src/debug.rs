//====================================================================

use shipyard::{AllStoragesView, EntitiesViewMut, EntityId, Get, Unique, ViewMut};

use crate::{
    images::Pos,
    renderer::{
        camera::MainCamera,
        circle_pipeline::Circle,
        text::{TextBuffer, TextBufferDescriptor, TextPipeline},
    },
    tools::{MouseInput, Res, ResMut, Time},
};

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

pub(crate) fn sys_tick_upkeep(mut upkeep: ResMut<Upkeep>, time: Res<Time>) {
    upkeep.tick(time.delta_seconds(), false);
}

//====================================================================

#[derive(Unique)]
pub struct MouseTracker {
    text_id: EntityId,
    circle_id: EntityId,
}

pub(crate) fn sys_setup_mouse_tracker(
    all_storages: AllStoragesView,
    mut entities: EntitiesViewMut,

    mut text: ResMut<TextPipeline>,
    mut vm_text_buffer: ViewMut<TextBuffer>,

    mut vm_circles: ViewMut<Circle>,
    mut vm_pos: ViewMut<Pos>,
) {
    let text_id = entities.add_entity(
        &mut vm_text_buffer,
        TextBuffer::new(&mut text, &TextBufferDescriptor::default()),
    );

    let circle_id = entities.add_entity(
        (&mut vm_circles, &mut vm_pos),
        (Circle { radius: 30. }, Pos { x: 0., y: 0. }),
    );

    let tracker = MouseTracker { text_id, circle_id };

    all_storages.add_unique(tracker);
}

pub(crate) fn sys_update_mouse_tracker(
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
