//====================================================================

use std::{
    env,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    time::Duration,
};

use ahash::AHashMap;
use cabat::{
    common::Size,
    renderer::{
        text::{Text2dBuffer, Text2dBufferDescriptor, TextFontSystem},
        texture, Device, Queue,
    },
    shipyard_tools::prelude::*,
};
use crossbeam_channel::{Receiver, Sender};
use image::{
    codecs::gif::GifDecoder, AnimationDecoder, DynamicImage, GenericImage, GenericImageView,
};
use shipyard::{AllStoragesView, SystemModificator, Unique, ViewMut, Workload};

use crate::{
    images::{GifImage, ImageCreator, ImageIndex, ImageMeta, StandardImage},
    layout::LayoutManager,
    renderer::{
        gif::{
            Gif, MAX_TEXTURE_HEIGHT, MAX_TEXTURE_WIDTH, MAX_USABLE_IMAGE_HEIGHT,
            MAX_USABLE_IMAGE_WIDTH,
        },
        gif2d_pipeline::{Gif2dInstance, Gif2dInstanceRaw, Gif2dPipeline},
        texture2d_pipeline::{Texture2dInstance, Texture2dInstanceRaw, Texture2dPipeline},
    },
};

//====================================================================

pub(crate) struct StoragePlugin;

impl Plugin for StoragePlugin {
    fn build(self, workload_builder: &WorkloadBuilder) {
        workload_builder
            .add_workload_pre(Stages::Setup, sys_setup_storage)
            .add_workload_pre(
                Stages::Update,
                (
                    sys_process_new_images.run_if(sys_check_loading),
                    sys_spawn_new_images.run_if(sys_check_pending),
                ),
            )
            .add_event::<LoadFolderEvent>(Workload::new("").with_system(sys_load_path));
    }
}

fn sys_setup_storage(all_storages: AllStoragesView, mut events: ResMut<EventHandler>) {
    all_storages.add_unique(Storage::new());

    let args: Vec<String> = env::args().collect();
    log::debug!("Args {:?}", args);

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
    Texture(texture::RawTexture),
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
    let duration = std::time::Instant::now();

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

                    let resize_image = image.width() > MAX_USABLE_IMAGE_WIDTH
                        || image.height() > MAX_USABLE_IMAGE_HEIGHT;

                    let image = match resize_image {
                        true => image.resize(
                            MAX_USABLE_IMAGE_WIDTH,
                            MAX_USABLE_IMAGE_HEIGHT,
                            image::imageops::FilterType::Nearest,
                        ),
                        false => image,
                    };

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

    log::info!(
        "Finished loading images - took {:.3} seconds",
        duration.elapsed().as_secs_f32()
    );
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

    let original_frame_width = frames[0].buffer().width();
    let original_frame_height = frames[0].buffer().height();

    // Shrink gifs if they are larger than they need to be
    let (frame_width, frame_height) = {
        let new_width = match original_frame_width > MAX_USABLE_IMAGE_WIDTH {
            true => MAX_USABLE_IMAGE_WIDTH,
            false => original_frame_width,
        };

        let new_height = match original_frame_height > MAX_USABLE_IMAGE_HEIGHT {
            true => MAX_USABLE_IMAGE_HEIGHT,
            false => original_frame_height,
        };

        let wratio = new_width as f32 / original_frame_width as f32;
        let hratio = new_height as f32 / original_frame_height as f32;
        let ratio = f32::min(wratio, hratio);

        (
            (original_frame_width as f32 * ratio).round() as u32,
            (original_frame_height as f32 * ratio).round() as u32,
        )
    };

    let frames_per_row = MAX_TEXTURE_WIDTH / frame_width;
    let total_rows = frames.len() as u32 / frames_per_row + 1;

    let texture_width = frame_width * frames_per_row;
    let texture_height = frame_height * total_rows;

    let data = match texture_height > MAX_TEXTURE_HEIGHT {
        true => {
            log::warn!(
                "Failed to load gif {:?} of {} frames and frame size ({}, {}). texure size ({}, {}) exceeds max texture size ({}, {})",
                &path.file_name().unwrap_or(&path.as_os_str()),
                frames.len(),
                frame_width,
                frame_height,
                texture_width,
                texture_height,
                MAX_TEXTURE_WIDTH,
                MAX_TEXTURE_HEIGHT
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

                    let frame_img = DynamicImage::from(frame.buffer().clone());
                    let frame_img = frame_img.resize(
                        frame_width,
                        frame_height,
                        image::imageops::FilterType::Nearest,
                    );

                    sub_img.copy_from(&frame_img, 0, 0).unwrap();
                    // sub_img.copy_from(frame.buffer(), 0, 0).unwrap();

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
                    let texture = texture::RawTexture::from_image(
                        device.inner(),
                        queue.inner(),
                        &image,
                        None,
                        None,
                    );

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
    mut font_system: ResMut<TextFontSystem>,

    mut storage: ResMut<Storage>,
    mut layout: ResMut<LayoutManager>,

    mut image_creator: ImageCreator,
    mut vm_indexed: ViewMut<ImageIndex>,
    mut vm_text: ViewMut<Text2dBuffer>,
) {
    storage.to_spawn.iter().for_each(|id| {
        let texture = storage.textures.get(id).unwrap();

        let index = layout.next();

        let meta = ImageMeta {
            texture_resolution: texture.resolution,
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
                Text2dBuffer::new(
                    font_system.inner_mut(),
                    &Text2dBufferDescriptor::new_text(
                        texture.path.file_name().unwrap().to_str().unwrap(),
                    ),
                ),
            ),
        );
    });

    storage.to_spawn.clear();
}

//====================================================================
