//====================================================================

use std::{
    env,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use shipyard::{AllStoragesView, IntoIter, View};
use winit::{
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow},
    keyboard::KeyCode,
};

use crate::{
    debug::{self, Upkeep},
    images::{self, Image},
    layout::{self, ImageViewport, LayoutManager, LayoutNavigation},
    renderer::{
        self,
        camera::{self, MainCamera},
        texture::DepthTexture,
        texture_pipeline::TexturePipeline,
        Device, Queue, RenderPassTools, RenderPassToolsDesc, Surface,
    },
    storage::{self, Storage},
    tools::{self, Input, MouseInput, Res, ResMut, Size, Time, UniqueTools, WorldTools},
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

        world
            .and_run_with_data(window::sys_add_window, window.clone())
            .and_run_with_data(renderer::sys_setup_renderer_components, window);

        let mut app = Self {
            world,
            timestep: Duration::from_secs_f32(TIMESTEP),
        };

        app.setup();
        app
    }

    //--------------------------------------------------

    fn resize(&mut self, new_size: Size<u32>) {
        if new_size.width == 0 || new_size.height == 0 {
            log::warn!("Resize width or height of '0' provided");
            return;
        }

        self.world
            .and_run_with_data(window::sys_resize, new_size)
            .and_run(renderer::sys_resize)
            .and_run(renderer::camera::sys_resize_camera)
            .and_run(renderer::texture::sys_resize_depth_texture)
            .and_run(layout::sys_resize_layout);
    }

    //--------------------------------------------------

    fn setup(&mut self) {
        self.world
            .insert(Time::default())
            .insert(Input::<KeyCode>::new())
            .insert(MouseInput::default())
            .insert(Upkeep::new());

        self.world
            .and_run(renderer::camera::sys_setup_camera)
            .and_run(renderer::texture::sys_setup_depth_texture)
            .and_run(renderer::sys_setup_pipelines);

        let args: Vec<String> = env::args().collect();
        log::trace!("Args {:?}", args);

        let path = match args.get(1) {
            Some(arg) => {
                let path = Path::new(arg);
                if !path.is_dir() {
                    panic!("Invalid path provided");
                }

                PathBuf::from(path)
            }
            None => {
                env::current_dir().expect("No path provided and cannot access current directory.")
            }
        };

        self.world
            .insert(LayoutManager::default())
            .insert(ImageViewport::default())
            .insert(LayoutNavigation::default())
            .insert(Storage::new())
            .and_run_with_data(storage::sys_load_path, path);
    }

    //--------------------------------------------------

    fn tick(&mut self) {
        self.update();
        self.render();

        // self.world
        //     .run(|window: shipyard::UniqueView<Window>| window.request_redraw());
    }

    fn update(&mut self) {
        // Upkeep
        self.world
            .and_run(tools::sys_update_time)
            .and_run(debug::sys_tick_upkeep);

        // Get any newly loaded images
        if self.world.run(storage::sys_check_loading) {
            self.world
                .and_run(storage::sys_process_new_images)
                .and_run(storage::sys_spawn_new_images);
        }

        // Any other stuff
        self.world
            .and_run(camera::sys_update_camera)
            .and_run(layout::sys_navigate_layout);

        // Format images - Always do second to last
        self.world
            .and_run(layout::sys_order_images)
            .and_run(layout::sys_rebuild_images);

        // Clear up
        self.world
            .and_run(tools::sys_reset_key_input)
            .and_run(tools::sys_reset_mouse_input)
            .and_run(images::sys_remove_pending)
            .and_run(images::sys_clear_dirty);
    }

    fn render(&mut self) {
        if let Err(e) = self.world.run(sys_setup_render) {
            match e {
                wgpu::SurfaceError::Lost => todo!(),
                wgpu::SurfaceError::OutOfMemory => todo!(),
                // wgpu::SurfaceError::Timeout => todo!(),
                // wgpu::SurfaceError::Outdated => todo!(),
                _ => {}
            }

            return;
        }

        self.world.run(sys_render);
        self.world.run(sys_finish_render);
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

fn sys_render(
    mut tools: ResMut<RenderPassTools>,
    depth: Res<DepthTexture>,
    viewport: Res<ImageViewport>,

    texture_pipeline: Res<TexturePipeline>,
    camera: Res<MainCamera>,
    v_images: View<Image>,
) {
    let desc = RenderPassToolsDesc {
        use_depth: Some(&depth.main_texture().view),
        clear_color: Some([0.3, 0.3, 0.3, 1.]),
    };

    tools.render_pass_desc(desc, |pass| {
        // ui_pipeline.render(pass, &camera, ui.instances());

        let images = v_images.iter().map(|image| &image.instance);

        texture_pipeline.render(pass, &camera, images.into_iter(), Some(viewport.inner()))
    });
}

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
