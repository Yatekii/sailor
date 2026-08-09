fn wind_color(kn: f32) -> vec3<f32> {
    let t = clamp(kn / 50.0, 0.0, 1.0);
    let c0 = vec3<f32>(0.13, 0.20, 0.55);
    let c1 = vec3<f32>(0.13, 0.60, 0.75);
    let c2 = vec3<f32>(0.25, 0.70, 0.30);
    let c3 = vec3<f32>(0.85, 0.80, 0.20);
    let c4 = vec3<f32>(0.90, 0.45, 0.15);
    let c5 = vec3<f32>(0.75, 0.15, 0.35);
    let s = t * 5.0;
    if (s < 1.0) { return mix(c0, c1, s); }
    if (s < 2.0) { return mix(c1, c2, s - 1.0); }
    if (s < 3.0) { return mix(c2, c3, s - 2.0); }
    if (s < 4.0) { return mix(c3, c4, s - 3.0); }
    return mix(c4, c5, s - 4.0);
}
