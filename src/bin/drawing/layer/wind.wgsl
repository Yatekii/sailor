struct Uniforms {
    scale: vec2<f32>,
    viewport: vec2<f32>,
};
@group(0) @binding(0) var<uniform> u: Uniforms;

struct VsIn {
    @location(0) corner: vec2<f32>,   // unit arrow vertex, x along shaft
    @location(1) rel: vec2<f32>,      // position relative to camera centre
    @location(2) wind: vec2<f32>,     // u (east), v (north) in knots
};

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) speed: f32,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    let speed = length(in.wind);
    // The basemap flips clip-space y (shader.vert ends with gl_Position.y = -y),
    // so the final screen is +y up / north up. Point the arrow straight along the
    // wind vector (u east, v north) in that space.
    let dir = normalize(in.wind + vec2<f32>(1e-6, 0.0));
    // On-screen arrow length in pixels, growing a little with speed.
    let px = 70.0 + min(speed, 40.0) * 3.0;
    let rot = mat2x2<f32>(dir.x, dir.y, -dir.y, dir.x);
    let offset_px = rot * (in.corner * px);
    // Only the scale is applied here; the centre offset was already subtracted in
    // f64 on the cpu, so there is no large-number cancellation at high zoom. Then
    // match the basemap's y-flip so positions and pan track the map.
    var anchor = in.rel * u.scale;
    anchor.y = -anchor.y;
    let ndc_off = offset_px / (u.viewport * 0.5);
    var out: VsOut;
    out.pos = vec4<f32>(anchor + ndc_off, 0.0, 1.0);
    out.speed = speed;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // in.speed is the wind magnitude in knots; colour it with the shared palette.
    return vec4<f32>(wind_color(in.speed), 0.9);
}
