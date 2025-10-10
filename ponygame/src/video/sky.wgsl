struct ViewportUniform {
    // TODO:
    // Apparently if we use mat4x3 here, it has an expected size of 64. It's
    // not clear to me exactly what layout is expected in that case.
    view_proj_matrix: mat4x4f,
    view: mat4x4f,
    proj: mat4x4f,
    inv_view_proj_dir: mat4x4f,
}

@group(0) @binding(0) var<uniform> viewport: ViewportUniform;
// @group(0) @binding(1) var<uniform> lights: array<Light, 1>;
@group(0) @binding(2) var envmap_t: texture_2d<f32>;
@group(0) @binding(3) var envmap_s: sampler;

struct VertexOutput {
    @builtin(position) builtin_position: vec4f,
    @location(0)       clip_position: vec4f,
};

@vertex
fn vs_main(
    @builtin(vertex_index) in_vertex_index: u32,
) -> VertexOutput {
    var out: VertexOutput;

    let pos = array(
        vec2f( 3, -1),
        vec2f(-1,  3),
        vec2f(-1, -1),
    );

    out.clip_position = vec4f(pos[in_vertex_index], 1.0, 1.0);
    out.builtin_position = out.clip_position;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    // Slightly simpler setup as per webgpu fundamentals
    let t = viewport.inv_view_proj_dir * in.clip_position;
    let ray = normalize(t.xyz / t.w) * vec3f(1, 1, -1);

    // Cubemap sample:
    // let sample = textureSample(skybox_t, skybox_s, ray_direction);

    let PI = 3.141592653589793238462643383;

    // Panoramic sample:
    let u = (atan2(ray.z, ray.x) / (2.0 * PI)) + 0.5;
    let v = (-asin(ray.y) / PI) + 0.5;
    let sample = textureSampleLevel(envmap_t, envmap_s, vec2f(u, v), 0.0);

    return sample * 1.0;
}