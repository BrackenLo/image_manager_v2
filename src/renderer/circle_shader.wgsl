//====================================================================
// Uniforms

struct Camera {
    projection: mat4x4<f32>,
    position: vec3<f32>,
}

@group(0) @binding(0) var<uniform> camera: Camera;

//====================================================================

struct VertexIn {
    // Vertex
    @location(0) vertex_pos: vec2<f32>,
    // Instance
    @location(1) pos: vec2<f32>,
    @location(2) radius: f32,
    @location(3) border_radius: f32,
    @location(4) color: vec4<f32>,
    @location(5) border_color: vec4<f32>,
}

struct VertexOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) pos: vec2<f32>,
    @location(1) center: vec2<f32>,
    @location(2) radius: f32,
    @location(3) border_radius: f32,
    @location(4) color: vec4<f32>,
    @location(5) border_color: vec4<f32>,
}

//====================================================================

@vertex
fn vs_main(in: VertexIn) -> VertexOut {
    var out: VertexOut;

    var vertex_pos = in.vertex_pos 
        * (in.radius * 2. + in.border_radius * 2.) 
        + in.pos;

    out.clip_position = camera.projection 
        * vec4<f32>(vertex_pos, 0., 1.);

    out.pos = vertex_pos;
    out.center = in.pos;

    out.radius = in.radius;
    out.border_radius = in.border_radius;

    out.color = in.color;
    out.border_color = in.border_color;

    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let distance = distance(in.pos, in.center);

    if distance < in.radius {
        if in.color.w == 0. {
            discard;
        }
        return in.color;
    }
    if distance <= in.radius + in.border_radius {
        if in.border_color.w == 0. {
            discard;
        }
        return in.border_color;
    }

    discard;
}

//====================================================================

