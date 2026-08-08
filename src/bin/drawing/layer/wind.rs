use crate::config::CONFIG;
use osm::drawing::as_byte_slice;
use osm::math::{deg2num, num2deg, Camera, Coord, Geo, Pixel, TileCoordinate};
use osm::wind::cache::{Bbox, WindCache};
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::*;

use super::{FramePass, Layer, LayerCtx};

/// One arrow instance: world-space anchor (mercator [0,1]) and wind vector (kn).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct Instance {
    world_x: f32,
    world_y: f32,
    u: f32,
    v: f32,
}

/// Camera uniform: world->clip matrix (mat4) + viewport size in pixels.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct Uniforms {
    world_to_clip: [f32; 16],
    viewport: [f32; 2],
    _pad: [f32; 2],
}

const WGSL: &str = r#"
struct Uniforms {
    world_to_clip: mat4x4<f32>,
    viewport: vec2<f32>,
    _pad: vec2<f32>,
};
@group(0) @binding(0) var<uniform> u: Uniforms;

struct VsIn {
    @location(0) template: vec2<f32>,   // unit arrow vertex, x along shaft
    @location(1) world: vec2<f32>,
    @location(2) wind: vec2<f32>,       // u (east), v (north) in knots
};

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) speed: f32,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    let speed = length(in.wind);
    // Screen direction: north is -y in clip space, so flip v.
    let dir = normalize(vec2<f32>(in.wind.x, -in.wind.y) + vec2<f32>(1e-6, 0.0));
    // Constant on-screen arrow length in pixels, growing a little with speed.
    let px = 14.0 + min(speed, 40.0) * 0.6;
    let rot = mat2x2<f32>(dir.x, dir.y, -dir.y, dir.x);
    let offset_px = rot * (in.template * px);
    // Anchor in clip space, then add the pixel offset converted to clip units.
    var anchor = u.world_to_clip * vec4<f32>(in.world, 0.0, 1.0);
    let ndc_off = offset_px / (u.viewport * 0.5);
    var out: VsOut;
    out.pos = vec4<f32>(anchor.xy + ndc_off, 0.0, 1.0);
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
    instances: Option<Buffer>,
    instance_count: u32,
    cache: WindCache,
    /// snapped region the current instance buffer was built for
    built_bbox: Option<Bbox>,
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

        Self {
            pipeline,
            bind_group_layout,
            bind_group: None,
            template,
            uniform,
            instances: None,
            instance_count: 0,
            cache: WindCache::new(CONFIG.general.data_root.clone()),
            built_bbox: None,
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
        let bbox = Self::viewport_bbox(ctx.screen);
        self.cache.request(bbox);

        // Rebuild the instance buffer only when the loaded field region changes,
        // not every pan frame — the field only changes when we cross a grid line.
        let field_bbox = self.cache.loaded_bbox();
        if field_bbox != self.built_bbox {
            if let Some(field) = self.cache.field() {
                let instances: Vec<Instance> = field
                    .samples
                    .iter()
                    .map(|s| {
                        // world space is the z=0 tile coordinate (mercator [0,1]).
                        let t = deg2num(Coord::<Geo>::new(s.lon, s.lat), 0);
                        Instance { world_x: t.x, world_y: t.y, u: s.u, v: s.v }
                    })
                    .collect();
                self.instance_count = instances.len() as u32;
                self.instances = Some(ctx.device.create_buffer_init(&BufferInitDescriptor {
                    label: Some("wind instances"),
                    contents: as_byte_slice(&instances),
                    usage: BufferUsages::VERTEX,
                }));
                self.built_bbox = field_bbox;
            }
        }

        // Upload the camera uniform every frame (world->clip changes on pan/zoom).
        let m = ctx.screen.world_to_gpu();
        let mut world_to_clip = [0.0f32; 16];
        world_to_clip.copy_from_slice(m.matrix().as_slice());
        let uniforms = Uniforms {
            world_to_clip,
            viewport: [ctx.screen.width, ctx.screen.height],
            _pad: [0.0, 0.0],
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
        let Some(instances) = &self.instances else { return };
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
        pass.set_vertex_buffer(1, instances.slice(..));
        pass.draw(0..ARROW.len() as u32, 0..self.instance_count);
    }
}
