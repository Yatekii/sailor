use std::num::NonZeroU64;
use std::path::Path;
use std::sync::Arc;

use crossbeam_channel::{unbounded, Receiver, TryRecvError};
use glyphon::{Cache, FontSystem, Resolution, SwashCache, TextAtlas, TextRenderer, Viewport};
use nalgebra_glm::{vec2, vec4};
use notify::{event::ModifyKind, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use osm::drawing::as_byte_slice;
use osm::drawing::vertex::Vertex;
use osm::feature::collection::FeatureCollection;
use osm::math::{Screen, TileId};
use osm::vector_tile::tile::Tile;
use pollster::block_on;
use util::StagingBelt;
use wgpu::naga::ShaderStage;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::*;
use winit::dpi::PhysicalSize;
use winit::window::Window;

use crate::app_state::AppState;
use crate::drawing::helpers::load_glsl;

use crate::config::CONFIG;

pub struct Painter {
    pub window: Arc<Window>,
    hidpi_factor: f64,
    pub device: Device,
    pub queue: Queue,
    surface: Surface<'static>,
    staging_belt: StagingBelt,
    pub surface_config: SurfaceConfiguration,
    blend_pipeline: RenderPipeline,
    noblend_pipeline: RenderPipeline,
    multisampled_framebuffer: TextureView,
    stencil: TextureView,
    uniform_buffer: Buffer,
    tile_transform_buffer: (Buffer, u64),
    bind_group_layout: BindGroupLayout,
    bind_group: BindGroup,
    rx: Receiver<Result<notify::event::Event, notify::Error>>,
    _watcher: RecommendedWatcher,

    font_system: FontSystem,
    swash_cache: SwashCache,
    viewport: glyphon::Viewport,
    atlas: glyphon::TextAtlas,
    text_renderer: glyphon::TextRenderer,
    // temperature: crate::drawing::weather::Temperature,
}

impl Painter {
    /// Initializes the entire draw machinery.
    pub fn init(window: Arc<Window>, size: PhysicalSize<u32>, app_state: &AppState) -> Self {
        let factor = window.scale_factor();

        let instance = wgpu::Instance::new(&InstanceDescriptor::default());
        let surface = instance.create_surface(window.clone()).unwrap();

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
                required_features: Features::DEPTH32FLOAT_STENCIL8,
                required_limits: wgpu::Limits {
                    max_uniform_buffer_binding_size: 1 << 16,
                    ..wgpu::Limits::default()
                },
                memory_hints: MemoryHints::Performance,
            },
            None,
        ))
        .expect("Failed to create device");

        let init_encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("initial command encoder (loading font atlas, etc)"),
        });

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
            label: Some("tile vertex stage bindings"),
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
            desired_maximum_frame_latency: 2,
            view_formats: vec![TextureFormat::Bgra8Unorm],
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
            std::iter::empty::<&Tile>(),
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

        let font_system = FontSystem::new();
        let swash_cache = SwashCache::new();
        let cache = Cache::new(&device);
        let viewport = Viewport::new(&device, &cache);
        let mut atlas = TextAtlas::new(&device, &queue, &cache, TextureFormat::Bgra8Unorm);
        let text_renderer =
            TextRenderer::new(&mut atlas, &device, MultisampleState::default(), None);

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
            font_system,
            swash_cache,
            viewport,
            atlas,
            text_renderer,
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
            label: Some("osm layer render pipeline layout"),
            bind_group_layouts: &[bind_group_layout],
            push_constant_ranges: &[],
        });

        device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("osm layer render pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: vs_module,
                entry_point: Some("main"),
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
                compilation_options: PipelineCompilationOptions::default(),
            },
            fragment: Some(FragmentState {
                module: fs_module,
                entry_point: Some("main"),
                targets: &[Some(ColorTargetState {
                    format: TextureFormat::Bgra8Unorm,
                    blend: Some(BlendState {
                        color: color_blend,
                        alpha: alpha_blend,
                    }),
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
                    read_mask: u32::MAX,
                    write_mask: u32::MAX,
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
            cache: None,
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
            label: Some("map canvas size data"),
            contents: as_byte_slice(&[screen.width as f32, screen.height as f32, 0.0, 0.0]),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_SRC,
        });

        let buffer = feature_collection.assemble_style_buffer();
        let len = buffer.len();
        let layer_data_len = len.max(1) * 12 * 4;
        let layer_data_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("feature style data"),
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
            label: Some("tile data"),
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
        visible_tiles: impl Iterator<Item = &'a Tile>,
    ) -> (Buffer, u64) {
        const TILE_DATA_SIZE: usize = 20;
        let tile_data_buffer_byte_size = TILE_DATA_SIZE * 4 * CONFIG.renderer.max_tiles;
        let mut data = vec![0f32; tile_data_buffer_byte_size];

        let mut i = 0;
        for vt in visible_tiles {
            let extent = vt.extent() as f32;
            let matrix = screen.tile_to_screen(z, &vt.tile_id());
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
                    label: Some("tile transforms buffer"),
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
            label: Some("bind vertex stage buffers"),
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
            app_state
                .visible_tiles()
                .iter()
                .map(|tile_id| app_state.tile_cache.get_tile(tile_id)),
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
            label: Some("MSAA texture render target"),
            size: multisampled_texture_extent,
            mip_level_count: 1,
            sample_count,
            dimension: TextureDimension::D2,
            format: surface_config.format,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_DST,
            view_formats: &[surface_config.format],
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
            label: Some("tile cutoff stencil"),
            size: texture_extent,
            mip_level_count: 1,
            sample_count: CONFIG.renderer.msaa_samples,
            dimension: TextureDimension::D2,
            format: TextureFormat::Depth32FloatStencil8,
            // usage: TextureUsages::OUTPUT_ATTACHMENT | TextureUsages::SAMPLED,
            usage: TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[TextureFormat::Depth32FloatStencil8],
        };

        device
            .create_texture(frame_descriptor)
            .create_view(&wgpu::TextureViewDescriptor::default())
    }

    pub fn paint(&mut self, hud: &mut super::ui::Hud, app_state: &mut AppState) {
        let feature_collection = app_state.feature_collection().read().unwrap().clone();

        for tile_id in &mut app_state.visible_tiles {
            let tile = app_state.tile_cache.try_get_tile_mut(tile_id).unwrap();
            tile.load_to_gpu(&self.device);
        }

        let features = feature_collection.features();
        if !features.is_empty() {
            if let Ok(frame) = self.surface.get_current_texture() {
                let mut encoder = self
                    .device
                    .create_command_encoder(&CommandEncoderDescriptor {
                        label: Some("tile polygon encoder"),
                    });
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
                        label: Some("tile polygons"),
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
                                store: StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                            view: &self.stencil,
                            depth_ops: Some(Operations::<f32> {
                                load: LoadOp::Clear(0.0),
                                store: StoreOp::Store,
                            }),
                            stencil_ops: Some(Operations::<u32> {
                                load: LoadOp::Clear(255),
                                store: StoreOp::Store,
                            }),
                        }),
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });
                    render_pass.set_bind_group(0, &self.bind_group, &[]);
                    let vec = vec4(0.0, 0.0, 0.0, 1.0);
                    let screen_dimensions = vec2(
                        app_state.screen.width as f32,
                        app_state.screen.height as f32,
                    ) / 2.0;

                    for (i, tile_id) in app_state.visible_tiles().iter().enumerate() {
                        let matrix = app_state.screen.tile_to_screen(app_state.zoom, tile_id);
                        let start = (matrix * vec).xy() + vec2(1.0, 1.0);
                        let s = vec2(
                            (start.x * screen_dimensions.x)
                                .round()
                                .max(0.0)
                                .min(screen_dimensions.x * 2.0),
                            (start.y * screen_dimensions.y)
                                .round()
                                .max(0.0)
                                .min(screen_dimensions.y * 2.0),
                        );
                        let matrix = app_state.screen.tile_to_screen(
                            app_state.zoom,
                            &(*tile_id + TileId::new(tile_id.z, 1, 1)),
                        );
                        let end = (matrix * vec).xy() + vec2(1.0, 1.0);
                        let e = vec2(
                            (end.x * screen_dimensions.x)
                                .round()
                                .max(0.0)
                                .min(screen_dimensions.x * 2.0),
                            (end.y * screen_dimensions.y)
                                .round()
                                .max(0.0)
                                .min(screen_dimensions.y * 2.0),
                        );
                        let width = (e.x - s.x) as u32;
                        let height = (e.y - s.y) as u32;

                        if width > 0 && height > 0 {
                            render_pass.set_scissor_rect(s.x as u32, s.y as u32, width, height);
                        }

                        let tile = app_state.tile_cache.try_get_tile(tile_id).unwrap();
                        let gpu_tile = tile.gpu_tile();
                        tile.paint(
                            &mut render_pass,
                            &self.blend_pipeline,
                            gpu_tile,
                            &feature_collection,
                            i as u32,
                        );
                    }
                }

                self.viewport.update(
                    &self.queue,
                    Resolution {
                        width: self.surface_config.width,
                        height: self.surface_config.height,
                    },
                );

                let tile_cache = &mut app_state.tile_cache;
                let screen = &app_state.screen;
                let zoom = app_state.zoom;
                for tile_id in &app_state.visible_tiles {
                    let tile = tile_cache.get_tile_mut(tile_id);
                    tile.prepare_text(&mut self.font_system);
                }
                let text_areas = app_state.visible_tiles.iter().flat_map(|tile_id| {
                    let tile = tile_cache.get_tile(tile_id);
                    tile.queue_text(screen, zoom)
                });

                self.text_renderer
                    .prepare(
                        &self.device,
                        &self.queue,
                        &mut self.font_system,
                        &mut self.atlas,
                        &self.viewport,
                        text_areas,
                        &mut self.swash_cache,
                    )
                    .unwrap();

                let view = &frame
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                    label: Some("tile text pass"),
                    color_attachments: &[Some(RenderPassColorAttachment {
                        view,
                        resolve_target: None,
                        ops: Operations {
                            load: LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

                self.text_renderer
                    .render(&self.atlas, &self.viewport, &mut pass)
                    .unwrap();

                drop(pass);

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

                self.queue.submit(vec![encoder.finish()]);
                frame.present();
            }
        }
    }
}
