//====================================================================

use std::{sync::Arc, time::Duration};

use shipyard::AllStoragesView;
use winit::{
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow},
};

use crate::{
    debug::DebugPlugin,
    images::ImagePlugin,
    layout::LayoutPlugin,
    renderer::{Device, Queue, RenderPassTools, RendererPlugin, Surface},
    shipyard_tools::{Res, Stages, WorkloadBuilder},
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

        world.run_workload(Stages::PreSetup).unwrap();
        world.run_workload(Stages::Setup).unwrap();
        world.run_workload(Stages::PostSetup).unwrap();

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
        self.world.run_workload(Stages::Resize).unwrap();
    }

    //--------------------------------------------------

    fn tick(&mut self) {
        self.world.run_workload(Stages::First).unwrap();

        self.world.run_workload(Stages::PreUpdate).unwrap();
        self.world.run_workload(Stages::Update).unwrap();
        self.world.run_workload(Stages::PostUpdate).unwrap();

        // Rendering
        if let Err(e) = self.world.run(sys_setup_render) {
            match e {
                wgpu::SurfaceError::Lost => todo!(),
                wgpu::SurfaceError::OutOfMemory => todo!(),
                // wgpu::SurfaceError::Timeout => todo!(),
                // wgpu::SurfaceError::Outdated => todo!(),
                _ => {}
            }

            log::debug!("Skipped render frame: {}", e);

            return;
        }

        self.world.run_workload(Stages::PreRender).unwrap();
        self.world.run_workload(Stages::Render).unwrap();
        self.world.run_workload(Stages::PostRender).unwrap();

        self.world.run(sys_finish_render);

        self.world.run_workload(Stages::Last).unwrap();
    }

    //--------------------------------------------------
}

fn sys_setup_render(
    all_storages: AllStoragesView,
    device: Res<Device>,
    surface: Res<Surface>,
) -> Result<(), wgpu::SurfaceError> {
    let tools = RenderPassTools::new(device.inner(), surface.inner())?;

    all_storages.add_unique(tools);

    Ok(())
}

fn sys_finish_render(all_storages: AllStoragesView, queue: Res<Queue>) {
    let tools = all_storages.remove_unique::<RenderPassTools>().unwrap();
    tools.finish(queue.inner());
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
                        tools::sys_process_keypress,
                        (key, event.state.is_pressed()),
                    );
                }
            }

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
