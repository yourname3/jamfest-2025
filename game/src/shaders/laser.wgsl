fn pbr_fn(in: VertexOutput) -> PBROut {
    var out: PBROut = pbr_basic(in);

    let t = dot(vec3f(0.0, 0.0, 1.0), out.normal);

    out.emission = mix(vec3f(1.0, 1.0, 1.0), vec3f(1.0, 0.0, 0.0), t);
    out.albedo = out.emission;

    return out;
}