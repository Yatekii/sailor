use osm::drawing::as_byte_slice;
use osm::math::Camera;
use osm::wind::grid::WindGrid;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::*;

/// Mercator wind texture resolution (u/v per texel).
const TEX_W: u32 = 1024;
const TEX_H: u32 = 1024;

/// Number of advected particles.
const PARTICLES: u32 = 6000;

/// One particle: current + previous world position, age, and a per-particle seed.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct Particle {
    pos: [f32; 2],
    prev: [f32; 2],
    age: f32,
    seed: f32,
    _pad: [f32; 2],
}

const ADVECT_WGSL: &str = r#"
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
"#;

const DRAW_WGSL: &str = r#"
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
    out.speed = length(p.pos - p.prev);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let t = clamp(in.speed * 4000.0, 0.0, 1.0);
    return vec4<f32>(t, 0.15, 1.0 - t, 0.85);
}
"#;

// fullscreen triangle vertex shader shared by fade and composite passes.
const FULLSCREEN_VS: &str = r#"
struct VsOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    // fullscreen triangle
    let p = array<vec2<f32>, 3>(vec2(-1.0,-1.0), vec2(3.0,-1.0), vec2(-1.0,3.0));
    let uv = array<vec2<f32>, 3>(vec2(0.0,1.0), vec2(2.0,1.0), vec2(0.0,-1.0));
    var o: VsOut; o.pos = vec4<f32>(p[vi], 0.0, 1.0); o.uv = uv[vi]; return o;
}
"#;

// fade a trails texture by 0.96 each frame to create fading tails.
const FADE_FS: &str = r#"
@group(0) @binding(0) var t: texture_2d<f32>;
@group(0) @binding(1) var s: sampler;
@fragment
fn fs_main(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    return textureSample(t, s, uv) * 0.96;
}
"#;

// composite the trails texture (with its alpha) over the map surface.
const COMPOSITE_FS: &str = r#"
@group(0) @binding(0) var t: texture_2d<f32>;
@group(0) @binding(1) var s: sampler;
@fragment
fn fs_main(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    return textureSample(t, s, uv);
}
"#;

/// GPU resources for the particle flow.
pub struct ParticleSystem {
    wind_tex: Texture,
    wind_view: TextureView,
    sampler: Sampler,
    particles: Buffer,
    pub count: u32,
    advect_pipeline: ComputePipeline,
    advect_bgl: BindGroupLayout,
    cu: Buffer,
    /// segment-draw pipeline targeting Rgba8Unorm trails texture.
    draw_pipeline: RenderPipeline,
    draw_bgl: BindGroupLayout,
    du: Buffer,
    draw_bg: Option<BindGroup>,
    frame: f32,
    /// two ping-pong Rgba8Unorm textures for accumulating fading trails.
    trails: [Texture; 2],
    trails_views: [TextureView; 2],
    trails_size: (u32, u32),
    /// which trails index is the current "src" (composite reads from it after swap).
    src: usize,
    /// bilinear sampler for the fullscreen fade/composite passes.
    linear_sampler: Sampler,
    fade_pipeline: RenderPipeline,
    fade_bgl: BindGroupLayout,
    composite_pipeline: RenderPipeline,
    composite_bgl: BindGroupLayout,
    /// pre-built fade bind groups: fade_bgs[src] samples trails[src] into trails[1-src].
    fade_bgs: [Option<BindGroup>; 2],
    /// pre-built composite bind group: samples trails[src] (set after swap).
    composite_bg: Option<BindGroup>,
}

impl ParticleSystem {
    pub fn new(device: &Device) -> Self {
        let wind_tex = device.create_texture(&TextureDescriptor {
            label: Some("wind uv texture"),
            size: Extent3d {
                width: TEX_W,
                height: TEX_H,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rg32Float,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let wind_view = wind_tex.create_view(&TextureViewDescriptor::default());
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("wind sampler"),
            address_mode_u: AddressMode::Repeat,
            address_mode_v: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Nearest,
            min_filter: FilterMode::Nearest,
            ..Default::default()
        });

        // seed particles at hashed-random world positions so they start spread out.
        let mut data = Vec::with_capacity(PARTICLES as usize);
        for i in 0..PARTICLES {
            let x = hash01(i * 2 + 1);
            let y = hash01(i * 2 + 2);
            data.push(Particle {
                pos: [x, y],
                prev: [x, y],
                age: hash01(i * 2 + 3) * 100.0,
                seed: i as f32,
                _pad: [0.0, 0.0],
            });
        }
        let particles = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("particles"),
            contents: as_byte_slice(&data),
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        });

        // advect bind group layout: particles (rw), wind texture, sampler, uniform
        let advect_bgl = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("advect bgl"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: false },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Sampler(SamplerBindingType::NonFiltering),
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let advect_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("advect wgsl"),
            source: ShaderSource::Wgsl(ADVECT_WGSL.into()),
        });

        let advect_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("advect pipeline layout"),
            bind_group_layouts: &[Some(&advect_bgl)],
            immediate_size: 0,
        });

        let advect_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("advect pipeline"),
            layout: Some(&advect_layout),
            module: &advect_shader,
            entry_point: Some("main"),
            compilation_options: PipelineCompilationOptions::default(),
            cache: None,
        });

        let cu = device.create_buffer(&BufferDescriptor {
            label: Some("advect uniform"),
            size: 32,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // draw bind group layout: particles (readonly), uniform
        let draw_bgl = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("draw bgl"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::VERTEX,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let draw_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("draw wgsl"),
            source: ShaderSource::Wgsl(DRAW_WGSL.into()),
        });

        let draw_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("draw pipeline layout"),
            bind_group_layouts: &[Some(&draw_bgl)],
            immediate_size: 0,
        });

        // draw pipeline targets Rgba8Unorm — it writes into the trails texture, not the surface.
        let draw_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("draw particles pipeline"),
            layout: Some(&draw_layout),
            vertex: VertexState {
                module: &draw_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: PipelineCompilationOptions::default(),
            },
            fragment: Some(FragmentState {
                module: &draw_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(ColorTargetState {
                    format: TextureFormat::Rgba8Unorm,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: PipelineCompilationOptions::default(),
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::LineList,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        let du = device.create_buffer(&BufferDescriptor {
            label: Some("draw uniform"),
            size: 32,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // fullscreen texture bind group layout: texture + sampler (shared by fade and composite).
        let tex_bgl = |device: &Device, label: &'static str| {
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some(label),
                entries: &[
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Texture {
                            sample_type: TextureSampleType::Float { filterable: true },
                            view_dimension: TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Sampler(SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            })
        };

        let fade_bgl = tex_bgl(device, "fade bgl");
        let composite_bgl = tex_bgl(device, "composite bgl");

        let linear_sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("trails linear sampler"),
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..Default::default()
        });

        let fullscreen_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("fullscreen vs"),
            source: ShaderSource::Wgsl(FULLSCREEN_VS.into()),
        });

        let fade_fs_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("fade fs"),
            source: ShaderSource::Wgsl(FADE_FS.into()),
        });

        let fade_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("fade pipeline layout"),
            bind_group_layouts: &[Some(&fade_bgl)],
            immediate_size: 0,
        });

        // fade pipeline: samples src trails, writes faded result into dst (REPLACE blend — every texel is overwritten).
        let fade_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("fade pipeline"),
            layout: Some(&fade_layout),
            vertex: VertexState {
                module: &fullscreen_module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: PipelineCompilationOptions::default(),
            },
            fragment: Some(FragmentState {
                module: &fade_fs_module,
                entry_point: Some("fs_main"),
                targets: &[Some(ColorTargetState {
                    format: TextureFormat::Rgba8Unorm,
                    blend: Some(BlendState::REPLACE),
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: PipelineCompilationOptions::default(),
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        let composite_fs_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("composite fs"),
            source: ShaderSource::Wgsl(COMPOSITE_FS.into()),
        });

        let composite_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("composite pipeline layout"),
            bind_group_layouts: &[Some(&composite_bgl)],
            immediate_size: 0,
        });

        // composite pipeline: blends the trails texture over the surface (Bgra8Unorm).
        let composite_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("composite pipeline"),
            layout: Some(&composite_layout),
            vertex: VertexState {
                module: &fullscreen_module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: PipelineCompilationOptions::default(),
            },
            fragment: Some(FragmentState {
                module: &composite_fs_module,
                entry_point: Some("fs_main"),
                targets: &[Some(ColorTargetState {
                    format: TextureFormat::Bgra8Unorm,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: PipelineCompilationOptions::default(),
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        // start with a 1x1 placeholder; recreated on first render_trails call.
        let (trails, trails_views) = make_trails(device, 1, 1);

        Self {
            wind_tex,
            wind_view,
            sampler,
            particles,
            count: PARTICLES,
            advect_pipeline,
            advect_bgl,
            cu,
            draw_pipeline,
            draw_bgl,
            du,
            draw_bg: None,
            frame: 0.0,
            trails,
            trails_views,
            trails_size: (1, 1),
            src: 0,
            linear_sampler,
            fade_pipeline,
            fade_bgl,
            composite_pipeline,
            composite_bgl,
            fade_bgs: [None, None],
            composite_bg: None,
        }
    }

    /// Resample the grid into the mercator wind texture.
    pub fn upload_wind(&self, queue: &Queue, grid: &WindGrid) {
        let field = grid.resample_mercator(TEX_W as usize, TEX_H as usize);
        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &self.wind_tex,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            as_byte_slice(&field),
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(TEX_W * 8), // rg32float = 8 bytes/texel
                rows_per_image: Some(TEX_H),
            },
            Extent3d {
                width: TEX_W,
                height: TEX_H,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Run compute advect + fade + segment-draw into the trails texture, then swap src/dst.
    pub fn render_trails(
        &mut self,
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        camera: &Camera,
        resolution: (u32, u32),
    ) {
        self.frame += 1.0;

        // recreate trails textures if the resolution changed.
        if self.trails_size != resolution {
            let (trails, views) = make_trails(device, resolution.0, resolution.1);
            self.trails = trails;
            self.trails_views = views;
            self.trails_size = resolution;
            // invalidate cached bind groups so they're rebuilt below.
            self.fade_bgs = [None, None];
            self.composite_bg = None;
        }

        // --- compute advect pass ---
        // speed tuned so particles drift a fraction of the viewport per second,
        // not zip across it; the /2^zoom in the shader cancels the draw scale, so
        // on-screen speed is roughly zoom-independent. vmin/vmax are the viewport
        // world bounds, so respawned particles stay on screen.
        let s = 2f32.powf(camera.zoom) * camera.tile_size();
        let hx = (camera.width / 2.0) / s;
        let hy = (camera.height / 2.0) / s;
        let cx = camera.center.x as f32;
        let cy = camera.center.y as f32;
        let cu = [
            1.0f32,
            0.0004,
            camera.zoom,
            self.frame,
            cx - hx,
            cy - hy,
            cx + hx,
            cy + hy,
        ];
        queue.write_buffer(&self.cu, 0, as_byte_slice(&cu));

        let advect_bg = device.create_bind_group(&BindGroupDescriptor {
            label: Some("advect bg"),
            layout: &self.advect_bgl,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: self.particles.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(&self.wind_view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::Sampler(&self.sampler),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: self.cu.as_entire_binding(),
                },
            ],
        });
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("advect"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.advect_pipeline);
            pass.set_bind_group(0, &advect_bg, &[]);
            pass.dispatch_workgroups(self.count.div_ceil(64), 1, 1);
        }

        let src = self.src;
        let dst = 1 - src;

        // build fade bind group if missing: samples src into dst.
        if self.fade_bgs[src].is_none() {
            self.fade_bgs[src] = Some(device.create_bind_group(&BindGroupDescriptor {
                label: Some("fade bg"),
                layout: &self.fade_bgl,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::TextureView(&self.trails_views[src]),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::Sampler(&self.linear_sampler),
                    },
                ],
            }));
        }

        // --- fade pass: src -> dst with REPLACE blend; clear dst first (fade writes every texel). ---
        {
            let fade_bg = self.fade_bgs[src].as_ref().unwrap();
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("trails fade"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    depth_slice: None,
                    view: &self.trails_views[dst],
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color::TRANSPARENT),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.fade_pipeline);
            pass.set_bind_group(0, fade_bg, &[]);
            pass.draw(0..3, 0..1);
        }

        // draw uniform: center (f32x2), scale (world->clip)
        let s = 2f32.powf(camera.zoom) * camera.tile_size();
        let du = [
            camera.center.x as f32,
            camera.center.y as f32,
            s / (camera.width / 2.0),
            s / (camera.height / 2.0),
            0.0f32,
            0.0,
            0.0,
            0.0,
        ];
        queue.write_buffer(&self.du, 0, as_byte_slice(&du));

        // build draw bind group (stores computed du).
        self.draw_bg = Some(device.create_bind_group(&BindGroupDescriptor {
            label: Some("draw bg"),
            layout: &self.draw_bgl,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: self.particles.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: self.du.as_entire_binding(),
                },
            ],
        }));

        // --- segment pass: draw particles into dst with LoadOp::Load ---
        {
            let draw_bg = self.draw_bg.as_ref().unwrap();
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("trails segments"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    depth_slice: None,
                    view: &self.trails_views[dst],
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Load,
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.draw_pipeline);
            pass.set_bind_group(0, draw_bg, &[]);
            pass.draw(0..2, 0..self.count);
        }

        // swap: dst becomes the new src for composite and the next frame.
        self.src = dst;

        // rebuild composite bind group to read from the new src.
        self.composite_bg = Some(device.create_bind_group(&BindGroupDescriptor {
            label: Some("composite bg"),
            layout: &self.composite_bgl,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&self.trails_views[self.src]),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(&self.linear_sampler),
                },
            ],
        }));
        // the fade bind group for the old dst (now src) is stale — clear it so it's rebuilt next frame.
        self.fade_bgs[self.src] = None;
    }

    /// Composite the trails texture over an active render pass (surface format Bgra8Unorm).
    pub fn composite<'a>(&'a self, pass: &mut RenderPass<'a>) {
        let Some(bg) = &self.composite_bg else { return };
        pass.set_pipeline(&self.composite_pipeline);
        pass.set_bind_group(0, bg, &[]);
        pass.draw(0..3, 0..1);
    }
}

/// Create two Rgba8Unorm trails textures and their views.
fn make_trails(device: &Device, w: u32, h: u32) -> ([Texture; 2], [TextureView; 2]) {
    let make = |label: &'static str| {
        device.create_texture(&TextureDescriptor {
            label: Some(label),
            size: Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
    };
    let t0 = make("trails 0");
    let t1 = make("trails 1");
    let v0 = t0.create_view(&TextureViewDescriptor::default());
    let v1 = t1.create_view(&TextureViewDescriptor::default());
    ([t0, t1], [v0, v1])
}

/// Deterministic 0..1 hash of an integer (for seeding — no rng dependency).
fn hash01(n: u32) -> f32 {
    let mut x = n.wrapping_mul(747796405).wrapping_add(2891336453);
    x = (x >> ((x >> 28).wrapping_add(4))) ^ x;
    x = x.wrapping_mul(277803737);
    x = (x >> 22) ^ x;
    (x & 0xffffff) as f32 / 0xffffff as f32
}
