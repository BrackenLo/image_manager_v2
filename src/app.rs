//====================================================================

use std::{sync::Arc, time::Duration};

use shipyard_tools::{Stages, WorkloadBuilder};
use winit::{
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow},
};

use crate::{
    debug::DebugPlugin,
    images::ImagePlugin,
    layout::LayoutPlugin,
    renderer::RendererPlugin,
    storage::StoragePlugin,
    tools::{self, Size, ToolsPlugin},
    window::{self, Window},
};

//====================================================================

const TIMESTEP: f32 = 1. / 75.;

pub struct App {
    world: shipyard::World,
    timestep: Duration,
}

impl App {
    pub fn new(event_loop: &ActiveEventLoop) -> Self {
        let world = shipyard::World::new();

        let window = Arc::new(
            event_loop
                .create_window(winit::window::Window::default_attributes())
                .unwrap(),
        );

        // Setup window and renderer components
        world.run_with_data(window::sys_add_window, window);

        WorkloadBuilder::new(&world)
            .add_plugin(ToolsPlugin)
            .add_plugin(RendererPlugin)
            .add_plugin(DebugPlugin)
            .add_plugin(StoragePlugin)
            .add_plugin(LayoutPlugin)
            .add_plugin(ImagePlugin)
            .build();

        world.run_workload(Stages::Setup).unwrap();

        Self {
            world,
            timestep: Duration::from_secs_f32(TIMESTEP),
        }
    }

    //--------------------------------------------------

    fn resize(&mut self, new_size: Size<u32>) {
        if new_size.width == 0 || new_size.height == 0 {
            log::warn!("Resize width or height of '0' provided");
            return;
        }

        self.world.run_with_data(window::sys_resize, new_size);
    }

    //--------------------------------------------------

    fn tick(&mut self) {
        self.world.run_workload(Stages::First).unwrap();

        shipyard_tools::activate_events(&self.world);

        self.world.run_workload(Stages::Update).unwrap();
        self.world.run_workload(Stages::Render).unwrap();

        self.world.run_workload(Stages::Last).unwrap();
    }

    //--------------------------------------------------
}

//====================================================================

impl App {
    pub fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        match event {
            WindowEvent::Resized(new_size) => self.resize(new_size.into()),

            WindowEvent::Destroyed => log::error!("Window was destroyed"), // panic!("Window was destroyed"),
            WindowEvent::CloseRequested => {
                log::info!("Close requested. Closing App.");
                event_loop.exit();
            }

            WindowEvent::RedrawRequested => {
                self.tick();

                event_loop.set_control_flow(ControlFlow::wait_duration(self.timestep));
            }

            WindowEvent::KeyboardInput { event, .. } => {
                if let winit::keyboard::PhysicalKey::Code(key) = event.physical_key {
                    self.world.run_with_data(
                        tools::sys_process_input::<winit::keyboard::KeyCode>,
                        (key, event.state.is_pressed()),
                    );
                }
            }

            WindowEvent::MouseInput { state, button, .. } => self.world.run_with_data(
                tools::sys_process_input::<winit::event::MouseButton>,
                (button, state.is_pressed()),
            ),

            WindowEvent::CursorMoved { position, .. } => self.world.run_with_data(
                tools::sys_process_mouse_pos,
                [position.x as f32, position.y as f32],
            ),
            WindowEvent::MouseWheel { delta, .. } => match delta {
                winit::event::MouseScrollDelta::LineDelta(h, v) => {
                    self.world.run_with_data(tools::sys_process_wheel, [h, v])
                }
                winit::event::MouseScrollDelta::PixelDelta(_) => {}
            },

            _ => {}
        }
    }

    pub fn resumed(&mut self) {
        self.world
            .run(|window: shipyard::UniqueView<Window>| window.request_redraw());
    }
}

//====================================================================
