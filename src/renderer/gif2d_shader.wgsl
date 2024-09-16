//====================================================================
// Uniforms

struct Camera {
    projection: mat4x4<f32>,
    position: vec3<f32>,
}

struct Frames {
    total_frames: f32,
    frames_per_row: f32,

    sample_width: f32,
    sample_height: f32,
}

struct TextureInstance {
    pos: vec2<f32>,
    size: vec2<f32>,
    color: vec4<f32>,
    frame_x: f32,
    frame_y: f32,
}

@group(0) @binding(0) var<uniform> camera: Camera;

@group(1) @binding(0) var texture: texture_2d<f32>;
@group(1) @binding(1) var texture_sampler: sampler;
@group(1) @binding(2) var<uniform> frames: Frames; 

@group(2) @binding(0) var<uniform> instance: TextureInstance;

//====================================================================

struct VertexIn {
    // Vertex
    @location(0) vertex_position: vec2<f32>,
    @location(1) uv: vec2<f32>,
}

struct VertexOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
}

//====================================================================

@vertex
fn vs_main(in: VertexIn) -> VertexOut {
    var out: VertexOut;

    var vertex_pos = in.vertex_position 
        * instance.size 
        + instance.pos;

    out.clip_position = camera.projection 
        * vec4<f32>(vertex_pos, 2., 1.);

    out.uv = in.uv;
    out.color = instance.color;

    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {

    var uv: vec2<f32>;    
    uv.x = in.uv.x * frames.sample_width + (instance.frame_x * frames.sample_width);
    uv.y = in.uv.y * frames.sample_height + (instance.frame_y * frames.sample_height);
    
    let tex_color = textureSample(texture, texture_sampler, uv);

    return tex_color * in.color;
}

//====================================================================


