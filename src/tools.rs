//====================================================================

use std::{
    fmt::Display,
    hash::Hash,
    time::{Duration, Instant},
};

use ahash::{HashSet, HashSetExt};
use shipyard::Unique;
use winit::keyboard::KeyCode;

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

pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    const ZERO: Self = Rect {
        x: 0.,
        y: 0.,
        width: 0.,
        height: 0.,
    };

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

//====================================================================
