//====================================================================

use std::{
    fmt::Display,
    hash::Hash,
    time::{Duration, Instant},
};

use ahash::{HashSet, HashSetExt};
use shipyard::{AllStoragesView, Unique, Workload};
use winit::keyboard::KeyCode;

use crate::{
    shipyard_tools::{Plugin, Res, ResMut, Stages, UniqueTools},
    window::WindowSize,
};

//====================================================================

pub(crate) struct ToolsPlugin;

impl Plugin for ToolsPlugin {
    fn build(&self, workload_builder: &mut crate::shipyard_tools::WorkloadBuilder) {
        workload_builder
            .add_workload(
                Stages::Setup,
                Workload::new("").with_system(sys_setup_uniques),
            )
            .add_workload(
                Stages::First,
                Workload::new("").with_system(sys_update_time),
            )
            .add_workload(
                Stages::Last,
                Workload::new("")
                    .with_system(sys_reset_key_input)
                    .with_system(sys_reset_mouse_input),
            );
    }
}

fn sys_setup_uniques(all_storages: AllStoragesView) {
    all_storages
        .insert(Time::default())
        .insert(Input::<KeyCode>::default())
        .insert(MouseInput::default());
}

//====================================================================

#[derive(Clone, Copy)]
pub struct Size<T> {
    pub width: T,
    pub height: T,
}

impl<T> Size<T> {
    #[inline]
    pub fn new(width: T, height: T) -> Self {
        Self { width, height }
    }
}

impl<T> From<winit::dpi::PhysicalSize<T>> for Size<T> {
    #[inline]
    fn from(value: winit::dpi::PhysicalSize<T>) -> Self {
        Self {
            width: value.width,
            height: value.height,
        }
    }
}

impl<T> From<(T, T)> for Size<T> {
    #[inline]
    fn from(value: (T, T)) -> Self {
        Self {
            width: value.0,
            height: value.1,
        }
    }
}

impl<T: Display> Display for Size<T> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}, {})", self.width, self.height)
    }
}

//--------------------------------------------------

#[derive(Clone)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    #[inline]
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    #[inline]
    pub fn from_size(width: f32, height: f32) -> Self {
        Self {
            x: 0.,
            y: 0.,
            width,
            height,
        }
    }
}

impl Default for Rect {
    fn default() -> Self {
        Self {
            x: 0.,
            y: 0.,
            width: 1.,
            height: 1.,
        }
    }
}

//====================================================================

#[derive(Unique)]
pub struct Time {
    elapsed: Instant,

    last_frame: Instant,
    delta: Duration,
    delta_seconds: f32,
}

impl Default for Time {
    fn default() -> Self {
        Self {
            elapsed: Instant::now(),
            last_frame: Instant::now(),
            delta: Duration::ZERO,
            delta_seconds: 0.,
        }
    }
}

#[allow(dead_code)]
impl Time {
    #[inline]
    pub fn elapsed(&self) -> &Instant {
        &self.elapsed
    }

    #[inline]
    pub fn delta(&self) -> &Duration {
        &self.delta
    }

    #[inline]
    pub fn delta_seconds(&self) -> f32 {
        self.delta_seconds
    }
}

fn sys_update_time(mut time: ResMut<Time>) {
    time.delta = time.last_frame.elapsed();
    time.delta_seconds = time.delta.as_secs_f32();

    time.last_frame = Instant::now();
}

//====================================================================

#[derive(Unique, Debug)]
pub struct Input<T>
where
    T: 'static + Send + Sync + Eq + PartialEq + Hash,
{
    pressed: HashSet<T>,
    just_pressed: HashSet<T>,
    released: HashSet<T>,
}

impl<T> Default for Input<T>
where
    T: 'static + Send + Sync + Eq + PartialEq + Hash,
{
    fn default() -> Self {
        Self {
            pressed: HashSet::new(),
            just_pressed: HashSet::new(),
            released: HashSet::new(),
        }
    }
}

impl<T> Input<T>
where
    T: 'static + Send + Sync + Eq + PartialEq + Hash + Clone + Copy,
{
    pub fn new() -> Self {
        Self::default()
    }

    fn add_pressed(&mut self, input: T) {
        self.pressed.insert(input);
        self.just_pressed.insert(input);
    }

    fn remove_pressed(&mut self, input: T) {
        self.pressed.remove(&input);
        self.released.insert(input);
    }

    fn reset(&mut self) {
        self.just_pressed.clear();
        self.released.clear();
    }

    fn process_input(&mut self, input: T, pressed: bool) {
        match pressed {
            true => self.add_pressed(input),
            false => self.remove_pressed(input),
        }
    }

    #[inline]
    pub fn pressed(&self, input: T) -> bool {
        self.pressed.contains(&input)
    }

    #[inline]
    pub fn just_pressed(&self, input: T) -> bool {
        self.just_pressed.contains(&input)
    }

    #[inline]
    pub fn released(&self, input: T) -> bool {
        self.released.contains(&input)
    }
}

pub(super) fn sys_process_keypress(key: (KeyCode, bool), mut keys: ResMut<Input<KeyCode>>) {
    keys.process_input(key.0, key.1);
}

fn sys_reset_key_input(mut keys: ResMut<Input<KeyCode>>) {
    keys.reset();
}

//--------------------------------------------------

#[derive(Unique, Debug, Default)]
pub struct MouseInput {
    pos: glam::Vec2,
    screen_pos: glam::Vec2,
    pos_delta: glam::Vec2,
    scroll: glam::Vec2,
}

impl MouseInput {
    #[inline]
    pub fn scroll(&self) -> glam::Vec2 {
        self.scroll
    }

    #[inline]
    pub fn _pos(&self) -> glam::Vec2 {
        self.pos
    }

    #[inline]
    pub fn screen_pos(&self) -> glam::Vec2 {
        self.screen_pos
    }
}

pub(super) fn sys_process_wheel(wheel: [f32; 2], mut mouse: ResMut<MouseInput>) {
    mouse.scroll += glam::Vec2::from(wheel);
}

pub(super) fn sys_process_mouse_pos(
    pos: [f32; 2],
    mut mouse: ResMut<MouseInput>,
    size: Res<WindowSize>,
) {
    mouse.pos = pos.into();
    mouse.screen_pos = glam::vec2(mouse.pos.x, size.height_f32() - mouse.pos.y as f32);
}

fn sys_reset_mouse_input(mut mouse: ResMut<MouseInput>) {
    mouse.pos_delta = glam::Vec2::ZERO;
    mouse.scroll = glam::Vec2::ZERO;
}

//====================================================================

pub fn aabb_point(point: glam::Vec2, area_pos: glam::Vec2, area_size: glam::Vec2) -> bool {
    let dx = point.x - area_pos.x;
    let px = area_size.x / 2. - dx.abs();

    if px <= 0. {
        return false;
    }

    let dy = point.y - area_pos.y;
    let py = area_size.y / 2. - dy.abs();

    if py <= 0. {
        return false;
    }

    true
}

// pub(crate) fn aabb(
//     pos_a: glam::Vec2,
//     size_a: glam::Vec2,
//     pos_b: glam::Vec2,
//     size_b: glam::Vec2,
// ) -> bool {
//     let half_a = glam::vec2(size_a.x / 2., size_a.y / 2.);
//     let half_b = glam::vec2(size_b.x / 2., size_b.y / 2.);

//     let a_min_x = pos_a.x - half_a.x;
//     let a_max_x = pos_a.x + half_a.x;

//     let b_min_x = pos_b.x - half_b.x;
//     let b_max_x = pos_b.x + half_b.x;

//     let a_min_y = pos_a.y - half_a.y;
//     let a_max_y = pos_a.y + half_a.y;

//     let b_min_y = pos_b.y - half_b.y;
//     let b_max_y = pos_b.y + half_b.y;

//     a_min_x <= b_max_x && a_max_x >= b_min_x && a_min_y <= b_max_y && a_max_y >= b_min_y
// }

//====================================================================
