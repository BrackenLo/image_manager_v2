//====================================================================

use std::sync::Arc;

use shipyard::{AllStoragesView, Unique};

use crate::tools::{ResMut, Size, UniqueTools};

//====================================================================

#[derive(Unique)]
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
}

#[derive(Unique)]
pub struct Window(Arc<winit::window::Window>);
impl Window {
    #[inline]
    pub fn _inner(&self) -> &winit::window::Window {
        &self.0
    }

    #[inline]
    pub fn request_redraw(&self) {
        self.0.request_redraw();
    }
}

//====================================================================

pub fn sys_add_window(window: Arc<winit::window::Window>, all_storages: AllStoragesView) {
    all_storages
        .insert(WindowSize(window.inner_size().into()))
        .insert(Window(window));
}

pub fn sys_resize(new_size: Size<u32>, mut size: ResMut<WindowSize>) {
    size.0 = new_size;
}

//====================================================================
