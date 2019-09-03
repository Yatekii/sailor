use std::collections::BTreeMap;
use std::sync::{
    Arc,
    RwLock,
};
use nalgebra_glm::{
    vec4,
    vec2,
};
use crossbeam_channel::{
    unbounded,
    TryRecvError,
};
use notify::{
    RecursiveMode,
    RecommendedWatcher,
    Watcher,
    EventKind,
    event::{
        ModifyKind,
    },
};
use wgpu::{
    winit::{
        Window,
        EventsLoop,
        dpi::{
            LogicalSize,
        },
    },
    ShaderModule,
    SwapChainDescriptor,
    SwapChain,
    Surface,
    Device,
    Buffer,
    BindGroupLayout,
    BindGroup,
    CommandEncoder,
    RenderPipeline,
    PresentMode,
    LoadOp,
    StoreOp,
    TextureView,
    RenderPassDepthStencilAttachmentDescriptor,
    DepthStencilStateDescriptor,
};
use osm::*;

use crate::drawing::helpers::{
    ShaderStage,
    load_glsl,
};
use crate::app_state::AppState;

use crate::config::CONFIG;

pub struct Painter {
    #[cfg(feature = "vulkan")]
    pub window: Window,
    hidpi_factor: f64,
    pub device: Device,
    surface: Surface,
    swap_chain_descriptor: SwapChainDescriptor,
    swap_chain: SwapChain,
    blend_pipeline: RenderPipeline,
    noblend_pipeline: RenderPipeline,
    multisampled_framebuffer: TextureView,
    stencil: TextureView,
    uniform_buffer: Buffer,
    tile_transform_buffer: (Buffer, u64),
    loaded_tiles: BTreeMap<TileId, DrawableTile>,
    bind_group_layout: BindGroupLayout,
    bind_group: BindGroup,
    rx: crossbeam_channel::Receiver<std::result::Result<notify::event::Event, notify::Error>>,
    _watcher: RecommendedWatcher,
    feature_collection: Arc<RwLock<FeatureCollection>>,
    temperature: crate::drawing::weather::Temperature,
}

impl Painter {
    /// Initializes the entire draw machinery.
    pub fn init(events_loop: &EventsLoop, width: u32, height: u32, app_state: &AppState) -> Self {
        #[cfg(feature = "vulkan")]
        let (window, instance, size, surface, factor) = {
            let instance = wgpu::Instance::new();

            let window = Window::new(&events_loop).unwrap();
            window.set_inner_size(LogicalSize { width: width as f64, height: height as f64 });
            let factor = window.get_hidpi_factor();
            let size = window
                .get_inner_size()
                .unwrap()
                .to_physical(factor);

            let surface = instance.create_surface(&window);

            (window, instance, size, surface, factor)
        };

        #[cfg(feature = "gl")]
        let (instance, size, surface, factor) = {
            let wb = wgpu::winit::WindowBuilder::new()
                .with_dimensions(LogicalSize { width: width as f64, height: height as f64 });
            let cb = wgpu::glutin::ContextBuilder::new().with_vsync(true);
            let context = wgpu::glutin::WindowedContext::new_windowed(wb, cb, &events_loop).unwrap();

            let factor = context.window().get_hidpi_factor();
            let size = context
                .window()
                .get_inner_size()
                .unwrap()
                .to_physical(factor);

            let instance = wgpu::Instance::new(context);
            let surface = instance.get_surface();

            (instance, size, surface, factor)
        };

        let adapter = instance.get_adapter(&wgpu::AdapterDescriptor {
            power_preference: wgpu::PowerPreference::LowPower,
        });

        let mut device = adapter.request_device(&wgpu::DeviceDescriptor {
            extensions: wgpu::Extensions {
                anisotropic_filtering: false,
            },
            limits: wgpu::Limits::default(),
        });

        let init_encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { todo: 0 });

        let (tx, rx) = unbounded();
        
        let mut watcher: RecommendedWatcher = match Watcher::new_immediate(tx) {
            Ok(watcher) => watcher,
            Err(err) => {
                log::info!("Failed to create a watcher for the vertex shader:");
                log::info!("{}", err);
                panic!("Unable to load a vertex shader.");
            },
        };

        match watcher.watch(&CONFIG.renderer.vertex_shader, RecursiveMode::Recursive) {
            Ok(_) => {},
            Err(err) => {
                log::info!("Failed to start watching {}:", &CONFIG.renderer.vertex_shader);
                log::info!("{}", err);
            },
        };

        match watcher.watch(&CONFIG.renderer.fragment_shader, RecursiveMode::Recursive) {
            Ok(_) => {},
            Err(err) => {
                log::info!("Failed to start watching {}:", &CONFIG.renderer.fragment_shader);
                log::info!("{}", err);
            },
        };

        let (layer_vs_module, layer_fs_module) = Self::load_shader(
            &device, &CONFIG.renderer.vertex_shader,
            &CONFIG.renderer.fragment_shader
        ).expect("Fatal Error. Unable to load shaders.");

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            bindings: &[
                wgpu::BindGroupLayoutBinding {
                    binding: 0,
                    visibility: wgpu::ShaderStage::VERTEX,
                    ty: wgpu::BindingType::UniformBuffer,
                },
                wgpu::BindGroupLayoutBinding {
                    binding: 1,
                    visibility: wgpu::ShaderStage::VERTEX,
                    ty: wgpu::BindingType::UniformBuffer,
                },
            ]
        });

        let feature_collection = Arc::new(RwLock::new(FeatureCollection::new(CONFIG.renderer.max_features as u32)));

        let swap_chain_descriptor = wgpu::SwapChainDescriptor {
            usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8Unorm,
            width: size.width.round() as u32,
            height: size.height.round() as u32,
            present_mode: PresentMode::NoVsync,
        };

        let multisampled_framebuffer = Self::create_multisampled_framebuffer(
            &device,
            &swap_chain_descriptor,
            CONFIG.renderer.msaa_samples
        );
        let stencil = Self::create_stencil(&device, &swap_chain_descriptor);

        let uniform_buffer = Self::create_uniform_buffer(&device);
        let tile_transform_buffer = Self::create_tile_transform_buffer(
            &device,
            &app_state.screen,
            app_state.zoom,
            std::iter::empty::<&DrawableTile>()
        );

        let blend_pipeline = Self::create_layer_render_pipeline(
            &device,
            &bind_group_layout,
            &layer_vs_module,
            &layer_fs_module,
            wgpu::BlendDescriptor {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            wgpu::BlendDescriptor {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            false
        );

        let noblend_pipeline = Self::create_layer_render_pipeline(
            &device,
            &bind_group_layout,
            &layer_vs_module,
            &layer_fs_module,
            wgpu::BlendDescriptor::REPLACE,
            wgpu::BlendDescriptor::REPLACE,
            true
        );

        let swap_chain = device.create_swap_chain(
            &surface,
            &swap_chain_descriptor,
        );

        let bind_group = Self::create_blend_bind_group(
            &device,
            &bind_group_layout,
            &uniform_buffer,
            &tile_transform_buffer
        );

        let mut temperature = crate::drawing::weather::Temperature::init(&mut device);

        let init_command_buf = init_encoder.finish();
        device.get_queue().submit(&[init_command_buf]);

        let width = 64 * 8;
        let height = 64 * 8;

        temperature.generate_texture(&mut device, width, height);

        Self {
            #[cfg(feature = "vulkan")]
            window: window,
            hidpi_factor: factor,
            device,
            surface,
            swap_chain_descriptor,
            swap_chain,
            blend_pipeline,
            noblend_pipeline,
            multisampled_framebuffer,
            uniform_buffer,
            stencil,
            tile_transform_buffer,
            loaded_tiles: BTreeMap::new(),
            bind_group_layout,
            bind_group,
            _watcher: watcher,
            rx,
            feature_collection,
            temperature,
        }
    }

    fn create_layer_render_pipeline(
        device: &Device,
        bind_group_layout: &BindGroupLayout,
        vs_module: &ShaderModule,
        fs_module: &ShaderModule,
        color_blend: wgpu::BlendDescriptor,
        alpha_blend: wgpu::BlendDescriptor,
        depth_write_enabled: bool,
    ) -> RenderPipeline {
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            bind_group_layouts: &[&bind_group_layout],
        });

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            layout: &pipeline_layout,
            vertex_stage: wgpu::PipelineStageDescriptor {
                module: &vs_module,
                entry_point: "main",
            },
            fragment_stage: Some(wgpu::PipelineStageDescriptor {
                module: &fs_module,
                entry_point: "main",
            }),
            rasterization_state: wgpu::RasterizationStateDescriptor {
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: wgpu::CullMode::None,
                depth_bias: 0,
                depth_bias_slope_scale: 0.0,
                depth_bias_clamp: 0.0,
            },
            primitive_topology: wgpu::PrimitiveTopology::TriangleList,
            color_states: &[wgpu::ColorStateDescriptor {
                format: wgpu::TextureFormat::Bgra8Unorm,
                color_blend,
                alpha_blend,
                write_mask: wgpu::ColorWrite::ALL,
            }],
            depth_stencil_state: Some(DepthStencilStateDescriptor {
                format: wgpu::TextureFormat::D24UnormS8Uint,
                depth_write_enabled,
                depth_compare: wgpu::CompareFunction::Greater,
                stencil_front: wgpu::StencilStateFaceDescriptor {
                    compare: wgpu::CompareFunction::NotEqual,
                    fail_op: wgpu::StencilOperation::Keep,
                    depth_fail_op: wgpu::StencilOperation::Replace,
                    pass_op: wgpu::StencilOperation::Replace,
                },
                stencil_back: wgpu::StencilStateFaceDescriptor {
                    compare: wgpu::CompareFunction::NotEqual,
                    fail_op: wgpu::StencilOperation::Keep,
                    depth_fail_op: wgpu::StencilOperation::Replace,
                    pass_op: wgpu::StencilOperation::Replace,
                },
                stencil_read_mask: std::u32::MAX,
                stencil_write_mask: std::u32::MAX,
            }),
            index_format: wgpu::IndexFormat::Uint32,
            vertex_buffers: &[wgpu::VertexBufferDescriptor {
                stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                step_mode: wgpu::InputStepMode::Vertex,
                attributes: &[
                    wgpu::VertexAttributeDescriptor {
                        format: wgpu::VertexFormat::Short2,
                        offset: 0,
                        shader_location: 0,
                    },
                    wgpu::VertexAttributeDescriptor {
                        format: wgpu::VertexFormat::Short2,
                        offset: 4,
                        shader_location: 1,
                    },
                    wgpu::VertexAttributeDescriptor {
                        format: wgpu::VertexFormat::Uint,
                        offset: 8,
                        shader_location: 2,
                    },
                ],
            }],
            sample_count: CONFIG.renderer.msaa_samples,
        })
    }

    /// Creates a new bind group containing all the relevant uniform buffers.
    fn create_uniform_buffers(device: &Device, screen: &Screen, feature_collection: &FeatureCollection) -> Vec<(Buffer, usize)> {
        let canvas_size_len = 4 * 4;
        let canvas_size_buffer = device
            .create_buffer_mapped(
                canvas_size_len / 4,
                wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::TRANSFER_SRC,
            )
            .fill_from_slice(&[screen.width as f32, screen.height as f32, 0.0, 0.0]);

        let buffer = feature_collection.assemble_style_buffer();
        let layer_data_len = buffer.len() * 12 * 4;
        let layer_data_buffer = device
            .create_buffer_mapped(
                layer_data_len / 12 / 4,
                wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::TRANSFER_SRC,
            )
            .fill_from_slice(&buffer.as_slice());

        vec![
            (canvas_size_buffer, canvas_size_len),
            (layer_data_buffer, layer_data_len)
        ]
    }

    fn create_uniform_buffer(device: &Device) -> Buffer {
        let data = vec![0; Self::uniform_buffer_size() as usize];
        device
            .create_buffer_mapped::<u8>(
                Self::uniform_buffer_size() as usize,
                wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::TRANSFER_DST,
            )
            .fill_from_slice(&data)
    }

    /// Creates a new transform buffer from the tile transforms.
    /// 
    /// Ensures that the buffer has the size configured in the config, to match the size configured in the shader.
    fn create_tile_transform_buffer<'a>(
        device: &Device,
        screen: &Screen,
        z: f32,
        drawable_tiles: impl Iterator<Item=&'a DrawableTile>
    ) -> (Buffer, u64) {
        const TILE_DATA_SIZE: usize = 20;
        let tile_data_buffer_byte_size = TILE_DATA_SIZE * 4 * CONFIG.renderer.max_tiles;
        let mut data = vec![0f32; tile_data_buffer_byte_size];

        let mut i = 0;
        for dt in drawable_tiles {
            let matrix = screen.tile_to_global_space(z, &dt.tile_id);
            for float in matrix.as_slice() {
                data[i] = *float;
                i += 1;
            }
            for _ in 0..4 {
                data[i] = dt.extent as f32;
                i += 1;
            }
        }
        (
            device
            .create_buffer_mapped::<f32>(
                tile_data_buffer_byte_size,
                wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::TRANSFER_DST,
            )
            .fill_from_slice(data.as_slice()),
            tile_data_buffer_byte_size as u64
        )
    }

    fn copy_uniform_buffers(encoder: &mut CommandEncoder, source: &Vec<(Buffer, usize)>, destination: &Buffer) {
        let mut total_bytes = 0;
        for (buffer, len) in source {
            encoder.copy_buffer_to_buffer(
                &buffer,
                0,
                &destination,
                total_bytes,
                *len as u64
            );
            total_bytes += *len as u64;
        }
    }

    fn uniform_buffer_size() -> u64 {
        4 * 4
      + 12 * 4 * CONFIG.renderer.max_features
    }

    pub fn create_blend_bind_group(
        device: &Device,
        bind_group_layout: &BindGroupLayout,
        uniform_buffer: &Buffer,
        tile_transform_buffer: &(Buffer, u64)
    ) -> BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: bind_group_layout,
            bindings: &[
                wgpu::Binding {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer {
                        buffer: uniform_buffer,
                        range: 0 .. Self::uniform_buffer_size(),
                    },
                },
                wgpu::Binding {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer {
                        buffer: &tile_transform_buffer.0,
                        range: 0 .. tile_transform_buffer.1,
                    },
                },
            ],
        })
    }

    /// Loads a shader module from a GLSL vertex and fragment shader each.
    fn load_shader(device: &Device, vertex_shader: &str, fragment_shader: &str) -> Result<(ShaderModule, ShaderModule), std::io::Error> {
        let vs_bytes = load_glsl(&std::fs::read_to_string(vertex_shader)?, ShaderStage::Vertex);
        let vs_module = device.create_shader_module(vs_bytes.as_slice());
        let fs_bytes = load_glsl(&std::fs::read_to_string(fragment_shader)?, ShaderStage::Fragment);
        let fs_module = device.create_shader_module(fs_bytes.as_slice());
        Ok((vs_module, fs_module))
    }

    /// Reloads the shader if the file watcher has detected any change to the shader files.
    pub fn update_shader(&mut self) -> bool {
        self.temperature.update_shader(&self.device);
        match self.rx.try_recv() {
            Ok(Ok(notify::event::Event {
                kind: EventKind::Modify(ModifyKind::Data(_)),
                ..
            })) => {
                if let Ok((vs_module, fs_module)) = Self::load_shader(
                    &self.device,
                    &CONFIG.renderer.vertex_shader,
                    &CONFIG.renderer.fragment_shader
                ) {
                    self.blend_pipeline = Self::create_layer_render_pipeline(
                        &self.device,
                        &self.bind_group_layout,
                        &vs_module,
                        &fs_module,
                        wgpu::BlendDescriptor {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        wgpu::BlendDescriptor {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        false
                    );

                    self.noblend_pipeline = Self::create_layer_render_pipeline(
                        &self.device,
                        &self.bind_group_layout,
                        &vs_module,
                        &fs_module,
                        wgpu::BlendDescriptor::REPLACE,
                        wgpu::BlendDescriptor::REPLACE,
                        true
                    );
                    true
                } else {
                    false
                }
            },
            // Everything is alright but file wasn't actually changed.
            Ok(Ok(_)) => { false },
            // This happens all the time when there is no new message.
            Err(TryRecvError::Empty) => false,
            Ok(Err(err)) => {
                log::info!("Something went wrong with the shader file watcher:\r\n{:?}", err);
                false
            },
            Err(err) => {
                log::info!("Something went wrong with the shader file watcher:\r\n{:?}", err);
                false
            },
        }
    }

    pub fn update_styles(&mut self, zoom: f32, css_cache: &mut RulesCache) {
        if css_cache.update() {
            let mut feature_collection = self.feature_collection.write().unwrap();
            feature_collection.load_styles(zoom, css_cache);
        }
    }

    pub fn get_hidpi_factor(&self) -> f64 {
        self.hidpi_factor
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.swap_chain_descriptor.width = width;
        self.swap_chain_descriptor.height = height;
        self.swap_chain = self.device.create_swap_chain(&self.surface, &self.swap_chain_descriptor);
        self.multisampled_framebuffer = Self::create_multisampled_framebuffer(
            &self.device,
            &self.swap_chain_descriptor,
            CONFIG.renderer.msaa_samples
        );
        self.stencil = Self::create_stencil(&self.device, &self.swap_chain_descriptor);
    }

    fn update_uniforms<'a>(
        &mut self,
        encoder: &mut CommandEncoder,
        app_state: &AppState,
        feature_collection: &FeatureCollection
    ) {
        Self::copy_uniform_buffers(
            encoder,
            &Self::create_uniform_buffers(
                &self.device,
                &app_state.screen,
                feature_collection
            ),
            &self.uniform_buffer
        );

        self.tile_transform_buffer = Self::create_tile_transform_buffer(
            &self.device,
            &app_state.screen,
            app_state.zoom,
            self.loaded_tiles.values()
        );
    }

    fn create_multisampled_framebuffer(
        device: &Device,
        swap_chain_descriptor: &SwapChainDescriptor,
        sample_count: u32
    ) -> wgpu::TextureView {
        let multisampled_texture_extent = wgpu::Extent3d {
            width: swap_chain_descriptor.width,
            height: swap_chain_descriptor.height,
            depth: 1,
        };
        let multisampled_frame_descriptor = &wgpu::TextureDescriptor {
            size: multisampled_texture_extent,
            array_layer_count: 1,
            mip_level_count: 1,
            sample_count: sample_count,
            dimension: wgpu::TextureDimension::D2,
            format: swap_chain_descriptor.format,
            usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT | wgpu::TextureUsage::SAMPLED,
        };

        device.create_texture(multisampled_frame_descriptor).create_default_view()
    }

    fn create_stencil(device: &Device, swap_chain_descriptor: &SwapChainDescriptor) -> wgpu::TextureView {
        let texture_extent = wgpu::Extent3d {
            width: swap_chain_descriptor.width,
            height: swap_chain_descriptor.height,
            depth: 1,
        };
        let frame_descriptor = &wgpu::TextureDescriptor {
            size: texture_extent,
            array_layer_count: 1,
            mip_level_count: 1,
            sample_count: CONFIG.renderer.msaa_samples,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::D24UnormS8Uint,
            usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT | wgpu::TextureUsage::SAMPLED,
        };

        device.create_texture(frame_descriptor).create_default_view()
    }

    fn load_tiles(&mut self, app_state: &mut AppState) {
        let tile_field = app_state.screen.get_tile_boundaries_for_zoom_level(app_state.zoom, 1);

        // Remove old bigger tiles which are not in the FOV anymore.
        let old_tile_field = app_state.screen.get_tile_boundaries_for_zoom_level(app_state.zoom - 1.0, 2);
        let key_iter: Vec<_> = self.loaded_tiles.keys().copied().collect();
        for key in key_iter {
            if key.z == (app_state.zoom - 1.0) as u32 {
                if !old_tile_field.contains(&key) {
                    self.loaded_tiles.remove(&key);
                }
            } else {
                if !tile_field.contains(&key) {
                    self.loaded_tiles.remove(&key);
                }
            }
        }

        app_state.tile_cache.fetch_tiles();
        for tile_id in tile_field.iter() {
            if !self.loaded_tiles.contains_key(&tile_id) {
                app_state.tile_cache.request_tile(&tile_id, self.feature_collection.clone(), CONFIG.renderer.selection_tags.clone());
                
                let tile_cache = &mut app_state.tile_cache;
                if let Some(tile) = tile_cache.try_get_tile(&tile_id) {

                    let drawable_tile = DrawableTile::load_from_tile_id(
                        &self.device,
                        tile_id,
                        &tile
                    );

                    self.loaded_tiles.insert(
                        tile_id.clone(),
                        drawable_tile
                    );

                    // Remove old bigger tile when all 4 smaller tiles are loaded.
                    let mut count = 0;
                    let num_x = (tile_id.x / 2) * 2;
                    let num_y = (tile_id.y / 2) * 2;
                    for tile_id in &[
                        TileId::new(tile_id.z, num_x, num_y),
                        TileId::new(tile_id.z, num_x + 1, num_y),
                        TileId::new(tile_id.z, num_x + 1, num_y + 1),
                        TileId::new(tile_id.z, num_x, num_y + 1),
                    ] {
                        if !tile_field.contains(tile_id) {
                            count += 1;
                            continue;
                        }
                        if self.loaded_tiles.contains_key(tile_id) {
                            count += 1;
                        }
                    }
                    if count == 4 {
                        self.loaded_tiles.remove(&TileId::new(tile_id.z - 1, num_x / 2, num_y / 2));
                    }

                    // Remove old smaller tiles when all 4 smaller tiles are loaded.
                    for tile_id in &[
                        TileId::new(tile_id.z + 1, tile_id.x * 2, tile_id.y * 2),
                        TileId::new(tile_id.z + 1, tile_id.x * 2 + 1, tile_id.y * 2),
                        TileId::new(tile_id.z + 1, tile_id.x * 2 + 1, tile_id.y * 2 + 1),
                        TileId::new(tile_id.z + 1, tile_id.x * 2, tile_id.y * 2 + 1),
                    ] {
                        self.loaded_tiles.remove(tile_id);
                    }
                } else {
                    log::trace!("Could not read tile {} from cache.", tile_id);
                }
            }
        }

        let mut feature_collection = self.feature_collection.write().unwrap();
        feature_collection.load_styles(app_state.zoom, &mut app_state.css_cache);
    }

    pub fn paint(&mut self, hud: &mut super::ui::HUD, app_state: &mut AppState) {
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { todo: 0 });
        self.load_tiles(app_state);
        let feature_collection = {
            let lock = self.feature_collection.clone();
            let feature_collection = lock.read().unwrap();
            (*feature_collection).clone()
        };
        self.update_uniforms(&mut encoder, &app_state, &feature_collection);
        self.bind_group = Self::create_blend_bind_group(
            &self.device,
            &self.bind_group_layout,
            &self.uniform_buffer,
            &self.tile_transform_buffer
        );
        let num_tiles = self.loaded_tiles.len();
        let features = feature_collection.get_features();
        if features.len() > 0 && num_tiles > 0 {
            let frame = self.swap_chain.get_next_texture();
            {
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                        attachment: if CONFIG.renderer.msaa_samples > 1 { &self.multisampled_framebuffer } else { &frame.view },
                        resolve_target: if CONFIG.renderer.msaa_samples > 1 { Some(&frame.view) } else { None },
                        load_op: wgpu::LoadOp::Clear,
                        store_op: wgpu::StoreOp::Store,
                        clear_color: wgpu::Color::TRANSPARENT,
                    }],
                    depth_stencil_attachment: Some(RenderPassDepthStencilAttachmentDescriptor{
                        attachment: &self.stencil,
                        depth_load_op: LoadOp::Clear,
                        depth_store_op: StoreOp::Store,
                        clear_depth: 0.0,
                        stencil_load_op: LoadOp::Clear,
                        stencil_store_op: StoreOp::Store,
                        clear_stencil: 255,
                    }),
                });
                render_pass.set_bind_group(0, &self.bind_group, &[]);
                let vec = vec4(0.0, 0.0, 0.0, 1.0);
                let screen_dimensions = vec2(
                    app_state.screen.width as f32,
                    app_state.screen.height as f32
                ) / 2.0;
                for (i, dt) in self.loaded_tiles.values_mut().enumerate() {
                    let matrix = app_state.screen.tile_to_global_space(
                        app_state.zoom,
                        &dt.tile_id
                    );
                    let start = (matrix * &vec).xy() + &vec2(1.0, 1.0);
                    let s = vec2({
                        let x = (start.x * screen_dimensions.x).round();
                        if x < 0.0 { 0.0 } else { x }
                    }, {
                        let y = (start.y * screen_dimensions.y).round();
                        if y < 0.0 { 0.0 } else { y }
                    });
                    let matrix = app_state.screen.tile_to_global_space(
                        app_state.zoom,
                        &(dt.tile_id + TileId::new(dt.tile_id.z, 1, 1))
                    );
                    let end = (matrix * &vec).xy() + &vec2(1.0, 1.0);
                    let e = vec2({
                        let x = (end.x * screen_dimensions.x).round();
                        if x < 0.0 { 0.0 } else { x }
                    }, {
                        let y = (end.y * screen_dimensions.y).round();
                        if y < 0.0 { 0.0 } else { y }
                    });

                    render_pass.set_scissor_rect(
                        s.x as u32,
                        s.y as u32,
                        (e.x - s.x) as u32,
                        (e.y - s.y) as u32
                    );
                    dt.paint(&mut render_pass, &self.blend_pipeline, &self.noblend_pipeline, &feature_collection, i as u32);
                }
            }

            self.temperature.paint(&mut encoder, &frame.view);

            hud.paint(
                app_state,
                &self.window,
                app_state.screen.width as f64,
                app_state.screen.height as f64,
                self.hidpi_factor,
                &mut self.device,
                &mut encoder,
                &frame.view,
            );
            self.device.get_queue().submit(&[encoder.finish()]);
        }
    }
}