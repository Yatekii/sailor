struct LU {
    viewport: vec2<f32>,
    panel_pos: vec2<f32>,
    panel_size: vec2<f32>,
    bar_pos: vec2<f32>,
    bar_size: vec2<f32>,
    pad: vec2<f32>,
};
@group(0) @binding(0) var<uniform> u: LU;

struct VsOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    // triangle-strip quad covering the whole (rounded) panel.
    var corners = array<vec2<f32>, 4>(vec2(0.0, 0.0), vec2(0.0, 1.0), vec2(1.0, 0.0), vec2(1.0, 1.0));
    let c = corners[vi];
    let px = u.panel_pos + c * u.panel_size;
    let ndc = vec2<f32>(px.x / u.viewport.x * 2.0 - 1.0, 1.0 - px.y / u.viewport.y * 2.0);
    var out: VsOut;
    out.pos = vec4<f32>(ndc, 0.0, 1.0);
    out.uv = c;
    return out;
}

// signed distance to a rounded rect centred at origin.
fn rrect(p: vec2<f32>, half: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - half + vec2<f32>(r, r);
    return length(max(q, vec2<f32>(0.0, 0.0))) + min(max(q.x, q.y), 0.0) - r;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let px = u.panel_pos + in.uv * u.panel_size;
    // rounded-corner mask over the frosted card.
    let d = rrect(px - (u.panel_pos + u.panel_size * 0.5), u.panel_size * 0.5, 10.0);
    let card = 1.0 - smoothstep(-1.0, 1.0, d);
    // the gradient bar sits inside the card.
    let inbar = px.x >= u.bar_pos.x && px.x <= u.bar_pos.x + u.bar_size.x
        && px.y >= u.bar_pos.y && px.y <= u.bar_pos.y + u.bar_size.y;
    if (inbar) {
        let t = (px.x - u.bar_pos.x) / u.bar_size.x;
        return vec4<f32>(wind_color(t * 50.0), 0.95 * card);
    }
    // frosted light backing.
    return vec4<f32>(0.96, 0.96, 0.97, 0.7 * card);
}
