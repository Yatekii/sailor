use glyphon::{Attrs, Buffer, Color, Family, Metrics, Shaping, TextArea, TextBounds, TextRenderer};
use osm::drawing::as_byte_slice;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::*;

use super::text::TextStack;
use super::{FramePass, Layer, LayerCtx};

/// Bar geometry in physical pixels (sized for hidpi/retina).
const BAR_W: f32 = 360.0;
const BAR_H: f32 = 28.0;
const MARGIN_X: f32 = 40.0;
const MARGIN_BOTTOM: f32 = 64.0;
/// TextArea scale — matches the map's retina text handling.
const LABEL_SCALE: f32 = 2.0;
/// Tick labels along the bar (knots).
const TICKS: [&str; 6] = ["0", "10", "20", "30", "40", "50 kn"];

const GRADIENT_WGSL: &str = r#"
struct LU { viewport: vec2<f32>, rect_pos: vec2<f32>, rect_size: vec2<f32>, pad: vec2<f32> };
@group(0) @binding(0) var<uniform> u: LU;

struct VsOut { @builtin(position) pos: vec4<f32>, @location(0) t: f32 };

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    // triangle-strip quad corners (x across the bar, y down it)
    var corners = array<vec2<f32>, 4>(vec2(0.0, 0.0), vec2(0.0, 1.0), vec2(1.0, 0.0), vec2(1.0, 1.0));
    let c = corners[vi];
    let px = u.rect_pos + c * u.rect_size;
    let ndc = vec2<f32>(px.x / u.viewport.x * 2.0 - 1.0, 1.0 - px.y / u.viewport.y * 2.0);
    var out: VsOut;
    out.pos = vec4<f32>(ndc, 0.0, 1.0);
    out.t = c.x;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return vec4<f32>(wind_color(in.t * 50.0), 0.92);
}
"#;

/// On-map wind-speed color legend: a gradient bar plus knots tick labels, using
/// the shared palette and the shared glyphon atlas. Shown when the wind overlay
/// is visible.
pub struct LegendLayer {
    pipeline: RenderPipeline,
    bind_group: BindGroup,
    uniform: Buffer_,
    text_renderer: TextRenderer,
    labels: Vec<Buffer>,
    rect: [f32; 4],
    visible: bool,
}

// `wgpu::Buffer` and `glyphon::Buffer` clash by name; alias the wgpu one.
type Buffer_ = wgpu::Buffer;

impl LegendLayer {
    pub fn new(device: &Device, text: &mut TextStack) -> Self {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("legend wgsl"),
            source: ShaderSource::Wgsl(format!("{}{GRADIENT_WGSL}", super::PALETTE_WGSL).into()),
        });

        let bgl = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("legend uniform"),
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
            label: Some("legend pipeline layout"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("legend pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
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
                topology: PrimitiveTopology::TriangleStrip,
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

        let uniform = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("legend uniform"),
            contents: as_byte_slice(&[0.0f32; 8]),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("legend bind group"),
            layout: &bgl,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });

        let text_renderer =
            TextRenderer::new(&mut text.atlas, device, MultisampleState::default(), None);

        // Static tick labels, shaped once against the shared font system.
        let attrs = Attrs::new().family(Family::SansSerif);
        let labels = TICKS
            .iter()
            .map(|s| {
                let mut buffer = Buffer::new(&mut text.font_system, Metrics::relative(13.0, 1.2));
                buffer.set_text(s, &attrs, Shaping::Advanced, None);
                buffer.shape_until_scroll(&mut text.font_system, false);
                buffer
            })
            .collect();

        Self {
            pipeline,
            bind_group,
            uniform,
            text_renderer,
            labels,
            rect: [0.0; 4],
            visible: false,
        }
    }
}

impl Layer for LegendLayer {
    fn name(&self) -> &str {
        "legend"
    }

    fn visible(&self) -> bool {
        self.visible
    }

    fn update(&mut self, ctx: &mut LayerCtx) {
        self.visible = ctx.wind.visible;
        if !self.visible {
            return;
        }

        let (w, h) = (ctx.resolution.0 as f32, ctx.resolution.1 as f32);
        // bottom-left corner; rect = [x, y_top, width, height] in pixels.
        self.rect = [MARGIN_X, h - MARGIN_BOTTOM - BAR_H, BAR_W, BAR_H];
        let u = [
            w,
            h,
            self.rect[0],
            self.rect[1],
            self.rect[2],
            self.rect[3],
            0.0,
            0.0,
        ];
        ctx.queue.write_buffer(&self.uniform, 0, as_byte_slice(&u));

        // Labels under the bar, one per tick.
        let top = self.rect[1] + BAR_H + 4.0;
        let areas = self.labels.iter().enumerate().map(|(i, buffer)| {
            let left = self.rect[0] + i as f32 / (TICKS.len() - 1) as f32 * BAR_W - 6.0;
            TextArea {
                buffer,
                left,
                top,
                scale: LABEL_SCALE,
                bounds: TextBounds {
                    left: left as i32,
                    top: top as i32,
                    right: left as i32 + 200,
                    bottom: top as i32 + 40,
                },
                default_color: Color::rgb(25, 25, 25),
                custom_glyphs: &[],
            }
        });

        self.text_renderer
            .prepare(
                ctx.device,
                ctx.queue,
                &mut ctx.text.font_system,
                &mut ctx.text.atlas,
                &ctx.text.viewport,
                areas,
                &mut ctx.text.swash_cache,
            )
            .unwrap();
    }

    fn paint(&self, frame: &mut FramePass) {
        if !self.visible {
            return;
        }

        let mut pass = frame.encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("legend"),
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
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.draw(0..4, 0..1);
        self.text_renderer
            .render(&frame.text.atlas, &frame.text.viewport, &mut pass)
            .unwrap();
    }
}
