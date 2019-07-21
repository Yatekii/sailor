use crate::drawing::layer_collection::LayerCollection;
use crate::vector_tile::math::Screen;
use wgpu::TextureView;
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
use wgpu::Surface;
use wgpu::CommandEncoder;
use crate::vector_tile::math::TileId;
use crate::drawing::{
    drawable_tile::DrawableTile,
};
use crate::css::RulesCache;
use std::collections::BTreeMap;

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
    Device,
    Buffer,
    BindGroupLayout,
    BindGroup,
    RenderPipeline,
    Sampler,
    PresentMode,
    LoadOp,
    StoreOp,
    RenderPassDepthStencilAttachmentDescriptor,
    DepthStencilStateDescriptor,
};

use super::{
    helpers::{
        ShaderStage,
        load_glsl,
    },
    vertex::{
        Vertex,
    },
};

use std::sync::{
    Arc,
    RwLock,
};

use crate::app_state::AppState;

const MSAA_SAMPLES: u32 = 1;

pub struct Painter {
    #[cfg(feature = "vulkan")]
    _window: Window,
    hidpi_factor: f64,
    device: Device,
    surface: Surface,
    swap_chain_descriptor: SwapChainDescriptor,
    swap_chain: SwapChain,
    layer_render_pipeline: RenderPipeline,
    multisampled_framebuffer: TextureView,
    framebuffer: TextureView,
    stencil: TextureView,
    uniform_buffer: Buffer,
    tile_transform_buffer: (Buffer, u64),
    loaded_tiles: BTreeMap<TileId, DrawableTile>,
    bind_group_layout: BindGroupLayout,
    bind_group: BindGroup,
    sampler: Sampler,
    vertex_shader: String,
    fragment_shader: String,
    rx: crossbeam_channel::Receiver<std::result::Result<notify::event::Event, notify::Error>>,
    _watcher: RecommendedWatcher,
    layer_collection: Arc<RwLock<LayerCollection>>,
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

        let layer_vertex_shader = "config/shader.vert".to_string();
        let layer_fragment_shader = "config/shader.frag".to_string();

        match watcher.watch(&layer_vertex_shader, RecursiveMode::Recursive) {
            Ok(_) => {},
            Err(err) => {
                log::info!("Failed to start watching {}:", &layer_vertex_shader);
                log::info!("{}", err);
            },
        };

        match watcher.watch(&layer_fragment_shader, RecursiveMode::Recursive) {
            Ok(_) => {},
            Err(err) => {
                log::info!("Failed to start watching {}:", &layer_fragment_shader);
                log::info!("{}", err);
            },
        };

        let (layer_vs_module, layer_fs_module) = Self::load_shader(&device, &layer_vertex_shader, &layer_fragment_shader).expect("Fatal Error. Unable to load shaders.");
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
                wgpu::BindGroupLayoutBinding {
                    binding: 2,
                    visibility: wgpu::ShaderStage::FRAGMENT,
                    ty: wgpu::BindingType::SampledTexture,
                },
                wgpu::BindGroupLayoutBinding {
                    binding: 3,
                    visibility: wgpu::ShaderStage::FRAGMENT,
                    ty: wgpu::BindingType::Sampler,
                }
            ]
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            lod_min_clamp: -100.0,
            lod_max_clamp: 100.0,
            compare_function: wgpu::CompareFunction::Always,
        });

        let layer_collection = Arc::new(RwLock::new(LayerCollection::new(20, 500)));

        let swap_chain_descriptor = wgpu::SwapChainDescriptor {
            usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8Unorm,
            width: size.width.round() as u32,
            height: size.height.round() as u32,
            present_mode: PresentMode::NoVsync,
        };

        let multisampled_framebuffer = Self::create_multisampled_framebuffer(&device, &swap_chain_descriptor, MSAA_SAMPLES);
        let framebuffer = Self::create_framebuffer(&device, &swap_chain_descriptor);
        let stencil = Self::create_stencil(&device, &swap_chain_descriptor);

        let uniform_buffer = Self::create_uniform_buffer(&device);
        let tile_transform_buffer = Self::create_tile_transform_buffer(&device, &app_state.screen, app_state.zoom, std::iter::empty::<&DrawableTile>());

        let layer_render_pipeline = Self::create_layer_render_pipeline(&device, &bind_group_layout, &layer_vs_module, &layer_fs_module);

        let swap_chain = device.create_swap_chain(
            &surface,
            &swap_chain_descriptor,
        );

        let bind_group = Self::create_blend_bind_group(
            &device,
            &bind_group_layout,
            &uniform_buffer,
            &tile_transform_buffer,
            &framebuffer,
            &sampler
        );

        let init_command_buf = init_encoder.finish();
        device.get_queue().submit(&[init_command_buf]);

        Self {
            #[cfg(feature = "vulkan")]
            _window: window,
            hidpi_factor: factor,
            device,
            surface,
            swap_chain_descriptor,
            swap_chain,
            layer_render_pipeline,
            multisampled_framebuffer,
            framebuffer,
            uniform_buffer,
            stencil,
            tile_transform_buffer,
            loaded_tiles: BTreeMap::new(),
            bind_group_layout,
            bind_group,
            sampler,
            vertex_shader: layer_vertex_shader,
            fragment_shader: layer_fragment_shader,
            _watcher: watcher,
            rx,
            layer_collection,
        }
    }

    fn create_layer_render_pipeline(
        device: &Device,
        bind_group_layout: &BindGroupLayout,
        vs_module: &ShaderModule,
        fs_module: &ShaderModule
    ) -> RenderPipeline {
        dbg!(std::mem::size_of::<Vertex>());
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
                color_blend: wgpu::BlendDescriptor {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha_blend: wgpu::BlendDescriptor {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
                write_mask: wgpu::ColorWrite::ALL,
            }],
            // depth_stencil_state: None,
            depth_stencil_state: Some(DepthStencilStateDescriptor {
                format: wgpu::TextureFormat::D24UnormS8Uint,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
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
                        format: wgpu::VertexFormat::Float2,
                        offset: 4,
                        shader_location: 1,
                    },
                    wgpu::VertexAttributeDescriptor {
                        format: wgpu::VertexFormat::Uint,
                        offset: 12,
                        shader_location: 2,
                    },
                ],
            }],
            sample_count: MSAA_SAMPLES,
        })
    }

    /// Creates a new bind group containing all the relevant uniform buffers.
    fn create_uniform_buffers(device: &Device, screen: &Screen, layer_collection: &LayerCollection) -> Vec<(Buffer, usize)> {
        let canvas_size_len = 4 * 4;
        let canvas_size_buffer = device
            .create_buffer_mapped(
                canvas_size_len / 4,
                wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::TRANSFER_SRC,
            )
            .fill_from_slice(&[screen.width as f32, screen.height as f32, 0.0, 0.0]);

        let buffer = layer_collection.assemble_style_buffer();
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
        device
            .create_buffer_mapped::<u8>(
                Self::uniform_buffer_size() as usize,
                wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::TRANSFER_DST,
            )
            .fill_from_slice(&[0; Self::uniform_buffer_size() as usize])
    }

    fn create_tile_transform_buffer<'a>(
        device: &Device,
        screen: &Screen,
        z: f32,
        drawable_tiles: impl Iterator<Item=&'a DrawableTile>
    ) -> (Buffer, u64) {
        let tile_data = drawable_tiles
            .flat_map(|dt| {
                let mut data = screen
                    .tile_to_global_space(z, &dt.tile_id)
                    .as_slice()
                    .iter()
                    .map(|f| *f)
                    .collect::<Vec<_>>();
                data.push(dt.extent as f32);
                data.push(dt.extent as f32);
                data.push(dt.extent as f32);
                data.push(dt.extent as f32);
                data
            }).collect::<Vec<f32>>();
        (
            device
            .create_buffer_mapped::<f32>(
                tile_data.len(),
                wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::TRANSFER_DST,
            )
            .fill_from_slice(tile_data.as_slice()),
            tile_data.len() as u64
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

    const fn uniform_buffer_size() -> u64 {
        4 * 4
      + 12 * 4 * 500
    }

    pub fn create_blend_bind_group(
        device: &Device,
        bind_group_layout: &BindGroupLayout,
        uniform_buffer: &Buffer,
        tile_transform_buffer: &(Buffer, u64),
        texture_view: &TextureView,
        sampler: &Sampler,
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
                wgpu::Binding {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(texture_view),
                },
                wgpu::Binding {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&sampler)
                }
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
        match self.rx.try_recv() {
            Ok(Ok(notify::event::Event {
                kind: EventKind::Modify(ModifyKind::Data(_)),
                ..
            })) => {
                if let Ok((vs_module, fs_module)) = Self::load_shader(&self.device, &self.vertex_shader, &self.fragment_shader) {
                    self.layer_render_pipeline = Self::create_layer_render_pipeline(&self.device, &self.bind_group_layout, &vs_module, &fs_module);
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
            let mut layer_collection = self.layer_collection.write().unwrap();
            layer_collection.load_styles(zoom, css_cache);
        }
    }

    pub fn get_hidpi_factor(&self) -> f64 {
        self.hidpi_factor
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.swap_chain_descriptor.width = width;
        self.swap_chain_descriptor.height = height;
        self.swap_chain = self.device.create_swap_chain(&self.surface, &self.swap_chain_descriptor);
        self.multisampled_framebuffer = Self::create_multisampled_framebuffer(&self.device, &self.swap_chain_descriptor, MSAA_SAMPLES);
        self.framebuffer = Self::create_framebuffer(&self.device, &self.swap_chain_descriptor);
        self.stencil = Self::create_stencil(&self.device, &self.swap_chain_descriptor);
    }

    fn update_uniforms<'a>(
        &mut self,
        encoder: &mut CommandEncoder,
        app_state: &AppState,
        layer_collection: &LayerCollection
    ) {
        Self::copy_uniform_buffers(
            encoder,
            &Self::create_uniform_buffers(
                &self.device,
                &app_state.screen,
                layer_collection
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

    fn create_multisampled_framebuffer(device: &Device, swap_chain_descriptor: &SwapChainDescriptor, sample_count: u32) -> wgpu::TextureView {
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

    fn create_framebuffer(device: &Device, swap_chain_descriptor: &SwapChainDescriptor) -> wgpu::TextureView {
        let texture_extent = wgpu::Extent3d {
            width: swap_chain_descriptor.width,
            height: swap_chain_descriptor.height,
            depth: 1,
        };
        let frame_descriptor = &wgpu::TextureDescriptor {
            size: texture_extent,
            array_layer_count: 1,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: swap_chain_descriptor.format,
            usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT | wgpu::TextureUsage::SAMPLED,
        };

        device.create_texture(frame_descriptor).create_default_view()
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
            sample_count: 1,
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
                app_state.tile_cache.request_tile(&tile_id, self.layer_collection.clone());
                
                let tile_cache = &mut app_state.tile_cache;
                if let Some(tile) = tile_cache.try_get_tile(&tile_id) {

                    let drawable_tile = DrawableTile::load_from_tile_id(
                        &self.device,
                        tile_id,
                        &tile,
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

        let mut layer_collection = self.layer_collection.write().unwrap();
        layer_collection.load_styles(app_state.zoom, &mut app_state.css_cache);
    }

    pub fn paint(&mut self, app_state: &mut AppState) {
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { todo: 0 });
        self.load_tiles(app_state);
        let layer_collection = {
            let lock = self.layer_collection.clone();
            let layer_collection = lock.read().unwrap();
            (*layer_collection).clone()
        };
        self.update_uniforms(&mut encoder, &app_state, &layer_collection);
        self.bind_group = Self::create_blend_bind_group(
            &self.device,
            &self.bind_group_layout,
            &self.uniform_buffer,
            &self.tile_transform_buffer,
            &self.framebuffer,
            &self.sampler
        );
        let num_tiles = self.loaded_tiles.len();
        if layer_collection.iter_layers().count() > 0 && num_tiles > 0 {
            let frame = self.swap_chain.get_next_texture();
            {
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                        attachment: if MSAA_SAMPLES > 1 { &self.multisampled_framebuffer } else { &frame.view },
                        resolve_target: if MSAA_SAMPLES > 1 { Some(&frame.view) } else { None },
                        load_op: wgpu::LoadOp::Clear,
                        store_op: wgpu::StoreOp::Store,
                        clear_color: wgpu::Color::WHITE,
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

                'outer: for (id, layer) in layer_collection.iter_layers().enumerate() {
                    // Check if we have anything to draw on a specific layer. If not, continue with the next layer.
                    let mut hit = false;
                    for drawable_tile in self.loaded_tiles.values_mut() {
                        if *layer {
                            if drawable_tile.layer_has_data(id as u32) {
                                hit = true;
                            }
                        }
                    }
                    if !hit {
                        continue 'outer;
                    }

                    render_pass.set_pipeline(&self.layer_render_pipeline);
                    render_pass.set_stencil_reference(id as u32);
                    render_pass.set_bind_group(0, &self.bind_group, &[]);

                    for (i, drawable_tile) in self.loaded_tiles.values_mut().enumerate() {
                        if *layer {
                            drawable_tile.paint(&mut render_pass, &layer_collection, i as u32, id as u32, false);
                        }
                    }

                    for (i, drawable_tile) in self.loaded_tiles.values_mut().enumerate() {
                        if *layer {
                            drawable_tile.paint(&mut render_pass, &layer_collection, i as u32, id as u32, true);
                        }
                    }
                }
            }
            self.device.get_queue().submit(&[encoder.finish()]);
        }
    }
}

// pub fn timestamp(old: std::time::Instant, string: &str) -> std::time::Instant {
//     log::debug!("{}: {}", string, old.elapsed().as_micros());
//     std::time::Instant::now()
// }