//====================================================================

use std::{collections::HashMap, ops::Range, time::Duration};

use image::DynamicImage;
use shipyard_renderer::texture;
use wgpu::util::DeviceExt;

//====================================================================

pub const MAX_TEXTURE_WIDTH: u32 = 8192;
pub const MAX_TEXTURE_HEIGHT: u32 = 8192;

pub const MAX_USABLE_IMAGE_WIDTH: u32 = 1920 / 2;
pub const MAX_USABLE_IMAGE_HEIGHT: u32 = 1080;

//====================================================================

pub struct GifFrameDelay {
    delays: HashMap<Range<u32>, Duration>,
}

impl GifFrameDelay {
    pub fn from_durations(delays: &Vec<Duration>) -> Self {
        if delays.is_empty() {
            log::warn!("Gif Frame Delay created with zero length vector");
            return Self {
                delays: HashMap::new(),
            };
        }

        let mut delays_final = HashMap::new();
        let mut start_index = 0;
        let mut prev = delays[0];

        delays
            .iter()
            .enumerate()
            .skip(1)
            .for_each(|(index, delay)| {
                if *delay == prev {
                    return;
                }

                let index = index as u32;
                delays_final.insert(start_index..index, prev);

                start_index = index;
                prev = *delay;
            });

        let final_index = delays.len() as u32;

        delays_final.insert(start_index..final_index, prev);

        Self {
            delays: delays_final,
        }
    }

    pub fn get_delay(&self, frame: &u32) -> Duration {
        let val = self.delays.iter().find(|(key, _)| key.contains(frame));

        match val {
            Some((_, key)) => *key,
            None => {
                log::warn!("Get delay: frame {} out of range", frame);
                Duration::ZERO
            }
        }
    }
}

pub struct Gif {
    pub texture: texture::Texture,
    pub buffer: wgpu::Buffer,
    pub total_frames: u32,
    pub frames_per_row: u32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Zeroable, bytemuck::Pod, Default)]
pub struct GifRawData {
    pub total_frames: f32,
    pub frames_per_row: f32,
    pub sample_width: f32,
    pub sample_height: f32,
}

impl Gif {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: &str,

        image: DynamicImage,
        total_frames: u32,
        frames_per_row: u32,
        total_rows: u32,
        frame_width: u32,
        frame_height: u32,
    ) -> Self {
        let texture = texture::Texture::from_image(device, queue, &image, None, None);

        let texture_width = frame_width * frames_per_row;
        let sample_width = frame_width as f32 / texture_width as f32;

        let texture_height = frame_height * total_rows;
        let sample_height = frame_height as f32 / texture_height as f32;

        let raw_data = GifRawData {
            total_frames: total_frames as f32,
            frames_per_row: frames_per_row as f32,
            sample_width,
            sample_height,
        };

        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{} gif buffer", label)),
            contents: bytemuck::cast_slice(&[raw_data]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        Self {
            texture,
            buffer,
            total_frames,
            frames_per_row,
        }
    }
}

//====================================================================
