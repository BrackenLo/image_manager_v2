//====================================================================

use std::ops::DerefMut;

use app::App;
use winit::{application::ApplicationHandler, event_loop::EventLoop};

pub mod app;
pub mod debug;
pub mod images;
pub mod layout;
pub mod renderer;
pub mod storage;
pub mod tools;
pub mod window;

//====================================================================

fn main() {
    println!("Hello, world!");

    env_logger::Builder::new()
        .filter_module("wgpu", log::LevelFilter::Warn)
        .filter_module("image_manager_v2", log::LevelFilter::Trace)
        .format_timestamp(None)
        .init();

    let mut app = Runner::new();
    let event_loop = EventLoop::new().unwrap();
    match event_loop.run_app(&mut app) {
        Ok(_) => {}
        Err(e) => println!("Error on close: {}", e),
    };
}

//====================================================================

struct Runner {
    inner: Option<App>,
}

impl Runner {
    pub fn new() -> Self {
        Self { inner: None }
    }
}

impl ApplicationHandler for Runner {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        log::trace!("App resumed");
        self.inner = Some(App::new(event_loop));
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        if let Some(inner) = &mut self.inner {
            inner.window_event(event_loop, window_id, event);
        }
    }

    fn new_events(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        cause: winit::event::StartCause,
    ) {
        let _ = (event_loop, cause);
    }

    fn user_event(&mut self, event_loop: &winit::event_loop::ActiveEventLoop, event: ()) {
        let _ = (event_loop, event);
    }

    fn device_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        let _ = (event_loop, device_id, event);
    }

    fn about_to_wait(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let _ = event_loop;
    }

    fn suspended(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let _ = event_loop;
    }

    fn exiting(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let _ = event_loop;
    }

    fn memory_warning(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let _ = event_loop;
    }
}

//====================================================================
