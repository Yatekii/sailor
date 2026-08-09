struct Particle { pos: vec2<f32>, prev: vec2<f32>, age: f32, seed: f32, pad: vec2<f32> };
// vmin/vmax are the viewport's world-space bounds; particles respawn inside them
// so on-screen density holds instead of scattering across the whole globe.
struct CU { dt: f32, speed: f32, zoom: f32, frame: f32, vmin: vec2<f32>, vmax: vec2<f32> };
@group(0) @binding(0) var<storage, read_write> parts: array<Particle>;
@group(0) @binding(1) var wind: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;
@group(0) @binding(3) var<uniform> u: CU;

fn hash01(n: u32) -> f32 {
    var x = n * 747796405u + 2891336453u;
    x = ((x >> ((x >> 28u) + 4u)) ^ x) * 277803737u;
    x = (x >> 22u) ^ x;
    return f32(x & 0xffffffu) / f32(0xffffffu);
}

// Manual bilinear sample of the wind texture (rg32float isn't hardware-filterable,
// and nearest sampling gives angular, kinked flow). Longitude wraps, latitude clamps.
fn sample_wind(uv: vec2<f32>) -> vec2<f32> {
    let dim = vec2<f32>(textureDimensions(wind));
    let p = uv * dim - vec2<f32>(0.5, 0.5);
    let base = floor(p);
    let f = p - base;
    let w = i32(dim.x);
    let h = i32(dim.y);
    let x0 = ((i32(base.x) % w) + w) % w;
    let x1 = ((i32(base.x) + 1) % w + w) % w;
    let y0 = clamp(i32(base.y), 0, h - 1);
    let y1 = clamp(i32(base.y) + 1, 0, h - 1);
    let c00 = textureLoad(wind, vec2<i32>(x0, y0), 0).xy;
    let c10 = textureLoad(wind, vec2<i32>(x1, y0), 0).xy;
    let c01 = textureLoad(wind, vec2<i32>(x0, y1), 0).xy;
    let c11 = textureLoad(wind, vec2<i32>(x1, y1), 0).xy;
    return mix(mix(c00, c10, f.x), mix(c01, c11, f.x), f.y);
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= arrayLength(&parts)) { return; }
    var p = parts[i];
    let uv = vec2<f32>(fract(p.pos.x), p.pos.y);
    let w = sample_wind(uv); // knots (u east, v north), bilinear
    p.pad.x = length(w); // carry the wind speed (kn) to the draw pass for colouring
    // step in world units, scaled so on-screen speed is roughly zoom-independent.
    let step = w * u.speed / pow(2.0, u.zoom);
    p.prev = p.pos;
    // world y is +south; north wind (+v) should move the particle toward -y.
    p.pos = vec2<f32>(p.pos.x + step.x, p.pos.y - step.y);
    p.age = p.age + u.dt;
    let out = p.pos.x < u.vmin.x || p.pos.x > u.vmax.x || p.pos.y < u.vmin.y || p.pos.y > u.vmax.y;
    let dead = p.age > 60.0 || out || length(step) < 1e-8;
    if (dead) {
        let nx = hash01(i * 3u + u32(u.frame) * 2654435761u);
        let ny = hash01(i * 5u + u32(u.frame) * 40503u + 7u);
        // respawn inside the viewport, ages staggered so they don't all blink together.
        p.pos = u.vmin + vec2<f32>(nx, ny) * (u.vmax - u.vmin);
        p.prev = p.pos;
        p.age = hash01(i * 7u + u32(u.frame) + 11u) * 60.0;
    }
    parts[i] = p;
}
