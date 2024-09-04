//====================================================================

use shipyard::Unique;

use crate::tools::{Res, ResMut, Time};

//====================================================================

#[derive(Unique)]
pub(crate) struct Upkeep {
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
    upkeep.tick(time.delta_seconds(), true);
}

//====================================================================
