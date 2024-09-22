//====================================================================

use cabat::{
    common::WindowSize,
    renderer::{Camera, Device, OrthographicCamera, Queue},
    shipyard_tools::{Res, ResMut, UniqueTools},
};
use shipyard::{AllStoragesView, Unique};

//====================================================================

#[derive(Unique)]
pub struct MainCamera {
    pub camera: Camera,
    pub raw: OrthographicCamera,
}

#[derive(Unique)]
pub struct UiCamera {
    pub camera: Camera,
    pub raw: OrthographicCamera,
}

pub(super) fn sys_setup_camera(all_storages: AllStoragesView, device: Res<Device>) {
    let raw = OrthographicCamera::default();
    let main_camera = MainCamera {
        camera: Camera::new(device.inner(), &raw),
        raw,
    };

    let raw = OrthographicCamera::default();
    let ui_camera = UiCamera {
        camera: Camera::new(device.inner(), &raw),
        raw,
    };

    all_storages.insert(main_camera).insert(ui_camera);
}

pub(super) fn sys_resize_camera(size: Res<WindowSize>, mut ui_camera: ResMut<UiCamera>) {
    ui_camera.raw.set_size(size.width_f32(), size.height_f32());
}

pub(super) fn sys_update_camera(
    queue: Res<Queue>,
    main_camera: ResMut<MainCamera>,
    ui_camera: ResMut<UiCamera>,
) {
    if main_camera.is_modified() {
        main_camera
            .camera
            .update_camera(queue.inner(), &main_camera.raw)
    }

    if ui_camera.is_modified() {
        ui_camera
            .camera
            .update_camera(queue.inner(), &main_camera.raw)
    }
}

//====================================================================
