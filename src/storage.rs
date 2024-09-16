//====================================================================

use std::{
    env,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    time::Duration,
};

use ahash::AHashMap;
use crossbeam_channel::{Receiver, Sender};
use image::{
    codecs::gif::GifDecoder, AnimationDecoder, DynamicImage, GenericImage, GenericImageView,
};
use shipyard::{AllStoragesView, IntoWorkload, SystemModificator, Unique, ViewMut, Workload};

use crate::{
    app::Stages,
    images::{GifImage, ImageCreator, ImageIndex, ImageMeta, StandardImage},
    layout::LayoutManager,
    renderer::{
        gif2d_pipeline::{Gif2dInstance, Gif2dInstanceRaw, Gif2dPipeline},
        text_pipeline::{TextBuffer, TextBufferDescriptor, TextPipeline},
        texture::{Gif, Texture, MAX_TEXTURE_HEIGHT, MAX_TEXTURE_WIDTH},
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

    _load_kill_sender: Sender<bool>,
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
    Gif { gif: Gif, frames: Vec<Duration> },
}

//====================================================================

enum ImageChannel {
    Finished,
    Image {
        path: PathBuf,
        image: DynamicImage,
    },
    Gif {
        path: PathBuf,
        image: DynamicImage,
        total_frames: u32,
        frames_per_row: u32,
        total_rows: u32,
        frame_size: (u32, u32),
        frame_delay: Vec<Duration>,
    },
}

impl Storage {
    pub fn new() -> Self {
        let (load_kill_sender, load_kill_receiver) = crossbeam_channel::unbounded();

        let (image_sender, image_receiver) = crossbeam_channel::unbounded();

        Self {
            textures: AHashMap::new(),

            loading: false,
            to_spawn: Vec::new(),

            _load_kill_sender: load_kill_sender,
            load_kill_receiver,
            image_sender,
            image_receiver,
        }
    }

    pub fn _stop_loading(&mut self) {
        self._load_kill_sender.send(true).ok();
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

    // TODO - Spawn multiple threads
    std::thread::spawn(move || load_images(images_to_load, load_kill_receiver, image_sender));
}

fn load_images(
    images: Vec<PathBuf>,
    load_kill_receiver: Receiver<bool>,
    image_sender: Sender<ImageChannel>,
) {
    for path in images.into_iter() {
        let data = match path.extension() {
            None => {
                log::trace!("Skipping file path '{:?}'", &path);
                continue;
            }
            Some(ext) => match ext.to_str() {
                Some("jpg") | Some("png") => {
                    let image_reader = image::ImageReader::open(&path).unwrap();
                    let image = image_reader.decode().unwrap();

                    ImageChannel::Image { path, image }
                }

                Some("gif") => load_gif(path).unwrap(),

                _ => continue,
            },
        };

        // Check if we should still be loading images before posting a new one
        // TODO - Already loaded the data at this point so check should probably be moved to receiver instead
        if load_kill_receiver.try_recv().is_ok() {
            return;
        }

        match &data {
            ImageChannel::Image { path, .. } => {
                log::trace!(
                    "Loaded image {:?}",
                    &path.file_name().unwrap_or(&path.as_os_str())
                )
            }
            ImageChannel::Gif {
                path,
                total_frames,
                frames_per_row,
                frame_size,
                ..
            } => {
                log::trace!(
                    "Loaded gif   {:?} - total frames '{}', frames per row '{}', frame size: {:?}",
                    &path.file_name().unwrap_or(&path.as_os_str()),
                    total_frames,
                    frames_per_row,
                    frame_size,
                )
            }

            _ => {}
        }

        image_sender.send(data).unwrap();
    }

    log::info!("Finished loading images");
    image_sender.send(ImageChannel::Finished).unwrap();
}

fn load_gif(path: PathBuf) -> Option<ImageChannel> {
    let file = std::fs::File::open(path.clone()).ok()?;
    let reader = std::io::BufReader::new(file);
    let gif = GifDecoder::new(reader).unwrap();

    let frames = gif.into_frames().collect_frames().ok()?;

    if frames.is_empty() {
        return None;
    }

    let frame_width = frames[0].buffer().width();
    let frame_height = frames[0].buffer().height();

    let frames_per_row = MAX_TEXTURE_WIDTH / frame_width;
    let total_rows = frames.len() as u32 / frames_per_row + 1;

    let texture_width = frame_width * frames_per_row;
    let texture_height = frame_height * total_rows;

    let data = match texture_height > MAX_TEXTURE_HEIGHT {
        true => {
            log::warn!(
                "Failed to load gif {:?} with size ({}, {}) (too large or too many frames)",
                &path.file_name().unwrap_or(&path.as_os_str()),
                frame_width,
                frame_height
            );

            let image = DynamicImage::from(frames[0].buffer().clone());

            ImageChannel::Gif {
                path,
                image,
                total_frames: 1,
                frames_per_row: 1,
                total_rows: 1,
                frame_size: (frame_width, frame_height),
                frame_delay: vec![Duration::from_secs(99999)],
            }
        }
        false => {
            //

            let mut image = DynamicImage::new_rgba8(texture_width, texture_height);

            let frame_delay = frames
                .iter()
                .enumerate()
                .map(|(index, frame)| {
                    let mut sub_img = image.sub_image(
                        index as u32 % frames_per_row * frame_width,
                        index as u32 / frames_per_row * frame_height,
                        frame_width,
                        frame_height,
                    );

                    sub_img.copy_from(frame.buffer(), 0, 0).unwrap();

                    let millis = frame.delay().numer_denom_ms().0;
                    let delay = Duration::from_millis(millis as u64);

                    delay
                })
                .collect::<Vec<_>>();

            ImageChannel::Gif {
                path,
                image,
                total_frames: frames.len() as u32,
                frames_per_row,
                total_rows,
                frame_size: (frame_width, frame_height),
                frame_delay,
            }
        }
    };

    Some(data)
}

fn sys_check_loading(storage: Res<Storage>) -> bool {
    storage.loading
}

fn sys_check_pending(storage: Res<Storage>) -> bool {
    !storage.to_spawn.is_empty()
}

fn sys_process_new_images(device: Res<Device>, queue: Res<Queue>, mut storage: ResMut<Storage>) {
    loop {
        let mut hasher = ahash::AHasher::default();

        let texture_data = match storage.image_receiver.try_recv() {
            Ok(image) => match image {
                ImageChannel::Image { path, image } => {
                    let texture =
                        Texture::from_image(device.inner(), queue.inner(), &image, None, None);

                    let resolution = image.dimensions().into();

                    path.hash(&mut hasher);

                    Some(TextureData {
                        texture: TextureType::Texture(texture),
                        path,
                        resolution,
                    })
                }

                ImageChannel::Gif {
                    path,
                    image,
                    total_frames,
                    frames_per_row,
                    total_rows,
                    frame_size,
                    frame_delay,
                } => {
                    path.hash(&mut hasher);

                    let resolution = Size::new(frame_size.0, frame_size.1);

                    let gif = Gif::new(
                        device.inner(),
                        queue.inner(),
                        path.file_name()
                            .unwrap_or(path.as_os_str())
                            .to_str()
                            .unwrap(),
                        image,
                        total_frames,
                        frames_per_row,
                        total_rows,
                        frame_size.0,
                        frame_size.1,
                    );

                    Some(TextureData {
                        texture: TextureType::Gif {
                            gif,
                            frames: frame_delay,
                        },
                        path,
                        resolution,
                    })
                }

                ImageChannel::Finished => {
                    storage.loading = false;
                    None
                }
            },
            Err(e) => match e {
                crossbeam_channel::TryRecvError::Empty => break,
                e => panic!("{}", e),
            },
        };

        if let Some(texture_data) = texture_data {
            let key = hasher.finish();

            storage.textures.insert(key, texture_data);

            storage.to_spawn.push(key);
        }
    }
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

            TextureType::Gif { gif, frames } => {
                let gif = GifImage {
                    id: *id,
                    frame: 0,
                    total_frames: gif.total_frames,
                    frames_per_row: gif.frames_per_row,
                    instance: Gif2dInstance::new(
                        device.inner(),
                        &gif_pipeline,
                        Gif2dInstanceRaw::default(),
                        gif,
                    ),
                };

                image_creator.spawn_gif(gif, frames, meta)
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
