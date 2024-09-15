//====================================================================
// Uniforms

struct Camera {
    projection: mat4x4<f32>,
    position: vec3<f32>,
}

struct Frames {
    total_frames: f32,
    frames_per_row: f32,
    total_rows: f32,

    frame_width: f32,
    frame_height: f32,
}

struct TextureInstance {
    pos: vec2<f32>,
    size: vec2<f32>,
    color: vec4<f32>,
    frame: f32,
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
    let frame = instance.frame % frames.total_frames;

    let x = frame % frames.frames_per_row;
    let y = floor(frame / frames.frames_per_row);

    let texture_width = frames.frame_width * frames.frames_per_row;
    let sample_width = frames.frame_width / texture_width;

    let texture_height = frames.frame_height * frames.total_rows;
    let sample_height = frames.frame_height / texture_height;

    var uv: vec2<f32>;    
    uv.x = in.uv.x * sample_width + (x * sample_width);
    uv.y = in.uv.y * sample_height + (y * sample_height);
    // uv.x = 0.16666666666666;
    // uv.y = 0.5;
    // uv.x = sample_width;
    // uv.y = sample_height;

    // uv.x = in.uv.x;
    // uv.y = in.uv.y;
    
    let tex_color = textureSample(texture, texture_sampler, uv);

    // let tex_color = textureSample(texture, texture_sampler, in.uv);
    return tex_color * in.color;
}

//====================================================================


