//====================================================================

use std::{
    env,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};

use ahash::AHashMap;
use crossbeam_channel::{Receiver, Sender};
use image::{DynamicImage, GenericImageView};
use shipyard::{AllStoragesView, IntoWorkload, Unique, ViewMut, Workload, WorkloadModificator};

use crate::{
    app::Stages,
    images::{ImageCreator, ImageIndex, ImageMeta, StandardImage},
    layout::LayoutManager,
    renderer::{
        texture::Texture,
        texture_pipeline::{RawTextureInstance, TextureInstance, TexturePipeline},
        Device, Queue,
    },
    shipyard_tools::{Plugin, Res, ResMut},
    tools::Size,
};

//====================================================================

pub(crate) struct StoragePlugin;

impl Plugin<Stages> for StoragePlugin {
    fn build(&self, workload_builder: &mut crate::shipyard_tools::WorkloadBuilder<Stages>) {
        workload_builder
            .add_workload(
                Stages::PreSetup,
                Workload::new("").with_system(sys_setup_storage),
            )
            .add_workload(
                Stages::PreUpdate,
                Workload::new("")
                    .with_system(sys_process_new_images)
                    .with_system(sys_spawn_new_images)
                    .into_sequential_workload()
                    .run_if(sys_check_loading),
            )
            .add_workload(
                Stages::Update,
                Workload::new("")
                    .with_system(sys_load_path)
                    .with_system(sys_remove_load_images)
                    .into_sequential_workload(), // .skip_if_missing_unique::<LoadImages>(),
            );
    }
}

fn sys_setup_storage(all_storages: AllStoragesView) {
    all_storages.add_unique(Storage::new());

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
        None => env::current_dir().expect("No path provided and cannot access current directory."),
    };

    all_storages.add_unique(LoadImages { path });
}

//====================================================================

pub type TextureID = u64;

#[derive(Unique)]
pub struct Storage {
    textures: AHashMap<TextureID, TextureData>,

    loading: bool,
    to_spawn: Vec<TextureID>,

    load_kill_sender: Sender<bool>,
    load_kill_receiver: Receiver<bool>,

    image_sender: Sender<ImageChannel>,
    image_receiver: Receiver<ImageChannel>,
}

pub struct TextureData {
    pub texture: Texture,
    pub path: PathBuf,
    pub resolution: Size<u32>,
}

//====================================================================

#[derive(Unique)]
pub struct LoadImages {
    pub path: PathBuf,
}

enum ImageChannel {
    Finished,
    Image(PathBuf, DynamicImage),
}

impl Storage {
    pub fn new() -> Self {
        let (load_kill_sender, load_kill_receiver) = crossbeam_channel::unbounded();

        let (image_sender, image_receiver) = crossbeam_channel::unbounded();

        Self {
            textures: AHashMap::new(),

            loading: false,
            to_spawn: Vec::new(),

            load_kill_sender,
            load_kill_receiver,
            image_sender,
            image_receiver,
        }
    }

    pub fn _stop_loading(&mut self) {
        self.load_kill_sender.send(true).ok();
        self.loading = false;
    }

    #[inline]
    pub fn get_texture(&self, id: TextureID) -> Option<&TextureData> {
        self.textures.get(&id)
    }
}

fn sys_load_path(mut storage: ResMut<Storage>, to_load: Option<Res<LoadImages>>) {
    let to_load = match to_load {
        Some(val) => val,
        None => return,
    };

    log::info!("Loading images from path '{:?}'", to_load.path);

    let dir = std::fs::read_dir(&to_load.path).unwrap();
    let entries = dir.into_iter().filter_map(|e| e.ok()).collect::<Vec<_>>();

    let images_to_load = entries
        .into_iter()
        .filter_map(|entry| {
            let path = entry.path();

            if !path.is_file() {
                return None;
            }

            match path.extension() {
                None => return None,
                Some(ext) => match ext.to_str() {
                    Some("jpg") | Some("png") | Some("gif") => Some(path),
                    _ => {
                        log::trace!("Skipping file path '{:?}'", &path);
                        None
                    }
                },
            }
        })
        .collect::<Vec<_>>();

    //

    log::info!("Found '{}' images to load.", images_to_load.len());
    log::debug!("Images: {:#?}", images_to_load);

    storage.loading = true;

    let load_kill_receiver = storage.load_kill_receiver.clone();
    let image_sender = storage.image_sender.clone();

    std::thread::spawn(move || load_images(images_to_load, load_kill_receiver, image_sender));
}

fn sys_remove_load_images(all_storages: AllStoragesView) {
    all_storages.remove_unique::<LoadImages>().ok();
}

fn load_images(
    images: Vec<PathBuf>,
    load_kill_receiver: Receiver<bool>,
    image_sender: Sender<ImageChannel>,
) {
    for to_load in images.into_iter() {
        let image_reader = image::ImageReader::open(&to_load).unwrap();
        let image = image_reader.decode().unwrap();

        // Check if we should still be loading images before posting a new one
        if load_kill_receiver.try_recv().is_ok() {
            return;
        }

        log::trace!(
            "Loaded image {:?}",
            &to_load.file_name().unwrap_or(&to_load.as_os_str())
        );

        image_sender
            .send(ImageChannel::Image(to_load, image))
            .unwrap();
    }

    log::info!("Finished loading images");
    image_sender.send(ImageChannel::Finished).unwrap();
}

fn sys_check_loading(storage: Res<Storage>) -> bool {
    storage.loading
}

fn sys_process_new_images(device: Res<Device>, queue: Res<Queue>, mut storage: ResMut<Storage>) {
    let mut to_process = Vec::new();

    loop {
        match storage.image_receiver.try_recv() {
            Ok(image) => match image {
                ImageChannel::Image(path, image) => to_process.push((path, image)),

                ImageChannel::Finished => {
                    storage.loading = false;
                    break;
                }
            },
            Err(e) => match e {
                crossbeam_channel::TryRecvError::Empty => break,
                e => panic!("{}", e),
            },
        }
    }

    let mut textures = to_process
        .into_iter()
        .map(|(path, image)| {
            let texture = Texture::from_image(device.inner(), queue.inner(), &image, None, None);

            let mut hasher = ahash::AHasher::default();
            path.hash(&mut hasher);
            let key = hasher.finish();

            let resolution = image.dimensions().into();

            storage.textures.insert(
                key,
                TextureData {
                    texture,
                    path,
                    resolution,
                },
            );

            key
        })
        .collect::<Vec<_>>();

    storage.to_spawn.append(&mut textures);
}

fn sys_spawn_new_images(
    device: Res<Device>,
    pipeline: Res<TexturePipeline>,

    mut storage: ResMut<Storage>,
    mut layout: ResMut<LayoutManager>,

    mut image_creator: ImageCreator,
    mut vm_indexed: ViewMut<ImageIndex>,
) {
    storage.to_spawn.iter().for_each(|id| {
        let texture = storage.textures.get(id).unwrap();

        let image = StandardImage {
            id: *id,
            instance: TextureInstance::new(
                device.inner(),
                &pipeline,
                RawTextureInstance {
                    pos: [0., 0.],
                    size: [1., 1.],
                    color: [1., 1., 1., 1.],
                },
                &texture.texture,
            ),
        };

        let index = layout.next();

        let meta = ImageMeta {
            texture_resolution: texture.resolution,
            aspect: texture.resolution.height as f32 / texture.resolution.width as f32,
        };

        let entity_id = image_creator.spawn_image(image, meta);
        image_creator
            .entities
            .add_component(entity_id, &mut vm_indexed, ImageIndex { index });
    });

    storage.to_spawn.clear();
}

//====================================================================
