//====================================================================

use std::{
    env,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};

use ahash::AHashMap;
use crossbeam_channel::{Receiver, Sender};
use image::{codecs::gif::GifDecoder, AnimationDecoder, DynamicImage, GenericImageView};
use shipyard::{AllStoragesView, IntoWorkload, SystemModificator, Unique, ViewMut, Workload};

use crate::{
    app::Stages,
    images::{GifImage, ImageCreator, ImageIndex, ImageMeta, StandardImage},
    layout::LayoutManager,
    renderer::{
        gif2d_pipeline::{Gif2dInstance, Gif2dInstanceRaw, Gif2dPipeline},
        text_pipeline::{TextBuffer, TextBufferDescriptor, TextPipeline},
        texture::{Gif, Texture},
        texture2d_pipeline::{Texture2dInstance, Texture2dInstanceRaw, Texture2dPipeline},
        Device, Queue,
    },
    shipyard_tools::{Event, EventHandler, Plugin, Res, ResMut},
    tools::Size,
};

//====================================================================

pub(crate) struct StoragePlugin;

impl Plugin<Stages> for StoragePlugin {
    fn build(&self, workload_builder: &mut crate::shipyard_tools::WorkloadBuilder<Stages>) {
        workload_builder
            .add_workload(Stages::PreSetup, (sys_setup_storage).into_workload())
            .add_workload(
                Stages::PreUpdate,
                (
                    sys_process_new_images.run_if(sys_check_loading),
                    sys_spawn_new_images.run_if(sys_check_pending),
                )
                    .into_workload(),
            )
            .add_event::<LoadFolderEvent>(Workload::new("").with_system(sys_load_path));
    }
}

fn sys_setup_storage(all_storages: AllStoragesView, mut events: ResMut<EventHandler>) {
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

    events.add_event(LoadFolderEvent { path });
}

#[derive(Event)]
pub struct LoadFolderEvent {
    path: PathBuf,
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
    pub texture: TextureType,
    pub path: PathBuf,
    pub resolution: Size<u32>,
}

pub enum TextureType {
    Texture(Texture),
    Gif(Gif),
}

//====================================================================

enum ImageChannel {
    Finished,
    Image(PathBuf, DynamicImage),
    Gif(PathBuf, Vec<image::Frame>),
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

fn sys_load_path(events: Res<EventHandler>, mut storage: ResMut<Storage>) {
    let to_load = events.get_event::<LoadFolderEvent>().unwrap();

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

fn load_images(
    images: Vec<PathBuf>,
    load_kill_receiver: Receiver<bool>,
    image_sender: Sender<ImageChannel>,
) {
    for to_load in images.into_iter() {
        let data = match to_load.extension() {
            None => {
                log::trace!("Skipping file path '{:?}'", &to_load);
                continue;
            }
            Some(ext) => match ext.to_str() {
                Some("jpg") | Some("png") => {
                    let image_reader = image::ImageReader::open(&to_load).unwrap();
                    let image = image_reader.decode().unwrap();

                    ImageChannel::Image(to_load, image)
                }

                Some("gif") => {
                    let file = std::fs::File::open(to_load.clone()).unwrap();
                    let reader = std::io::BufReader::new(file);
                    let gif = GifDecoder::new(reader).unwrap();
                    let frames = gif.into_frames();
                    let frames = frames.collect_frames().unwrap();

                    ImageChannel::Gif(to_load, frames)
                }

                _ => continue,
            },
        };

        // Check if we should still be loading images before posting a new one
        if load_kill_receiver.try_recv().is_ok() {
            return;
        }

        if let ImageChannel::Image(buf, _) | ImageChannel::Gif(buf, _) = &data {
            log::trace!(
                "Loaded image {:?}",
                &buf.file_name().unwrap_or(&buf.as_os_str())
            );
        }

        image_sender.send(data).unwrap();
    }

    log::info!("Finished loading images");
    image_sender.send(ImageChannel::Finished).unwrap();
}

fn sys_check_loading(storage: Res<Storage>) -> bool {
    storage.loading
}

fn sys_check_pending(storage: Res<Storage>) -> bool {
    !storage.to_spawn.is_empty()
}

fn sys_process_new_images(device: Res<Device>, queue: Res<Queue>, mut storage: ResMut<Storage>) {
    let mut textures_to_process = Vec::new();
    let mut gifs_to_process = Vec::new();

    loop {
        match storage.image_receiver.try_recv() {
            Ok(image) => match image {
                ImageChannel::Image(path, image) => textures_to_process.push((path, image)),

                ImageChannel::Gif(path, frames) => gifs_to_process.push((path, frames)),

                ImageChannel::Finished => {
                    storage.loading = false;
                }
            },
            Err(e) => match e {
                crossbeam_channel::TryRecvError::Empty => break,
                e => panic!("{}", e),
            },
        }
    }

    let mut textures = textures_to_process
        .into_iter()
        .map(|(path, image)| {
            let texture = Texture::from_image(device.inner(), queue.inner(), &image, None, None);
            let texture = TextureType::Texture(texture);

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

    let mut gifs = gifs_to_process
        .into_iter()
        .filter_map(|(path, frames)| {
            if frames.is_empty() {
                return None;
            }

            let mut hasher = ahash::AHasher::default();
            path.hash(&mut hasher);
            let key = hasher.finish();

            let buffer = frames[0].buffer();
            let resolution = Size::new(buffer.width(), buffer.height());

            // TODO - Turn this into an async job
            let gif = Gif::from_frames(device.inner(), queue.inner(), frames);

            storage.textures.insert(
                key,
                TextureData {
                    texture: TextureType::Gif(gif),
                    path,
                    resolution,
                },
            );

            Some(key)
        })
        .collect::<Vec<_>>();

    storage.to_spawn.append(&mut textures);
    storage.to_spawn.append(&mut gifs);
}

fn sys_spawn_new_images(
    device: Res<Device>,
    texture_pipeline: Res<Texture2dPipeline>,
    gif_pipeline: Res<Gif2dPipeline>,
    mut text_pipeline: ResMut<TextPipeline>,

    mut storage: ResMut<Storage>,
    mut layout: ResMut<LayoutManager>,

    mut image_creator: ImageCreator,
    mut vm_indexed: ViewMut<ImageIndex>,
    mut vm_text: ViewMut<TextBuffer>,
) {
    storage.to_spawn.iter().for_each(|id| {
        let texture = storage.textures.get(id).unwrap();

        let index = layout.next();

        let meta = ImageMeta {
            _texture_resolution: texture.resolution,
            aspect: texture.resolution.height as f32 / texture.resolution.width as f32,
        };

        let entity_id = match &texture.texture {
            TextureType::Texture(texture) => {
                let image = StandardImage {
                    id: *id,
                    instance: Texture2dInstance::new(
                        device.inner(),
                        &texture_pipeline,
                        Texture2dInstanceRaw::default(),
                        texture,
                    ),
                };

                image_creator.spawn_image(image, meta)
            }

            TextureType::Gif(gif) => {
                let gif = GifImage {
                    id: *id,
                    frame: 0,
                    instance: Gif2dInstance::new(
                        device.inner(),
                        &gif_pipeline,
                        Gif2dInstanceRaw::default(),
                        gif,
                    ),
                };

                image_creator.spawn_gif(gif, meta)
            }
        };

        image_creator.entities.add_component(
            entity_id,
            (&mut vm_indexed, &mut vm_text),
            (
                ImageIndex { index },
                TextBuffer::new(
                    &mut text_pipeline,
                    &TextBufferDescriptor::new_text(
                        texture.path.file_name().unwrap().to_str().unwrap(),
                    ),
                ),
            ),
        );
    });

    storage.to_spawn.clear();
}

//====================================================================
