fn pbr_fn(in: VertexOutput) -> PBROut {
    var out: PBROut = pbr_basic(in);

    out.emission = vec3(1.0, 0.0, 0.0);
    out.albedo = out.emission;

    return out;
}