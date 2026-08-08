use std::f64::consts::{FRAC_PI_4, PI};

use crate::config::CONFIG;
use osm::drawing::as_byte_slice;
use osm::math::{num2deg, Camera, Coord, Geo, Pixel, TileCoordinate};
use osm::wind::cache::{Bbox, WindCache};
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::*;

use super::{FramePass, Layer, LayerCtx};

/// Max arrows drawn in a frame; the lattice fetch is capped well below this.
const MAX_INSTANCES: usize = 512;

/// One arrow instance: position relative to the camera centre (mercator units)
/// and the wind vector (u east, v north, knots). The offset is kept relative
/// and subtracted in f64 on the cpu so it doesn't cancel in f32 at high zoom.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct Instance {
    rel_x: f32,
    rel_y: f32,
    u: f32,
    v: f32,
}

/// Camera uniform: mercator->clip scale (x, y) + viewport size in pixels.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct Uniforms {
    scale: [f32; 2],
    viewport: [f32; 2],
}

const WGSL: &str = r#"
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
    // Blue (calm) -> red (strong), saturating around 35 kn.
    let t = clamp(in.speed / 35.0, 0.0, 1.0);
    return vec4<f32>(t, 0.15, 1.0 - t, 0.9);
}
"#;

// Unit arrow as a triangle list in local space: a thin shaft quad + a head.
// x runs 0..1 along the wind direction; y is the across-shaft axis.
const ARROW: &[[f32; 2]] = &[
    // shaft quad
    [0.0, -0.06], [0.7, -0.06], [0.7, 0.06],
    [0.0, -0.06], [0.7, 0.06], [0.0, 0.06],
    // head triangle
    [0.6, -0.18], [1.0, 0.0], [0.6, 0.18],
];

pub struct WindLayer {
    pipeline: RenderPipeline,
    bind_group_layout: BindGroupLayout,
    bind_group: Option<BindGroup>,
    template: Buffer,
    uniform: Buffer,
    instances: Buffer,
    instance_count: u32,
    cache: WindCache,
    visible: bool,
}

impl WindLayer {
    pub fn new(device: &Device) -> Self {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("wind wgsl"),
            source: ShaderSource::Wgsl(WGSL.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("wind uniforms"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("wind pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("wind pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[
                    Some(VertexBufferLayout {
                        array_stride: 8,
                        step_mode: VertexStepMode::Vertex,
                        attributes: &[VertexAttribute {
                            format: VertexFormat::Float32x2,
                            offset: 0,
                            shader_location: 0,
                        }],
                    }),
                    Some(VertexBufferLayout {
                        array_stride: std::mem::size_of::<Instance>() as u64,
                        step_mode: VertexStepMode::Instance,
                        attributes: &[
                            VertexAttribute {
                                format: VertexFormat::Float32x2,
                                offset: 0,
                                shader_location: 1,
                            },
                            VertexAttribute {
                                format: VertexFormat::Float32x2,
                                offset: 8,
                                shader_location: 2,
                            },
                        ],
                    }),
                ],
                compilation_options: PipelineCompilationOptions::default(),
            },
            fragment: Some(FragmentState {
                module: &shader,
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
            // single-sampled: rendered directly onto frame.view like hover, so
            // we don't re-resolve the msaa buffer and clobber other overlays
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        let template = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("wind arrow template"),
            contents: as_byte_slice(ARROW),
            usage: BufferUsages::VERTEX,
        });

        let uniform = device.create_buffer(&BufferDescriptor {
            label: Some("wind uniform"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let instances = device.create_buffer(&BufferDescriptor {
            label: Some("wind instances"),
            size: (MAX_INSTANCES * std::mem::size_of::<Instance>()) as u64,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            bind_group_layout,
            bind_group: None,
            template,
            uniform,
            instances,
            instance_count: 0,
            cache: WindCache::new(CONFIG.general.data_root.clone()),
            visible: true,
        }
    }

    /// Viewport corners -> lon/lat bbox. World space is the z=0 tile coordinate
    /// (mercator [0,1]), so `num2deg` at z=0 turns a world point into lon/lat.
    fn viewport_bbox(camera: &Camera) -> Bbox {
        let p2w = camera.pixel_to_world();
        let corner = |px: f32, py: f32| -> Coord<Geo> {
            let w = p2w.apply(Coord::<Pixel>::new(px, py));
            num2deg(TileCoordinate::new(0, w.x(), w.y()))
        };
        let a = corner(0.0, 0.0);
        let b = corner(camera.width, camera.height);
        Bbox {
            min_lon: a.x().min(b.x()),
            min_lat: a.y().min(b.y()),
            max_lon: a.x().max(b.x()),
            max_lat: a.y().max(b.y()),
        }
    }
}

impl Layer for WindLayer {
    fn name(&self) -> &str {
        "wind"
    }

    fn visible(&self) -> bool {
        self.visible && self.instance_count > 0
    }

    fn update(&mut self, ctx: &mut LayerCtx) {
        let cam = ctx.screen;
        self.cache.request(Self::viewport_bbox(cam));

        // Rebuild instances each frame: position is relative to the camera centre,
        // which moves on every pan/zoom. Subtract the centre in f64 (and project
        // lat/lon in f64) so a small offset doesn't vanish in f32 at high zoom.
        let center = cam.center;
        let instances: Vec<Instance> = self
            .cache
            .field()
            .map(|field| {
                field
                    .samples
                    .iter()
                    .take(MAX_INSTANCES)
                    .map(|s| {
                        let lon = s.lon as f64;
                        let lat = (s.lat as f64).to_radians();
                        let wx = (lon + 180.0) / 360.0;
                        let wy = (1.0 - (FRAC_PI_4 + lat / 2.0).tan().ln() / PI) / 2.0;
                        Instance {
                            rel_x: (wx - center.x) as f32,
                            rel_y: (wy - center.y) as f32,
                            u: s.u,
                            v: s.v,
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        self.instance_count = instances.len() as u32;
        if !instances.is_empty() {
            ctx.queue
                .write_buffer(&self.instances, 0, as_byte_slice(&instances));
        }

        // Mercator->clip scale (same as world_to_gpu without the centre term).
        let s = 2f32.powf(cam.zoom) * cam.tile_size();
        let uniforms = Uniforms {
            scale: [s / (cam.width / 2.0), s / (cam.height / 2.0)],
            viewport: [cam.width, cam.height],
        };
        ctx.queue.write_buffer(&self.uniform, 0, as_byte_slice(&[uniforms]));

        // FramePass carries no device, so build the bind group here, not in paint.
        self.bind_group = Some(ctx.device.create_bind_group(&BindGroupDescriptor {
            label: Some("wind bind group"),
            layout: &self.bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: self.uniform.as_entire_binding(),
            }],
        }));
    }

    fn paint(&self, frame: &mut FramePass) {
        let Some(bind_group) = &self.bind_group else { return };
        if self.instance_count == 0 {
            return;
        }

        let mut pass = frame.encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("wind arrows"),
            color_attachments: &[Some(RenderPassColorAttachment {
                depth_slice: None,
                view: frame.view,
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
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.set_vertex_buffer(0, self.template.slice(..));
        pass.set_vertex_buffer(1, self.instances.slice(..));
        pass.draw(0..ARROW.len() as u32, 0..self.instance_count);
    }
}
