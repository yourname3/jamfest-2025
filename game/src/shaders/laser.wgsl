fn pbr_fn(in: VertexOutput) -> PBROut {
    var out: PBROut = pbr_basic(in);

    var t = dot(vec3f(0.0, 0.0, 1.0), out.normal);
    t = t * t;
    t = t * t;
    t = t * t;
    t = 1.0 - t;

    let laser_color = model.modulate.rgb; //vec3f(1.0, 0.0, 0.0);
    let w = vec3f(1.0, 1.0, 1.0);

    let a = mix(laser_color, w, clamp(t, 0.0, 0.5) * 2.0);
    //let b = mix(laser_color, w, (clamp(t, 0.6, 1.0) - 0.5) * 2.0);
    let c = mix(a, laser_color, t);

    out.emission = mix(laser_color * 4.0 + vec3(0.7), laser_color, t);
    out.albedo = vec3(0.0); //out.emission;

    // out.emission *= 0.5;

    return out;
}