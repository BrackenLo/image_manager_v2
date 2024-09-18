//====================================================================

use crate::app::App;
use winit::{application::ApplicationHandler, event_loop::EventLoop};

pub(crate) mod app;
pub(crate) mod debug;
pub(crate) mod images;
pub(crate) mod layout;
pub(crate) mod renderer;
pub(crate) mod storage;
pub(crate) mod tools;
pub(crate) mod window;

//====================================================================

const NAME: &str = env!("CARGO_PKG_NAME");

fn main() {
    println!("Hello, world!");

    env_logger::Builder::new()
        .filter_module("wgpu", log::LevelFilter::Warn)
        .filter_module(NAME, log::LevelFilter::Trace)
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
        _event_loop: &winit::event_loop::ActiveEventLoop,
        cause: winit::event::StartCause,
    ) {
        if let Some(inner) = &mut self.inner {
            match cause {
                winit::event::StartCause::ResumeTimeReached { .. } => inner.resumed(),
                // winit::event::StartCause::WaitCancelled { start, requested_resume } => todo!(),
                // winit::event::StartCause::Poll => todo!(),
                // winit::event::StartCause::Init => todo!(),
                _ => {}
            }
        }
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
