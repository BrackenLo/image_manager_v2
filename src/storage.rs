//====================================================================

use std::{
    hash::{Hash, Hasher},
    path::PathBuf,
};

use ahash::AHashMap;
use crossbeam_channel::{Receiver, Sender};
use image::DynamicImage;
use shipyard::Unique;

use crate::{
    images::{Image, ImageCreator},
    layout::LayoutManager,
    renderer::{
        texture::Texture,
        texture_pipeline::{RawTextureInstance, TextureInstance, TexturePipeline},
        Device, Queue,
    },
    tools::{Res, ResMut},
};

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
    texture: Texture,
    path: PathBuf,
}

//====================================================================

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
}

pub(crate) fn sys_load_path(path: PathBuf, mut storage: ResMut<Storage>) {
    log::info!("Loading images from path '{:?}'", path);

    let dir = std::fs::read_dir(path).unwrap();
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

pub(crate) fn sys_check_loading(storage: Res<Storage>) -> bool {
    storage.loading
}

pub(crate) fn sys_process_new_images(
    device: Res<Device>,
    queue: Res<Queue>,
    mut storage: ResMut<Storage>,
) {
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

            storage.textures.insert(key, TextureData { texture, path });

            key
        })
        .collect::<Vec<_>>();

    storage.to_spawn.append(&mut textures);
}

pub(crate) fn sys_spawn_new_images(
    device: Res<Device>,
    pipeline: Res<TexturePipeline>,

    mut storage: ResMut<Storage>,
    mut layout: ResMut<LayoutManager>,

    mut image_creator: ImageCreator,
) {
    storage.to_spawn.iter().for_each(|id| {
        let texture = storage.textures.get(id).unwrap();

        let image = Image {
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

        image_creator.spawn(image, index);
    });

    storage.to_spawn.clear();
}

//====================================================================
