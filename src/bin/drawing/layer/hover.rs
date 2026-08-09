use egui::{ClippedPrimitive, Context, TextureId};
use egui_wgpu::{Renderer, RendererOptions, ScreenDescriptor};

use super::{FramePass, Layer, LayerCtx};

/// A single key/value tag, borrowed for read-only render access (no allocation).
pub struct Tag<'a> {
    pub name: &'a str,
    pub value: &'a str,
}

/// One feature under the cursor, all borrowed. `layer` is the tile layer name
/// (the tooltip heading, e.g. "poi"), `class` its OMT class (e.g. "parking"),
/// `color` its styled fill (for the legend dot).
pub struct HoverItem<'a> {
    pub layer: Option<&'a str>,
    pub class: Option<&'a str>,
    pub title: Option<&'a str>,
    pub color: Option<[u8; 3]>,
    pub tags: &'a [Tag<'a>],
}

/// What the cursor is over this frame, fed to overlay layers so a tooltip can be
/// drawn without coupling to the app or the map. All borrowed, nothing cloned.
pub struct HoverInfo<'a> {
    /// Cursor position in logical points (window-relative).
    pub cursor: (f32, f32),
    /// Physical-pixels-per-point (scale factor), for point-space UI toolkits.
    pub pixels_per_point: f32,
    pub items: &'a [HoverItem<'a>],
}

/// Hover tooltip overlay. Draws a small egui panel at the cursor listing the
/// features under it, like the planetiler demo. Self-contained: it owns its own
/// egui context and renderer, so it is a drop-in `Layer` coupled to neither the
/// app's UI nor the basemap. Swap it for a custom renderer and nothing else
/// changes — the input is `LayerCtx::hover`.
pub struct HoverLayer {
    ctx: Context,
    renderer: Renderer,
    jobs: Vec<ClippedPrimitive>,
    descriptor: ScreenDescriptor,
    /// Textures to free next frame (render() consumes this frame's jobs first).
    free: Vec<TextureId>,
    visible: bool,
}

impl HoverLayer {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let ctx = Context::default();
        // Light panel with dark text, matching the demo tooltip.
        ctx.set_visuals(egui::Visuals::light());
        Self {
            ctx,
            renderer: Renderer::new(device, format, RendererOptions::default()),
            jobs: Vec::new(),
            descriptor: ScreenDescriptor {
                size_in_pixels: [0, 0],
                pixels_per_point: 1.0,
            },
            free: Vec::new(),
            visible: true,
        }
    }
}

impl Layer for HoverLayer {
    fn name(&self) -> &str {
        "hover"
    }

    fn visible(&self) -> bool {
        self.visible
    }

    fn update(&mut self, ctx: &mut LayerCtx) {
        // Free last frame's textures now that they've been rendered.
        for id in self.free.drain(..) {
            self.renderer.free_texture(&id);
        }
        self.jobs.clear();

        let Some(hover) = ctx.hover.as_ref() else {
            return;
        };
        if hover.items.is_empty() {
            return;
        }

        let (w, h) = ctx.resolution;
        let ppp = hover.pixels_per_point;
        self.descriptor = ScreenDescriptor {
            size_in_pixels: [w, h],
            pixels_per_point: ppp,
        };

        // Flip the tooltip inward near the window edges so it opens over the map
        // instead of under the side/bottom panels. Cursor is in logical points.
        let (sw, sh) = (w as f32 / ppp, h as f32 / ppp);
        let (cx, cy) = hover.cursor;
        let right = cx > sw * 0.6;
        let bottom = cy > sh * 0.7;
        let pivot = match (right, bottom) {
            (false, false) => egui::Align2::LEFT_TOP,
            (true, false) => egui::Align2::RIGHT_TOP,
            (false, true) => egui::Align2::LEFT_BOTTOM,
            (true, true) => egui::Align2::RIGHT_BOTTOM,
        };
        let pos = egui::pos2(
            cx + if right { -12.0 } else { 12.0 },
            cy + if bottom { -12.0 } else { 12.0 },
        );
        let items = hover.items;

        self.ctx.set_pixels_per_point(ppp);
        let raw_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(w as f32 / ppp, h as f32 / ppp),
            )),
            ..Default::default()
        };

        let full = self.ctx.run_ui(raw_input, |ui| {
            egui::Area::new("hover_tooltip".into())
                .fixed_pos(pos)
                .pivot(pivot)
                // Height-aware: egui nudges the whole tooltip back on-screen if the
                // pivot flip alone would still overflow (e.g. a tall tooltip near the bottom).
                .constrain(true)
                .interactable(false)
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        // Let rows extend rather than wrap "render_height" onto two lines.
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                        for (i, item) in items.iter().enumerate() {
                            if i > 0 {
                                ui.separator();
                            }
                            // Heading: legend dot in the layer's fill color + bold name.
                            ui.horizontal(|ui| {
                                if let Some([r, g, b]) = item.color {
                                    let (rect, _) = ui.allocate_exact_size(
                                        egui::vec2(11.0, 11.0),
                                        egui::Sense::hover(),
                                    );
                                    ui.painter().rect_filled(
                                        rect,
                                        2.0,
                                        egui::Color32::from_rgb(r, g, b),
                                    );
                                }
                                ui.strong(item.layer.unwrap_or("feature"));
                            });
                            egui::Grid::new(("hover", i))
                                .num_columns(2)
                                .spacing([12.0, 2.0])
                                .striped(true)
                                .show(ui, |ui| {
                                    // Weak gray key, plain value — so the two columns read apart.
                                    let mut row = |k: &str, v: &str| {
                                        ui.weak(k);
                                        ui.label(v);
                                        ui.end_row();
                                    };
                                    if let Some(class) = item.class {
                                        row("class", class);
                                    }
                                    if let Some(title) = item.title {
                                        row("name", title);
                                    }
                                    for tag in item.tags {
                                        row(tag.name, tag.value);
                                    }
                                });
                        }
                    });
                });
        });

        self.jobs = self.ctx.tessellate(full.shapes, full.pixels_per_point);
        for (id, deltas) in &full.textures_delta.set {
            for delta in deltas {
                self.renderer
                    .update_texture(ctx.device, ctx.queue, *id, delta);
            }
        }
        self.renderer.update_buffers(
            ctx.device,
            ctx.queue,
            ctx.encoder,
            &self.jobs,
            &self.descriptor,
        );
        self.free = full.textures_delta.free.iter().copied().collect();
    }

    fn paint(&self, frame: &mut FramePass) {
        if self.jobs.is_empty() {
            return;
        }
        let mut pass = frame
            .encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("hover"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: frame.view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            })
            .forget_lifetime();
        self.renderer
            .render(&mut pass, &self.jobs, &self.descriptor);
    }
}
