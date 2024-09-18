//====================================================================

use std::sync::Arc;

use shipyard::{AllStoragesView, Unique};
use shipyard_tools::{prelude::*, UniqueTools};

use crate::tools::Size;

//====================================================================

#[derive(Unique, Debug)]
pub struct WindowSize(Size<u32>);
impl WindowSize {
    #[inline]
    pub fn inner(&self) -> Size<u32> {
        self.0
    }

    #[inline]
    pub fn width(&self) -> u32 {
        self.0.width
    }

    #[inline]
    pub fn height(&self) -> u32 {
        self.0.height
    }

    #[inline]
    pub fn width_f32(&self) -> f32 {
        self.0.width as f32
    }

    #[inline]
    pub fn height_f32(&self) -> f32 {
        self.0.height as f32
    }
}

#[derive(Unique)]
pub struct Window(Arc<winit::window::Window>);
impl Window {
    #[inline]
    pub fn inner(&self) -> &winit::window::Window {
        &self.0
    }

    #[inline]
    pub fn request_redraw(&self) {
        self.0.request_redraw();
    }

    pub fn arc(&self) -> &Arc<winit::window::Window> {
        &self.0
    }
}

#[derive(Event)]
pub struct ResizeEvent;

//====================================================================

pub(super) fn sys_add_window(window: Arc<winit::window::Window>, all_storages: AllStoragesView) {
    all_storages
        .insert(WindowSize(window.inner_size().into()))
        .insert(Window(window));
}

pub(super) fn sys_resize(
    new_size: Size<u32>,
    mut size: ResMut<WindowSize>,
    mut event_handler: ResMut<EventHandler>,
) {
    size.0 = new_size;

    event_handler.add_event(ResizeEvent);
}

//====================================================================
