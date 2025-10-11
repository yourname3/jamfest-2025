const PI = 3.141592653589793238462643383;

struct ViewportUniform {
    // TODO:
    // Apparently if we use mat4x3 here, it has an expected size of 64. It's
    // not clear to me exactly what layout is expected in that case.
    view_proj_matrix: mat4x4f,
    view: mat4x4f,
    proj: mat4x4f,
    inv_view_proj_dir: mat4x4f,
}

struct PBR {
    albedo: vec3f,
    metallic: f32,
    roughness: f32,
    reflectance: f32,
}

struct InstanceData {
    @location(4) row0: vec3f,
    @location(5) row1: vec3f,
    @location(6) row2: vec3f,
    @location(7) row3: vec3f,
}

struct Light {
    // Eye-space directions.
    direction: vec3f,
    color: vec3f,
}

struct ModelUniform {
    transform: mat4x4f,
}

// Future group layout:
// Group 0: World: includes viewport, environment map, lights
// Group 1: Material: includes material properties, textures
// Group 2: Model: (Maybe replaced by instance data one day):
//          includes transform matrix, per-model tweaks (color?)
//
// The fundamental render process looks something like:
//
// for world in worlds {
//     
//     for (camera, viewport) in world.cam_viewport_pairs() {
//         queue.write_buffer(matrix);
//         // Probably we need a bind group per-triple of
//         // (world, camera, viewport), because I don't think
//         // it's possible to just re-use one uniform buffer and
//         // keep re-writing to it
//         pass.set_bind_group(0, (triple).bind_group);
//
//         mesh_pipeline.render_meshes(world);
//         text_pipeline.render_text(world);
//         
//         postprocessing.render();
//     }
// }

@group(0) @binding(0) var<uniform> viewport: ViewportUniform;
@group(0) @binding(1) var<uniform> lights: array<Light, 1>;
@group(0) @binding(2) var envmap_t: texture_2d<f32>;
@group(0) @binding(3) var envmap_s: sampler;

@group(1) @binding(0) var<uniform> pbr: PBR;
@group(1) @binding(1) var albedo_t: texture_2d<f32>;
@group(1) @binding(2) var metallic_rough_t: texture_2d<f32>;
@group(1) @binding(3) var albedo_decal_t: texture_2d<f32>;
@group(1) @binding(4) var metallic_rough_decal_t: texture_2d<f32>;
@group(1) @binding(5) var pbr_s: sampler;

@group(2) @binding(0) var<uniform> model: ModelUniform;

struct VertexInput {
    @location(0) position: vec3f,
    @location(1) normal  : vec3f,
    @location(2) uv      : vec2f,
    @location(3) uv2     : vec2f,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4f,

    // eye-space
    @location(0) f_pos   : vec3f,
    @location(1) f_normal: vec3f,
    @location(2) uv      : vec2f,
    @location(3) uv2     : vec2f,
}

fn expand_transformation_matrix(in: mat4x3f) -> mat4x4f {
    return mat4x4f(
        vec4f(in[0], 0.0),
        vec4f(in[1], 0.0),
        vec4f(in[2], 0.0),
        vec4f(in[3], 0.0)
    );
}

@vertex
fn vs_main(
    @builtin(vertex_index) in_vertex_index: u32,
    vertex: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;

    let m: mat4x4f = model.transform;
    // let m: mat4x4f = mat4x4f(
    //     1.0, 0.0, 0.0, 0.0,
    //     0.0, 1.0, 0.0, 0.0,
    //     0.0, 0.0, 1.0, 0.0,
    //     0.0, 0.0, 0.0, 1.0,
    // );
    let vp: mat4x4f = viewport.view_proj_matrix; //expand_transformation_matrix(uniforms.view_proj_matrix);

    let mvp: mat4x4f = vp * m;

    out.clip_position = mvp * vec4f(vertex.position, 1.0);
    out.f_pos = (viewport.view * m * vec4f(vertex.position, 1.0)).xyz;
    // Cheap approximation of normal for now. For good-looking normals we need
    // a normal matrix (inverse transpose(v * m), but I'm not sure yet how I
    // want to do that with instanced rendering, and I probably don't care for it
    // at all with armatures.
    out.f_normal = (viewport.view * m * vec4(vertex.normal, 0.0)).xyz;
    out.uv = vertex.uv;
    out.uv2 = vertex.uv2;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let norm = normalize(in.f_normal);

    let albedo = textureSample(albedo_t, pbr_s, in.uv).rgba;

    var light = clamp(dot(norm, normalize(vec3f(-0.5, -0.5, 1.0))), 0.0, 1.0);
    light += 0.1;
    light = clamp(light, 0.0, 1.0);
    let color = light * albedo.rgb;

    return vec4f(color, 1.0);
}

struct BrdfIn {
    diffuse_color: vec3f,
    f0: vec3f,
    roughness: f32,

    n: vec3f,
    l: vec3f,
    v: vec3f,
};

struct BrdfOut {
    NoV: f32,
    NoL: f32,
    NoH: f32,
    LoH: f32,

    D: f32,
    F: vec3f,
    V: f32,

    Fd: vec3f,
    Fr: vec3f,

    brdf: vec3f,

    h: vec3f,
};

fn D_GGX(NoH: f32, a: f32) -> f32 {
    let a2 = a * a;
    let f = (NoH * a2 - NoH) * NoH + 1.0;
    return a2 / (PI * f * f);
}

fn F_Schlick(u: f32, f0: vec3f) -> vec3f {
    return f0 + (vec3f(1.0) - f0) * pow(1.0 - u, 5.0);
}

fn V_SmithGGXCorrelated(NoV: f32, NoL: f32, a: f32) -> f32 {
    let a2 = a * a;
    let GGXL = NoV * sqrt((-NoL * a2 + NoL) * NoL + a2);
    let GGXV = NoL * sqrt((-NoV * a2 + NoV) * NoV + a2);
    return 0.5 / (GGXV + GGXL);
}

fn Fd_Lambert() -> f32 {
    return 1.0 / PI;
}

fn BRDF_Fr_h(bin: BrdfIn, h: vec3f) -> BrdfOut {
    let v = bin.v;
    let n = bin.n;
    let l = bin.l;

    var bout: BrdfOut;
    bout.h = h;

    bout.NoV = abs(dot(n, v)) + 1e-5;
    bout.NoL = clamp(dot(n, l), 0.0, 1.0);
    bout.NoH = clamp(dot(n, h), 0.0, 1.0);
    bout.LoH = clamp(dot(l, h), 0.0, 1.0);

    bout.D = D_GGX(bout.NoH, bin.roughness);
    bout.F = F_Schlick(bout.LoH, bin.f0);
    bout.V = V_SmithGGXCorrelated(bout.NoV, bout.NoL, bin.roughness);

    // Specular BRDF
    // Note that this formula multiplies by D even though we often need to
    // divide by D. This is not the most efficient--we could simply delete the
    // multiplication and division in that case--but to ensure that all the
    // shaders are using the same BRDF I did it this way for now.
    bout.Fr = (bout.D * bout.V) * bout.F;

    return bout;
}

fn BRDF_Fr(bin: BrdfIn) -> BrdfOut {
    return BRDF_Fr_h(bin, normalize(bin.v + bin.l));
}

fn BRDF(bin: BrdfIn) -> BrdfOut {
    var bout = BRDF_Fr(bin);
    bout.Fd = bin.diffuse_color * Fd_Lambert();
    return bout;
}

fn BRDF_attentuated(bin: BrdfIn) -> vec3f {
    let b = BRDF(bin);
    return (b.Fd + b.Fr) * b.NoL;
}

// For now, this is a panoramic texture coordinate lookup.
fn sample_envmap(ray: vec3f, level: f32) -> vec4f {
    let u = (atan2(ray.z, ray.x) / (2.0 * PI)) + 0.5;
    let v = (-asin(ray.y) / PI) + 0.5;
    let sample = textureSampleLevel(envmap_t, envmap_s, vec2f(u, v), level);
    return sample;
}

fn BRDF_envmap(bin_immut: BrdfIn) -> vec3f {
    var bin = bin_immut;
    let max_level = 1.0 + floor(log2(f32(textureDimensions(envmap_t, 0).x)));

    let Fd = bin.diffuse_color * sample_envmap(bin.n, max_level - 1).rgb;

    bin.l = reflect(-bin.v, bin.n);

    let Il = sample_envmap(bin.l, (max_level - 1) * pow(bin.roughness, 0.125)).rgb;

    let b = BRDF_Fr(bin);

    let Fr = 4.0 * b.NoV * b.Fr * Il * b.NoL / b.D;

    return Fd + Fr;
}

@fragment
fn pbr_main(in: VertexOutput) -> @location(0) vec4f {
    let albedo_decal = textureSample(albedo_decal_t, pbr_s, in.uv2);

    var metallic = pbr.metallic;
    var perceptual_roughness = pbr.roughness;
    // TODO: Should this be gated behind a flag? Is the dummy texture lookup
    // slow enough?
    if true {
        var data = textureSample(metallic_rough_t, pbr_s, in.uv);
        
        if albedo_decal.a > 0 {
            let data2 = textureSample(metallic_rough_decal_t, pbr_s, in.uv2);
            data = mix(data, data2, albedo_decal.a);
        }
        // Compute based on textures
        metallic *= data.b;
        perceptual_roughness *= data.g;
    }

    var reflectance = pbr.reflectance;

    var base_color = vec3(1.0, 1.0, 1.0);
    if true {
        var albedo_tex = textureSample(albedo_t, pbr_s, in.uv).rgb;
        albedo_tex = mix(albedo_tex, albedo_decal.rgb, albedo_decal.a);
        base_color *= albedo_tex;
    }

    var diffuse_color = (1.0 - metallic) * base_color;
    var f0 = vec3f(0.16 * reflectance * reflectance * (1.0 - metallic)) + base_color * metallic;
    var roughness = clamp(perceptual_roughness * perceptual_roughness, 0.01, 1.0);

    var normal = normalize(in.f_normal);
    if false {
        // compute normal based on eye-space tangent, bitangent, normal
    }

    var bin: BrdfIn;
    bin.diffuse_color = diffuse_color;
    bin.f0 = f0;
    bin.roughness = roughness;
    bin.v = -normalize(in.f_pos);
    bin.n = normal;

    var sum = vec3(0.0);
    for(var i = 0; i < 1; i += 1) {
        // These are currently passed in eye-space.
        var l_direction = lights[i].direction;

        bin.l = -normalize(l_direction);
        let brdf = BRDF_attentuated(bin);
        sum += brdf * lights[i].color;
    }

    if true {
        sum += BRDF_envmap(bin);
    }

    if false {
        // emission
    }

    // TODO: Instead of clamping here, write to an HDR texture
    // then tonemap
    //return vec4f(clamp(sum, vec3f(0.0), vec3f(1.0)), 1.0);
    return vec4f(sum, 1.0);
}