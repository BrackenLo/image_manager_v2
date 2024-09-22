//====================================================================

use cabat::{
    common::WindowResizeEvent,
    renderer::{Device, RenderPass, SurfaceConfig},
    shipyard_tools::{prelude::*, UniqueTools},
};
use camera::{sys_resize_camera, sys_setup_camera, sys_update_camera, MainCamera, UiCamera};
use circle_pipeline::{sys_update_circle_pipeline, CirclePipeline};
use gif2d_pipeline::Gif2dPipeline;
use shipyard::{AllStoragesView, IntoIter, IntoWorkload, View};
use texture2d_pipeline::Texture2dPipeline;

use crate::images::{GifImage, ImageShown, StandardImage};

pub mod camera;
pub mod circle_pipeline;
pub mod gif;
pub mod gif2d_pipeline;
pub mod texture2d_pipeline;

//====================================================================

pub(crate) struct CustomRendererPlugin;

impl Plugin for CustomRendererPlugin {
    fn build(self, workload_builder: WorkloadBuilder) -> WorkloadBuilder {
        workload_builder
            .add_workload_sub(
                Stages::Setup,
                SubStages::Pre,
                (sys_setup_camera, sys_setup_pipelines).into_sequential_workload(),
            )
            .add_workload_sub(
                Stages::Update,
                SubStages::Last,
                (sys_update_circle_pipeline, sys_update_camera).into_workload(),
            )
            .add_workload(
                Stages::Render,
                (sys_render_circles, sys_render_textures, sys_render_gifs).into_workload(),
            )
            .add_event::<WindowResizeEvent>((sys_resize_camera).into_workload())
    }
}

//====================================================================

fn sys_setup_pipelines(
    all_storages: AllStoragesView,
    device: Res<Device>,
    config: Res<SurfaceConfig>,
    camera: Res<MainCamera>,
) {
    all_storages
        .insert(Texture2dPipeline::new(
            device.inner(),
            config.inner(),
            camera.camera.bind_group_layout(),
        ))
        .insert(CirclePipeline::new(
            device.inner(),
            config.inner(),
            camera.camera.bind_group_layout(),
        ))
        .insert(Gif2dPipeline::new(
            device.inner(),
            config.inner(),
            camera.camera.bind_group_layout(),
        ));
}

//====================================================================

fn sys_render_circles(
    mut pass: ResMut<RenderPass>,
    circle_pipeline: Res<CirclePipeline>,
    main_camera: Res<MainCamera>,
) {
    circle_pipeline.render(pass.pass(), main_camera.camera.bind_group());
}

fn sys_render_textures(
    mut pass: ResMut<RenderPass>,
    texture_pipeline: Res<Texture2dPipeline>,

    main_camera: Res<MainCamera>,
    ui_camera: Res<UiCamera>,

    v_images: View<StandardImage>,
    v_shown: View<ImageShown>,
) {
    let images = (&v_images, !&v_shown)
        .iter()
        .map(|(image, _)| &image.instance);

    texture_pipeline.render(
        pass.pass(),
        main_camera.camera.bind_group(),
        images.into_iter(),
        // Some(viewport.inner()), // BUG - fix viewport not working with world space
        None,
    );

    if !v_shown.is_empty() {
        let images = (&v_images, &v_shown)
            .iter()
            .map(|(image, _)| &image.instance);

        texture_pipeline.render(
            pass.pass(),
            ui_camera.camera.bind_group(),
            images.into_iter(),
            // Some(viewport.inner()), // BUG - fix viewport not working with world space
            None,
        );
    }
}

fn sys_render_gifs(
    mut pass: ResMut<RenderPass>,
    gif_pipeline: Res<Gif2dPipeline>,

    main_camera: Res<MainCamera>,
    ui_camera: Res<UiCamera>,

    v_gifs: View<GifImage>,
    v_shown: View<ImageShown>,
) {
    let images = (&v_gifs, !&v_shown)
        .iter()
        .map(|(image, _)| &image.instance);

    gif_pipeline.render(
        pass.pass(),
        main_camera.camera.bind_group(),
        images.into_iter(),
    );

    if !v_shown.is_empty() {
        let images = (&v_gifs, &v_shown).iter().map(|(image, _)| &image.instance);

        gif_pipeline.render(
            pass.pass(),
            &ui_camera.camera.bind_group(),
            images.into_iter(),
        );
    }
}

//====================================================================
