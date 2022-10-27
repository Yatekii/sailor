use std::num::NonZeroU64;
use std::path::Path;

use crossbeam_channel::{unbounded, TryRecvError};
use nalgebra_glm::{vec2, vec4};
use notify::{event::ModifyKind, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use osm::*;
use pollster::block_on;
use util::StagingBelt;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::*;
use wgpu_glyph::{ab_glyph::FontArc, GlyphBrush, GlyphBrushBuilder};
use winit::{dpi::LogicalSize, event_loop::EventLoop, window::Window};

use crate::app_state::AppState;
use crate::drawing::helpers::{load_glsl, ShaderStage};

use crate::config::CONFIG;

pub struct Painter {
    pub window: Window,
    hidpi_factor: f64,
    pub device: Device,
    pub queue: Queue,
    surface: Surface,
    staging_belt: StagingBelt,
    surface_config: SurfaceConfiguration,
    blend_pipeline: RenderPipeline,
    noblend_pipeline: RenderPipeline,
    multisampled_framebuffer: TextureView,
    stencil: TextureView,
    uniform_buffer: Buffer,
    tile_transform_buffer: (Buffer, u64),
    bind_group_layout: BindGroupLayout,
    bind_group: BindGroup,
    rx: crossbeam_channel::Receiver<std::result::Result<notify::event::Event, notify::Error>>,
    _watcher: RecommendedWatcher,
    glyph_brush: GlyphBrush<()>,
    // temperature: crate::drawing::weather::Temperature,
}

impl Painter {
    /// Initializes the entire draw machinery.
    pub fn init(event_loop: &EventLoop<()>, width: u32, height: u32, app_state: &AppState) -> Self {
        let window = Window::new(event_loop).unwrap();
        window.set_inner_size(LogicalSize {
            width: width as f64,
            height: height as f64,
        });
        let factor = window.scale_factor();
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::Backends::PRIMARY);
        let surface = unsafe { instance.create_surface(&window) };

        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            // Request an adapter which can render to our surface
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .expect("Failed to find an appropiate adapter");

        let (device, queue) = block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("Main Device"),
                features: wgpu::Features::DEPTH32FLOAT_STENCIL8,
                limits: wgpu::Limits {
                    max_uniform_buffer_binding_size: 1 << 16,
                    ..wgpu::Limits::default()
                },
            },
            None,
        ))
        .expect("Failed to create device");

        let init_encoder = device.create_command_encoder(&CommandEncoderDescriptor { label: None });

        let (tx, rx) = unbounded();

        let mut watcher: RecommendedWatcher =
            match notify::recommended_watcher(move |res| tx.send(res).unwrap()) {
                Ok(watcher) => watcher,
                Err(err) => {
                    log::info!("Failed to create a watcher for the vertex shader:");
                    log::info!("{}", err);
                    panic!("Unable to load a vertex shader.");
                }
            };

        match watcher.watch(
            Path::new(&CONFIG.renderer.vertex_shader),
            RecursiveMode::Recursive,
        ) {
            Ok(_) => {}
            Err(err) => {
                log::info!(
                    "Failed to start watching {}:",
                    &CONFIG.renderer.vertex_shader
                );
                log::info!("{}", err);
            }
        };

        match watcher.watch(
            Path::new(&CONFIG.renderer.fragment_shader),
            RecursiveMode::Recursive,
        ) {
            Ok(_) => {}
            Err(err) => {
                log::info!(
                    "Failed to start watching {}:",
                    &CONFIG.renderer.fragment_shader
                );
                log::info!("{}", err);
            }
        };

        let (layer_vs_module, layer_fs_module) = Self::load_shader(
            &device,
            &CONFIG.renderer.vertex_shader,
            &CONFIG.renderer.fragment_shader,
        )
        .expect("Fatal Error. Unable to load shaders.");

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let surface_config = SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: TextureFormat::Bgra8Unorm,
            alpha_mode: CompositeAlphaMode::Auto,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Immediate,
        };

        surface.configure(&device, &surface_config);

        let multisampled_framebuffer = Self::create_multisampled_framebuffer(
            &device,
            &surface_config,
            CONFIG.renderer.msaa_samples,
        );
        let stencil = Self::create_stencil(&device, &surface_config);

        let uniform_buffer = Self::create_uniform_buffer(&device);
        let tile_transform_buffer = Self::create_tile_transform_buffer(
            &device,
            &app_state.screen,
            app_state.zoom,
            std::iter::empty::<&VisibleTile>(),
        );

        let blend_pipeline = Self::create_layer_render_pipeline(
            &device,
            &bind_group_layout,
            &layer_vs_module,
            &layer_fs_module,
            BlendComponent {
                src_factor: BlendFactor::SrcAlpha,
                dst_factor: BlendFactor::OneMinusSrcAlpha,
                operation: BlendOperation::Add,
            },
            BlendComponent {
                src_factor: BlendFactor::One,
                dst_factor: BlendFactor::OneMinusSrcAlpha,
                operation: BlendOperation::Add,
            },
            false,
        );

        let noblend_pipeline = Self::create_layer_render_pipeline(
            &device,
            &bind_group_layout,
            &layer_vs_module,
            &layer_fs_module,
            BlendComponent::REPLACE,
            BlendComponent::REPLACE,
            true,
        );

        let staging_belt = wgpu::util::StagingBelt::new(1024);

        let bind_group = Self::create_blend_bind_group(
            &device,
            &bind_group_layout,
            &uniform_buffer,
            &tile_transform_buffer,
        );

        let font =
            FontArc::try_from_slice(include_bytes!("../../../config/Ruda-Bold.ttf")).unwrap();

        let glyph_brush =
            GlyphBrushBuilder::using_font(font).build(&device, TextureFormat::Bgra8Unorm);

        // let mut temperature = crate::drawing::weather::Temperature::init(&mut device, &mut queue);

        let init_command_buf = init_encoder.finish();
        queue.submit(vec![init_command_buf]); // TODO this fix is bad

        // let width = 64 * 8;
        // let height = 64 * 8;

        // temperature.generate_texture(&mut device, &mut queue, width, height);

        Self {
            window,
            hidpi_factor: factor,
            device,
            queue,
            surface,
            staging_belt,
            surface_config,
            blend_pipeline,
            noblend_pipeline,
            multisampled_framebuffer,
            uniform_buffer,
            stencil,
            tile_transform_buffer,
            bind_group_layout,
            bind_group,
            _watcher: watcher,
            rx,
            glyph_brush,
            // temperature,
        }
    }

    fn create_layer_render_pipeline(
        device: &Device,
        bind_group_layout: &BindGroupLayout,
        vs_module: &ShaderModule,
        fs_module: &ShaderModule,
        color_blend: BlendComponent,
        alpha_blend: BlendComponent,
        depth_write_enabled: bool,
    ) -> RenderPipeline {
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[bind_group_layout],
            push_constant_ranges: &[],
        });

        device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Map"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: vs_module,
                entry_point: "main",
                buffers: &[VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: VertexStepMode::Vertex,
                    attributes: &[
                        VertexAttribute {
                            format: VertexFormat::Sint16x2,
                            offset: 0,
                            shader_location: 0,
                        },
                        VertexAttribute {
                            format: VertexFormat::Sint16x2,
                            offset: 4,
                            shader_location: 1,
                        },
                        VertexAttribute {
                            format: VertexFormat::Uint32,
                            offset: 8,
                            shader_location: 2,
                        },
                    ],
                }],
            },
            fragment: Some(FragmentState {
                module: fs_module,
                entry_point: "main",
                targets: &[Some(ColorTargetState {
                    format: TextureFormat::Bgra8Unorm,
                    blend: Some(BlendState {
                        color: color_blend,
                        alpha: alpha_blend,
                    }),
                    write_mask: ColorWrites::ALL,
                })],
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
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth32FloatStencil8,
                depth_write_enabled,
                depth_compare: CompareFunction::Greater,
                stencil: wgpu::StencilState {
                    front: StencilFaceState {
                        compare: CompareFunction::NotEqual,
                        fail_op: StencilOperation::Keep,
                        depth_fail_op: StencilOperation::Replace,
                        pass_op: StencilOperation::Replace,
                    },
                    back: StencilFaceState {
                        compare: CompareFunction::NotEqual,
                        fail_op: StencilOperation::Keep,
                        depth_fail_op: StencilOperation::Replace,
                        pass_op: StencilOperation::Replace,
                    },
                    read_mask: std::u32::MAX,
                    write_mask: std::u32::MAX,
                },
                bias: DepthBiasState {
                    constant: 0,
                    slope_scale: 0.0,
                    clamp: 0.0,
                },
            }),
            multisample: MultisampleState {
                count: CONFIG.renderer.msaa_samples,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        })
    }

    /// Creates a new bind group containing all the relevant uniform buffers.
    fn create_uniform_buffers(
        device: &Device,
        screen: &Screen,
        feature_collection: &FeatureCollection,
    ) -> Vec<(Buffer, usize)> {
        let canvas_size_len = 4 * 4;
        let canvas_size_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: as_byte_slice(&[screen.width as f32, screen.height as f32, 0.0, 0.0]),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_SRC,
        });

        let buffer = feature_collection.assemble_style_buffer();
        let len = buffer.len();
        let layer_data_len = len.max(1) * 12 * 4;
        let layer_data_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: if len == 0 {
                &[0; 48]
            } else {
                as_byte_slice(&buffer)
            },
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_SRC,
        });

        vec![
            (canvas_size_buffer, canvas_size_len),
            (layer_data_buffer, layer_data_len),
        ]
    }

    fn create_uniform_buffer(device: &Device) -> Buffer {
        let data = vec![0; Self::uniform_buffer_size() as usize];
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: as_byte_slice(&data),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });
        buffer
    }

    /// Creates a new transform buffer from the tile transforms.
    ///
    /// Ensures that the buffer has the size configured in the config, to match the size configured in the shader.
    fn create_tile_transform_buffer<'a>(
        device: &Device,
        screen: &Screen,
        z: f32,
        visible_tiles: impl Iterator<Item = &'a VisibleTile>,
    ) -> (Buffer, u64) {
        const TILE_DATA_SIZE: usize = 20;
        let tile_data_buffer_byte_size = TILE_DATA_SIZE * 4 * CONFIG.renderer.max_tiles;
        let mut data = vec![0f32; tile_data_buffer_byte_size];

        let mut i = 0;
        for vt in visible_tiles {
            let extent = vt.extent() as f32;
            let matrix = screen.tile_to_global_space(z, &vt.tile_id());
            for float in matrix.as_slice() {
                data[i] = *float;
                i += 1;
            }
            for _ in 0..4 {
                data[i] = extent;
                i += 1;
            }
        }
        (
            {
                let buffer = device.create_buffer_init(&BufferInitDescriptor {
                    label: None,
                    contents: as_byte_slice(data.as_slice()),
                    usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                });
                buffer
            },
            tile_data_buffer_byte_size as u64,
        )
    }

    fn copy_uniform_buffers(
        encoder: &mut CommandEncoder,
        source: &[(Buffer, usize)],
        destination: &Buffer,
    ) {
        let mut total_bytes = 0;
        for (buffer, len) in source {
            encoder.copy_buffer_to_buffer(buffer, 0, destination, total_bytes, *len as u64);
            total_bytes += *len as u64;
        }
    }

    fn uniform_buffer_size() -> u64 {
        4 * 4 + 12 * 4 * CONFIG.renderer.max_features
    }

    pub fn create_blend_bind_group(
        device: &Device,
        bind_group_layout: &BindGroupLayout,
        uniform_buffer: &Buffer,
        tile_transform_buffer: &(Buffer, u64),
    ) -> BindGroup {
        device.create_bind_group(&BindGroupDescriptor {
            label: None,
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: uniform_buffer,
                        offset: 0,
                        size: NonZeroU64::new(Self::uniform_buffer_size()),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: &tile_transform_buffer.0,
                        offset: 0,
                        size: NonZeroU64::new(tile_transform_buffer.1),
                    }),
                },
            ],
        })
    }

    /// Loads a shader module from a GLSL vertex and fragment shader each.
    fn load_shader(
        device: &Device,
        vertex_shader: &str,
        fragment_shader: &str,
    ) -> Result<(ShaderModule, ShaderModule), std::io::Error> {
        let vertex_shader = std::fs::read_to_string(vertex_shader)?;
        let vs_bytes = load_glsl(&vertex_shader, ShaderStage::Vertex);
        let vs_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("VertexShader"),
            source: vs_bytes,
        });

        let fragment_shader = std::fs::read_to_string(fragment_shader)?;
        let fs_bytes = load_glsl(&fragment_shader, ShaderStage::Fragment);
        let fs_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("FragmentShader"),
            source: fs_bytes,
        });

        Ok((vs_module, fs_module))
    }

    /// Reloads the shader if the file watcher has detected any change to the shader files.
    pub fn update_shader(&mut self) -> bool {
        // self.temperature.update_shader(&self.device);
        match self.rx.try_recv() {
            Ok(Ok(notify::event::Event {
                kind: EventKind::Modify(ModifyKind::Data(_)),
                ..
            })) => {
                if let Ok((vs_module, fs_module)) = Self::load_shader(
                    &self.device,
                    &CONFIG.renderer.vertex_shader,
                    &CONFIG.renderer.fragment_shader,
                ) {
                    self.blend_pipeline = Self::create_layer_render_pipeline(
                        &self.device,
                        &self.bind_group_layout,
                        &vs_module,
                        &fs_module,
                        BlendComponent {
                            src_factor: BlendFactor::SrcAlpha,
                            dst_factor: BlendFactor::OneMinusSrcAlpha,
                            operation: BlendOperation::Add,
                        },
                        BlendComponent {
                            src_factor: BlendFactor::One,
                            dst_factor: BlendFactor::OneMinusSrcAlpha,
                            operation: BlendOperation::Add,
                        },
                        false,
                    );

                    self.noblend_pipeline = Self::create_layer_render_pipeline(
                        &self.device,
                        &self.bind_group_layout,
                        &vs_module,
                        &fs_module,
                        BlendComponent::REPLACE,
                        BlendComponent::REPLACE,
                        true,
                    );
                    true
                } else {
                    false
                }
            }
            // Everything is alright but file wasn't actually changed.
            Ok(Ok(_)) => false,
            // This happens all the time when there is no new message.
            Err(TryRecvError::Empty) => false,
            Ok(Err(err)) => {
                log::info!(
                    "Something went wrong with the shader file watcher:\r\n{:?}",
                    err
                );
                false
            }
            Err(err) => {
                log::info!(
                    "Something went wrong with the shader file watcher:\r\n{:?}",
                    err
                );
                false
            }
        }
    }

    pub fn get_hidpi_factor(&self) -> f64 {
        self.hidpi_factor
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);
        self.multisampled_framebuffer = Self::create_multisampled_framebuffer(
            &self.device,
            &self.surface_config,
            CONFIG.renderer.msaa_samples,
        );
        self.stencil = Self::create_stencil(&self.device, &self.surface_config);
    }

    fn update_uniforms(
        &mut self,
        encoder: &mut CommandEncoder,
        app_state: &AppState,
        feature_collection: &FeatureCollection,
    ) {
        Self::copy_uniform_buffers(
            encoder,
            &Self::create_uniform_buffers(&self.device, &app_state.screen, feature_collection),
            &self.uniform_buffer,
        );

        self.tile_transform_buffer = Self::create_tile_transform_buffer(
            &self.device,
            &app_state.screen,
            app_state.zoom,
            app_state.visible_tiles().values(),
        );
    }

    fn create_multisampled_framebuffer(
        device: &Device,
        surface_config: &SurfaceConfiguration,
        sample_count: u32,
    ) -> TextureView {
        let multisampled_texture_extent = Extent3d {
            width: surface_config.width,
            height: surface_config.height,
            depth_or_array_layers: 1,
        };
        let multisampled_frame_descriptor = &TextureDescriptor {
            label: None,
            size: multisampled_texture_extent,
            mip_level_count: 1,
            sample_count,
            dimension: TextureDimension::D2,
            format: surface_config.format,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_DST,
        };

        device
            .create_texture(multisampled_frame_descriptor)
            .create_view(&wgpu::TextureViewDescriptor::default())
    }

    fn create_stencil(device: &Device, surface_config: &SurfaceConfiguration) -> TextureView {
        let texture_extent = Extent3d {
            width: surface_config.width,
            height: surface_config.height,
            depth_or_array_layers: 1,
        };
        let frame_descriptor = &TextureDescriptor {
            label: None,
            size: texture_extent,
            mip_level_count: 1,
            sample_count: CONFIG.renderer.msaa_samples,
            dimension: TextureDimension::D2,
            format: TextureFormat::Depth32FloatStencil8,
            // usage: TextureUsages::OUTPUT_ATTACHMENT | TextureUsages::SAMPLED,
            usage: TextureUsages::RENDER_ATTACHMENT,
        };

        device
            .create_texture(frame_descriptor)
            .create_view(&wgpu::TextureViewDescriptor::default())
    }

    pub fn paint(&mut self, hud: &mut super::ui::Hud, app_state: &mut AppState) {
        let feature_collection = app_state.feature_collection().read().unwrap().clone();
        let num_tiles = app_state
            .visible_tiles()
            .iter()
            .filter(|(_, v)| v.is_loaded_to_gpu())
            .count();
        let any_loaded = app_state
            .visible_tiles()
            .iter()
            .any(|(_, v)| v.is_loaded_to_gpu());

        // println!("Rendering {num_tiles} tiles ...");

        let features = feature_collection.get_features();
        if !features.is_empty() && any_loaded {
            if let Ok(frame) = self.surface.get_current_texture() {
                let mut encoder = self
                    .device
                    .create_command_encoder(&CommandEncoderDescriptor { label: None });
                self.update_uniforms(&mut encoder, app_state, &feature_collection);
                self.bind_group = Self::create_blend_bind_group(
                    &self.device,
                    &self.bind_group_layout,
                    &self.uniform_buffer,
                    &self.tile_transform_buffer,
                );
                {
                    let view = frame
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default());
                    let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                        label: Some("Tiles"),
                        color_attachments: &[Some(RenderPassColorAttachment {
                            view: if CONFIG.renderer.msaa_samples > 1 {
                                &self.multisampled_framebuffer
                            } else {
                                &view
                            },
                            resolve_target: if CONFIG.renderer.msaa_samples > 1 {
                                Some(&view)
                            } else {
                                None
                            },
                            ops: Operations::<wgpu::Color> {
                                load: LoadOp::Clear(wgpu::Color::TRANSPARENT),
                                store: true,
                            },
                        })],
                        depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                            view: &self.stencil,
                            depth_ops: Some(Operations::<f32> {
                                load: LoadOp::Clear(0.0),
                                store: true,
                            }),
                            stencil_ops: Some(Operations::<u32> {
                                load: LoadOp::Clear(255),
                                store: true,
                            }),
                        }),
                    });
                    render_pass.set_bind_group(0, &self.bind_group, &[]);
                    let vec = vec4(0.0, 0.0, 0.0, 1.0);
                    let screen_dimensions = vec2(
                        app_state.screen.width as f32,
                        app_state.screen.height as f32,
                    ) / 2.0;

                    for (i, vt) in app_state.visible_tiles().values().enumerate() {
                        if !vt.is_loaded_to_gpu() {
                            vt.load_to_gpu(&self.device);
                        }
                        let tile_id = vt.tile_id();
                        let matrix = app_state
                            .screen
                            .tile_to_global_space(app_state.zoom, &tile_id);
                        let start = (matrix * vec).xy() + vec2(1.0, 1.0);
                        let s = vec2(
                            {
                                let x = (start.x * screen_dimensions.x).round();
                                if x < 0.0 {
                                    0.0
                                } else {
                                    x
                                }
                            },
                            {
                                let y = (start.y * screen_dimensions.y).round();
                                if y < 0.0 {
                                    0.0
                                } else {
                                    y
                                }
                            },
                        );
                        let matrix = app_state.screen.tile_to_global_space(
                            app_state.zoom,
                            &(tile_id + TileId::new(tile_id.z, 1, 1)),
                        );
                        let end = (matrix * vec).xy() + vec2(1.0, 1.0);
                        let e = vec2(
                            {
                                let x = (end.x * screen_dimensions.x).round();
                                if x < 0.0 {
                                    0.0
                                } else {
                                    x
                                }
                            },
                            {
                                let y = (end.y * screen_dimensions.y).round();
                                if y < 0.0 {
                                    0.0
                                } else {
                                    y
                                }
                            },
                        );

                        // render_pass.set_scissor_rect(
                        //     s.x as u32,
                        //     s.y as u32,
                        //     (e.x - s.x) as u32,
                        //     (e.y - s.y) as u32,
                        // );

                        unsafe {
                            let gpu_tile = vt.gpu_tile();
                            let gpu_tile2 = std::mem::transmute(gpu_tile.as_ref());
                            vt.paint(
                                &mut render_pass,
                                &self.blend_pipeline,
                                gpu_tile2,
                                &feature_collection,
                                i as u32,
                            );
                        }

                        // hud.paint(
                        //     app_state,
                        //     &self.window,
                        //     &mut self.device,
                        //     &mut render_pass,
                        //     &self.queue,
                        // );

                        // TODO put hwd.paint here?
                    }
                }

                for (_i, vt) in app_state.visible_tiles().values().enumerate() {
                    vt.queue_text(&mut self.glyph_brush, &app_state.screen, app_state.zoom);
                }

                let view = &frame
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                let _ = self.glyph_brush.draw_queued(
                    &self.device,
                    &mut self.staging_belt,
                    &mut encoder,
                    view,
                    app_state.screen.width,
                    app_state.screen.height,
                );

                // self.temperature.paint(&mut encoder, view);

                hud.paint(
                    app_state,
                    &self.window,
                    &mut self.device,
                    &self.queue,
                    &mut encoder,
                    &frame,
                );
                self.staging_belt.finish();

                frame.present();

                self.queue.submit(vec![encoder.finish()]);
            }
        }
    }
}
