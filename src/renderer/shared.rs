//====================================================================

use super::Vertex;

//====================================================================

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Zeroable, bytemuck::Pod)]
pub struct RawTextureVertex {
    pos: [f32; 2],
    uv: [f32; 2],
}

impl Vertex for RawTextureVertex {
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        const VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 2] = wgpu::vertex_attr_array![
                0 => Float32x2, 1 => Float32x2
        ];

        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<RawTextureVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &VERTEX_ATTRIBUTES,
        }
    }
}

pub const TEXTURE_VERTICES: [RawTextureVertex; 4] = [
    RawTextureVertex {
        pos: [-0.5, 0.5],
        uv: [0., 0.],
    },
    RawTextureVertex {
        pos: [-0.5, -0.5],
        uv: [0., 1.],
    },
    RawTextureVertex {
        pos: [0.5, 0.5],
        uv: [1., 0.],
    },
    RawTextureVertex {
        pos: [0.5, -0.5],
        uv: [1., 1.],
    },
];

pub const TEXTURE_INDICES: [u16; 6] = [0, 1, 3, 0, 3, 2];

//====================================================================
