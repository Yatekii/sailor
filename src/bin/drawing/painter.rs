use std::num::NonZeroU64;
use std::sync::Arc;

use glyphon::{Cache, FontSystem, Resolution, SwashCache, TextAtlas, TextRenderer, Viewport};
use nalgebra_glm::{vec2, vec4};
use osm::config::{MAX_FEATURES, MAX_TILES};
use osm::drawing::as_byte_slice;
use osm::drawing::vertex::Vertex;
use osm::feature::collection::FeatureCollection;
use osm::math::{Screen, TileId};
use osm::platform::{self, FileWatcher, Watcher};
use util::StagingBelt;
use wgpu::naga::ShaderStage;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::*;
use winit::dpi::PhysicalSize;
use winit::window::Window;

use crate::app_state::AppState;
use crate::config::CONFIG;
use crate::drawing::helpers::load_glsl;

const TILE_DATA_BUFFER_BYTE_SIZE: u64 = 8;
// TODO: Should be u64::from once stabilized for const.
const UNIFORM_BUFFER_SIZE: u64 = 4 * 4 + 12 * 4 * (MAX_FEATURES as u64);

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
    tile_selection_buffer: Buffer,
    bind_group_layout: BindGroupLayout,
    bind_group: BindGroup,
    shader_watcher: FileWatcher,

    font_system: FontSystem,
    swash_cache: SwashCache,
    viewport: glyphon::Viewport,
    atlas: glyphon::TextAtlas,
    text_renderer: glyphon::TextRenderer,
    // temperature: crate::drawing::weather::Temperature,
}

impl Painter {
    /// Initializes the entire draw machinery.
    pub async fn init(window: Arc<Window>, size: PhysicalSize<u32>, app_state: &AppState) -> Self {
        let factor = window.scale_factor();

        let instance =
            wgpu::Instance::new(InstanceDescriptor::new_without_display_handle_from_env());
        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                // Request an adapter which can render to our surface
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .expect("Failed to find an appropiate adapter");

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Main Device"),
                required_features: Features::DEPTH32FLOAT_STENCIL8,
                required_limits: wgpu::Limits {
                    max_uniform_buffer_binding_size: 1 << 16,
                    ..wgpu::Limits::default()
                },
                memory_hints: MemoryHints::Performance,
                experimental_features: wgpu::ExperimentalFeatures::default(),
                trace: wgpu::Trace::Off,
            })
            .await
            .expect("Failed to create device");

        let init_encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("initial command encoder (loading font atlas, etc)"),
        });

        let shader_watcher = FileWatcher::watch(&[
            &CONFIG.renderer.vertex_shader,
            &CONFIG.renderer.fragment_shader,
        ]);

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
                BindGroupLayoutEntry {
                    binding: 2,
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

        // Prefer the low-latency `Immediate` mode, but fall back to `Fifo` (the
        // only guaranteed mode, and all the web allows).
        let present_mode = if surface
            .get_capabilities(&adapter)
            .present_modes
            .contains(&wgpu::PresentMode::Immediate)
        {
            wgpu::PresentMode::Immediate
        } else {
            wgpu::PresentMode::Fifo
        };

        let surface_config = SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: TextureFormat::Bgra8Unorm,
            alpha_mode: CompositeAlphaMode::Auto,
            width: size.width,
            height: size.height,
            present_mode,
            desired_maximum_frame_latency: 2,
            view_formats: vec![TextureFormat::Bgra8Unorm],
            color_space: wgpu::SurfaceColorSpace::Auto,
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
            std::iter::empty(),
        );
        let tile_selection_buffer = Self::create_tile_selection_buffer(&device, 0, 0);

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

        let staging_belt = wgpu::util::StagingBelt::new(device.clone(), 1024);

        let bind_group = Self::create_blend_bind_group(
            &device,
            &bind_group_layout,
            &uniform_buffer,
            &tile_transform_buffer,
            &tile_selection_buffer,
        );

        // Load the bundled Ruda font and use it for the default families. This
        // makes text deterministic and works on the web, which has no system fonts.
        let mut font_system = FontSystem::new_with_fonts([
            glyphon::fontdb::Source::Binary(Arc::new(
                include_bytes!("../../../config/Ruda-Regular.ttf").to_vec(),
            )),
            glyphon::fontdb::Source::Binary(Arc::new(
                include_bytes!("../../../config/Ruda-Bold.ttf").to_vec(),
            )),
        ]);
        {
            let db = font_system.db_mut();
            db.set_sans_serif_family("Ruda");
            db.set_serif_family("Ruda");
            db.set_monospace_family("Ruda");
        }
        let swash_cache = SwashCache::new();
        let cache = Cache::new(&device);
        let viewport = Viewport::new(&device, &cache);
        let mut atlas = TextAtlas::new(&device, &queue, &cache, TextureFormat::Bgra8Unorm);
        let text_renderer =
            TextRenderer::new(&mut atlas, &device, MultisampleState::default(), None);

        // let mut temperature = crate::drawing::weather::Temperature::init(&mut device, &mut queue);

        let init_command_buf = init_encoder.finish();
        queue.submit([init_command_buf]); // TODO this fix is bad

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
            tile_selection_buffer,
            bind_group_layout,
            bind_group,
            shader_watcher,
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
            bind_group_layouts: &[Some(bind_group_layout)],
            immediate_size: 0,
        });

        device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("osm layer render pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: vs_module,
                entry_point: Some("main"),
                buffers: &[Some(VertexBufferLayout {
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
                        VertexAttribute {
                            format: VertexFormat::Uint32,
                            offset: 12,
                            shader_location: 3,
                        },
                    ],
                })],
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
                depth_write_enabled: Some(depth_write_enabled),
                depth_compare: Some(CompareFunction::Greater),
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
            multiview_mask: None,
            cache: None,
        })
    }

    /// Creates a new bind group containing all the relevant uniform buffers.
    fn create_uniform_buffers(
        device: &Device,
        screen: &Screen,
        feature_collection: &FeatureCollection,
    ) -> [(Buffer, usize); 2] {
        let canvas_size_len = 4 * 4;
        let canvas_size_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("map canvas size data"),
            // screen width, screen height, selected tile, selected object
            contents: as_byte_slice(&[screen.width, screen.height, 0.0, 0.0]),
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

        [
            (canvas_size_buffer, canvas_size_len),
            (layer_data_buffer, layer_data_len),
        ]
    }

    fn create_uniform_buffer(device: &Device) -> Buffer {
        let data = [0; UNIFORM_BUFFER_SIZE as usize];

        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("tile data"),
            contents: as_byte_slice(&data),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        })
    }

    /// Creates a new transform buffer from the tile transforms.
    ///
    /// Ensures that the buffer has the size configured in the config, to match the size configured in the shader.
    fn create_tile_transform_buffer(
        device: &Device,
        screen: &Screen,
        z: f32,
        visible_tiles: impl Iterator<Item = (TileId, f32)>,
    ) -> (Buffer, u64) {
        #[derive(Copy, Clone, Debug, Default)]
        #[repr(C)]
        pub struct TileData {
            pub transform: [f32; 16],
            pub extent: f32,
            _unused: f32,
            _unused2: f32,
            _unused3: f32,
        }

        const TILE_DATA_SIZE: usize = 20;
        const TILE_DATA_BUFFER_BYTE_SIZE: usize = TILE_DATA_SIZE * 4 * MAX_TILES;
        let mut data = [TileData::default(); MAX_TILES];

        for (i, (tile_id, extent)) in visible_tiles.enumerate() {
            let matrix = screen.tile_to_screen(z, &tile_id);
            data[i].transform.copy_from_slice(matrix.as_slice());
            data[i].extent = extent;
        }
        (
            {
                device.create_buffer_init(&BufferInitDescriptor {
                    label: Some("tile transforms buffer"),
                    contents: as_byte_slice(data.as_slice()),
                    usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                })
            },
            TILE_DATA_BUFFER_BYTE_SIZE as u64,
        )
    }

    fn create_tile_selection_buffer(
        device: &Device,
        selected_tile_id: u32,
        selected_object_id: u32,
    ) -> Buffer {
        #[expect(dead_code)]
        #[derive(Copy, Clone, Debug, Default)]
        #[repr(C, packed)]
        pub struct SelectedData {
            pub selected_tile_id: u32,
            pub selected_object_id: u32,
        }

        device.create_buffer_init(&BufferInitDescriptor {
            label: Some("tile selection buffer"),
            contents: as_byte_slice(&[selected_tile_id, selected_object_id]),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        })
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

    pub fn create_blend_bind_group(
        device: &Device,
        bind_group_layout: &BindGroupLayout,
        uniform_buffer: &Buffer,
        tile_transform_buffer: &(Buffer, u64),
        tile_selection_buffer: &Buffer,
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
                        size: NonZeroU64::new(UNIFORM_BUFFER_SIZE),
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
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: tile_selection_buffer,
                        offset: 0,
                        size: NonZeroU64::new(TILE_DATA_BUFFER_BYTE_SIZE),
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
        let vertex_shader =
            platform::read_to_string(vertex_shader, include_str!("../../../config/shader.vert"));
        let fragment_shader =
            platform::read_to_string(fragment_shader, include_str!("../../../config/shader.frag"));

        let vs_bytes = load_glsl(&vertex_shader, ShaderStage::Vertex);
        let vs_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("VertexShader"),
            source: vs_bytes,
        });

        let fs_bytes = load_glsl(&fragment_shader, ShaderStage::Fragment);
        let fs_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("FragmentShader"),
            source: fs_bytes,
        });

        Ok((vs_module, fs_module))
    }

    /// Reloads the shader if the file watcher has detected any change to the shader files.
    pub fn update_shader(&mut self) -> bool {
        if !self.shader_watcher.changed() {
            return false;
        }
        let Ok((vs_module, fs_module)) = Self::load_shader(
            &self.device,
            &CONFIG.renderer.vertex_shader,
            &CONFIG.renderer.fragment_shader,
        ) else {
            return false;
        };
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
        app_state: &mut AppState,
        feature_collection: &FeatureCollection,
    ) {
        Self::copy_uniform_buffers(
            encoder,
            &Self::create_uniform_buffers(&self.device, &app_state.screen, feature_collection),
            &self.uniform_buffer,
        );

        let mut visible_tile_info = Vec::with_capacity(MAX_TILES);
        for tile_id in app_state.visible_tiles().to_vec() {
            let tile = app_state.tile_cache.get_tile(&tile_id);
            visible_tile_info.push((tile_id, tile.extent() as f32));
        }

        let (selected_tile_id, selected_object_id) = app_state
            .selected_object()
            .and_then(|object| {
                visible_tile_info
                    .iter()
                    .enumerate()
                    .find_map(|(i, (id, _))| {
                        (id == &object.tile_id).then_some((i as u32, object.id))
                    })
            })
            .unwrap_or((u32::MAX, u32::MAX));

        self.tile_transform_buffer = Self::create_tile_transform_buffer(
            &self.device,
            &app_state.screen,
            app_state.zoom,
            visible_tile_info.into_iter(),
        );

        self.tile_selection_buffer =
            Self::create_tile_selection_buffer(&self.device, selected_tile_id, selected_object_id);
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
            app_state.tile_cache.promote(tile_id);
        }

        let features = feature_collection.features();
        if !features.is_empty()
            && let wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) = self.surface.get_current_texture()
        {
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
                &self.tile_selection_buffer,
            );
            {
                let view = frame
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                    label: Some("tile polygons"),
                    color_attachments: &[Some(RenderPassColorAttachment {
                        depth_slice: None,
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
                    multiview_mask: None,
                });
                render_pass.set_bind_group(0, &self.bind_group, &[]);
                let vec = vec4(0.0, 0.0, 0.0, 1.0);
                let screen_dimensions = vec2(app_state.screen.width, app_state.screen.height) / 2.0;

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
                    let matrix = app_state
                        .screen
                        .tile_to_screen(app_state.zoom, &(*tile_id + TileId::new(tile_id.z, 1, 1)));
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
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            self.text_renderer
                .render(&self.atlas, &self.viewport, &mut pass)
                .unwrap();

            drop(pass);

            // self.temperature.paint(&mut encoder, view);

            hud.paint(
                app_state,
                &self.window,
                &self.device,
                &self.queue,
                &mut encoder,
                &frame,
            );
            self.staging_belt.finish();

            self.queue.submit([encoder.finish()]);
            self.queue.present(frame);
        }
    }
}
