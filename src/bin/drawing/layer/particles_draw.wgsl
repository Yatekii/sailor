struct Particle { pos: vec2<f32>, prev: vec2<f32>, age: f32, seed: f32, pad: vec2<f32> };
struct DU { center: vec2<f32>, scale: vec2<f32> };
@group(0) @binding(0) var<storage, read> parts: array<Particle>;
@group(0) @binding(1) var<uniform> u: DU;

struct VsOut { @builtin(position) pos: vec4<f32>, @location(0) speed: f32 };

fn to_clip(world: vec2<f32>, center: vec2<f32>, scale: vec2<f32>) -> vec2<f32> {
    var c = (world - center) * scale;
    c.y = -c.y; // match the basemap y-flip
    return c;
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VsOut {
    let p = parts[ii];
    let world = select(p.prev, p.pos, vi == 1u);
    var out: VsOut;
    out.pos = vec4<f32>(to_clip(world, u.center, u.scale), 0.0, 1.0);
    out.speed = p.pad.x; // wind speed in knots, stashed by the compute pass
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return vec4<f32>(wind_color(in.speed), 0.85);
}
