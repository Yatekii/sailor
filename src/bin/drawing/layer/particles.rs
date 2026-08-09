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
struct CU { dt: f32, speed: f32, zoom: f32, frame: f32, pad: vec4<f32> };
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

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= arrayLength(&parts)) { return; }
    var p = parts[i];
    let uv = vec2<f32>(fract(p.pos.x), p.pos.y);
    let w = textureSampleLevel(wind, samp, uv, 0.0).xy; // knots (u east, v north)
    // step in world units, scaled so on-screen speed is roughly zoom-independent.
    let step = w * u.speed / pow(2.0, u.zoom);
    p.prev = p.pos;
    // world y is +south; north wind (+v) should move the particle toward -y.
    p.pos = vec2<f32>(p.pos.x + step.x, p.pos.y - step.y);
    p.age = p.age + u.dt;
    let dead = p.age > 60.0 || p.pos.y < 0.0 || p.pos.y > 1.0 || length(step) < 1e-7;
    if (dead) {
        let nx = hash01(i * 3u + u32(u.frame) * 2654435761u);
        let ny = hash01(i * 5u + u32(u.frame) * 40503u + 7u);
        p.pos = vec2<f32>(nx, ny);
        p.prev = p.pos;
        p.age = 0.0;
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
    draw_pipeline: RenderPipeline,
    draw_bgl: BindGroupLayout,
    du: Buffer,
    draw_bg: Option<BindGroup>,
    frame: f32,
}

impl ParticleSystem {
    pub fn new(device: &Device) -> Self {
        let wind_tex = device.create_texture(&TextureDescriptor {
            label: Some("wind uv texture"),
            size: Extent3d { width: TEX_W, height: TEX_H, depth_or_array_layers: 1 },
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
            usage: BufferUsages::STORAGE | BufferUsages::VERTEX | BufferUsages::COPY_DST,
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
                    format: TextureFormat::Bgra8Unorm,
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
            Extent3d { width: TEX_W, height: TEX_H, depth_or_array_layers: 1 },
        );
    }

    /// Advance particles one step and rebuild the draw bind group.
    pub fn advance(
        &mut self,
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        camera: &Camera,
    ) {
        self.frame += 1.0;
        // compute uniform: dt, speed, zoom, frame (+pad to 32 bytes)
        let cu = [1.0f32, 0.02, camera.zoom, self.frame, 0.0, 0.0, 0.0, 0.0];
        queue.write_buffer(&self.cu, 0, as_byte_slice(&cu));

        let bg = device.create_bind_group(&BindGroupDescriptor {
            label: Some("advect bg"),
            layout: &self.advect_bgl,
            entries: &[
                BindGroupEntry { binding: 0, resource: self.particles.as_entire_binding() },
                BindGroupEntry { binding: 1, resource: BindingResource::TextureView(&self.wind_view) },
                BindGroupEntry { binding: 2, resource: BindingResource::Sampler(&self.sampler) },
                BindGroupEntry { binding: 3, resource: self.cu.as_entire_binding() },
            ],
        });
        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("advect"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.advect_pipeline);
        pass.set_bind_group(0, &bg, &[]);
        pass.dispatch_workgroups(self.count.div_ceil(64), 1, 1);
        drop(pass);

        // draw uniform: center (f32), scale (world->clip via scale/(dim/2))
        let s = 2f32.powf(camera.zoom) * camera.tile_size();
        let du = [
            camera.center.x as f32,
            camera.center.y as f32,
            s / (camera.width / 2.0),
            s / (camera.height / 2.0),
            0.0,
            0.0,
            0.0,
            0.0,
        ];
        queue.write_buffer(&self.du, 0, as_byte_slice(&du));

        // build the draw bind group here (has `device`) and store it — a bind
        // group used in a render pass can't be created inside the pass borrow.
        self.draw_bg = Some(device.create_bind_group(&BindGroupDescriptor {
            label: Some("draw bg"),
            layout: &self.draw_bgl,
            entries: &[
                BindGroupEntry { binding: 0, resource: self.particles.as_entire_binding() },
                BindGroupEntry { binding: 1, resource: self.du.as_entire_binding() },
            ],
        }));
    }

    /// Draw particle segments into an active render pass.
    pub fn draw<'a>(&'a self, pass: &mut RenderPass<'a>) {
        let Some(bg) = &self.draw_bg else { return };
        pass.set_pipeline(&self.draw_pipeline);
        pass.set_bind_group(0, bg, &[]);
        pass.draw(0..2, 0..self.count);
    }
}

/// Deterministic 0..1 hash of an integer (for seeding — no rng dependency).
fn hash01(n: u32) -> f32 {
    let mut x = n.wrapping_mul(747796405).wrapping_add(2891336453);
    x = (x >> ((x >> 28).wrapping_add(4))) ^ x;
    x = x.wrapping_mul(277803737);
    x = (x >> 22) ^ x;
    (x & 0xffffff) as f32 / 0xffffff as f32
}
