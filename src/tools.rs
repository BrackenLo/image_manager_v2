//====================================================================

use std::{
    fmt::Display,
    hash::Hash,
    time::{Duration, Instant},
};

use ahash::{HashSet, HashSetExt};
use shipyard::Unique;
use winit::keyboard::KeyCode;

use crate::window::WindowSize;

//====================================================================

pub type Res<'a, T> = shipyard::UniqueView<'a, T>;
pub type ResMut<'a, T> = shipyard::UniqueViewMut<'a, T>;

pub trait WorldTools {
    fn and_run<B, S: shipyard::System<(), B>>(&self, system: S) -> &Self;
    fn and_run_with_data<Data, B, S: shipyard::System<(Data,), B>>(
        &self,
        system: S,
        data: Data,
    ) -> &Self;
}

impl WorldTools for shipyard::World {
    #[inline]
    fn and_run<B, S: shipyard::System<(), B>>(&self, system: S) -> &Self {
        self.run(system);
        self
    }

    #[inline]
    fn and_run_with_data<Data, B, S: shipyard::System<(Data,), B>>(
        &self,
        system: S,
        data: Data,
    ) -> &Self {
        self.run_with_data(system, data);
        self
    }
}

pub trait UniqueTools {
    fn insert<U: shipyard::Unique + Send + Sync>(&self, unique: U) -> &Self;
    fn replace<U: shipyard::Unique + Send + Sync>(&self, unique: U);
}

impl UniqueTools for shipyard::World {
    #[inline]
    fn insert<U: shipyard::Unique + Send + Sync>(&self, unique: U) -> &Self {
        self.add_unique(unique);
        self
    }

    fn replace<U: shipyard::Unique + Send + Sync>(&self, unique: U) {
        self.remove_unique::<U>().ok();
        self.add_unique(unique);
    }
}

impl UniqueTools for shipyard::AllStoragesView<'_> {
    fn insert<U: shipyard::Unique + Send + Sync>(&self, unique: U) -> &Self {
        self.add_unique(unique);
        self
    }

    fn replace<U: shipyard::Unique + Send + Sync>(&self, unique: U) {
        self.remove_unique::<U>().ok();
        self.add_unique(unique);
    }
}

//====================================================================

#[derive(Clone, Copy)]
pub struct Size<T> {
    pub width: T,
    pub height: T,
}

impl<T> Size<T> {
    pub fn new(width: T, height: T) -> Self {
        Self { width, height }
    }
}

impl<T> From<winit::dpi::PhysicalSize<T>> for Size<T> {
    fn from(value: winit::dpi::PhysicalSize<T>) -> Self {
        Self {
            width: value.width,
            height: value.height,
        }
    }
}

impl<T: Display> Display for Size<T> {
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
pub(crate) struct Time {
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
    pub(crate) fn elapsed(&self) -> &Instant {
        &self.elapsed
    }

    #[inline]
    pub(crate) fn delta(&self) -> &Duration {
        &self.delta
    }

    #[inline]
    pub(crate) fn delta_seconds(&self) -> f32 {
        self.delta_seconds
    }
}

pub(crate) fn sys_update_time(mut time: ResMut<Time>) {
    time.delta = time.last_frame.elapsed();
    time.delta_seconds = time.delta.as_secs_f32();

    time.last_frame = Instant::now();
}

//====================================================================

#[derive(Unique, Debug)]
pub(crate) struct Input<T>
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

pub(crate) fn sys_process_keypress(key: (KeyCode, bool), mut keys: ResMut<Input<KeyCode>>) {
    keys.process_input(key.0, key.1);
}

pub(crate) fn sys_reset_key_input(mut keys: ResMut<Input<KeyCode>>) {
    keys.reset();
}

//--------------------------------------------------

#[derive(Unique, Debug, Default)]
pub(crate) struct MouseInput {
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

pub(crate) fn sys_process_wheel(wheel: [f32; 2], mut mouse: ResMut<MouseInput>) {
    mouse.scroll += glam::Vec2::from(wheel);
}

pub(crate) fn sys_process_mouse_pos(
    pos: [f32; 2],
    mut mouse: ResMut<MouseInput>,
    size: Res<WindowSize>,
) {
    mouse.pos = pos.into();

    mouse.screen_pos = glam::vec2(mouse.pos.x, size.height() as f32 - mouse.pos.y as f32);
    // let half_size = glam::vec2(size.width() as f32 / 2., size.height() as f32 / 2.);
    // mouse.screen_pos = pos - half_size - camera.raw.translation.truncate();

    // println!("pos = {}, Screen pos = {}", mouse.pos, mouse.screen_pos);
}

pub(crate) fn sys_reset_mouse_input(mut mouse: ResMut<MouseInput>) {
    mouse.pos_delta = glam::Vec2::ZERO;
    mouse.scroll = glam::Vec2::ZERO;
}

//====================================================================

pub(crate) fn aabb_point(point: glam::Vec2, area_pos: glam::Vec2, area_size: glam::Vec2) -> bool {
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
