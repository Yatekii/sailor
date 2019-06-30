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
    drawable_layer::DrawableLayer,
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

use crate::app_state::AppState;

pub struct Painter {
    window: Window,
    device: Device,
    surface: Surface,
    swap_chain_descriptor: SwapChainDescriptor,
    swap_chain: SwapChain,
    render_pipeline: RenderPipeline,
    multisampled_framebuffer: TextureView,
    loaded_tiles: BTreeMap<TileId, DrawableTile>,
    available_layers: Vec<Option<DrawableLayer>>,
    bind_group_layout: BindGroupLayout,
    bind_group: BindGroup,
    vertex_shader: String,
    fragment_shader: String,
    rx: crossbeam_channel::Receiver<std::result::Result<notify::event::Event, notify::Error>>,
    _watcher: RecommendedWatcher,
}

impl Painter {
    /// Initializes the entire draw machinery.
    pub fn init(events_loop: &EventsLoop, width: u32, height: u32, app_state: &AppState) -> Self {
        #[cfg(not(feature = "gl"))]
        let (window, instance, size, surface) = {
            let instance = wgpu::Instance::new();

            let window = Window::new(&events_loop).unwrap();
            window.set_inner_size(LogicalSize { width: width as f64, height: height as f64 });
            let size = window
                .get_inner_size()
                .unwrap()
                .to_physical(window.get_hidpi_factor());

            let surface = instance.create_surface(&window);

            (window, instance, size, surface)
        };

        #[cfg(feature = "gl")]
        let (instance, size, surface) = {
            let wb = wgpu::winit::WindowBuilder::new()
                .with_dimensions(LogicalSize { width, height });
            let cb = wgpu::glutin::ContextBuilder::new().with_vsync(true);
            let context = wgpu::glutin::WindowedContext::new_windowed(wb, cb, &events_loop).unwrap();

            let size = context
                .window()
                .get_inner_size()
                .unwrap()
                .to_physical(context.window().get_hidpi_factor());

            let instance = wgpu::Instance::new(context);
            let surface = instance.get_surface();

            (instance, size, surface)
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

        let mut init_encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { todo: 0 });

        let (tx, rx) = unbounded();
        
        let mut watcher: RecommendedWatcher = match Watcher::new_immediate(tx) {
            Ok(watcher) => watcher,
            Err(err) => {
                log::info!("Failed to create a watcher for the vertex shader:");
                log::info!("{}", err);
                panic!("Unable to load a vertex shader.");
            },
        };

        let vertex_shader = "config/shader.vert".to_string();
        let fragment_shader = "config/shader.frag".to_string();

        match watcher.watch(&vertex_shader, RecursiveMode::Recursive) {
            Ok(_) => {},
            Err(err) => {
                log::info!("Failed to start watching {}:", &vertex_shader);
                log::info!("{}", err);
            },
        };

        match watcher.watch(&fragment_shader, RecursiveMode::Recursive) {
            Ok(_) => {},
            Err(err) => {
                log::info!("Failed to start watching {}:", &fragment_shader);
                log::info!("{}", err);
            },
        };

        let (vs_module, fs_module) = Self::load_shader(&device, &vertex_shader, &fragment_shader).expect("Fatal Error. Unable to load shaders.");

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            bindings: &[
                wgpu::BindGroupLayoutBinding {
                    binding: 0,
                    visibility: wgpu::ShaderStage::VERTEX,
                    ty: wgpu::BindingType::UniformBuffer,
                },
                wgpu::BindGroupLayoutBinding {
                    binding: 1,
                    visibility: wgpu::ShaderStage::FRAGMENT,
                    ty: wgpu::BindingType::SampledTexture,
                },
            ]
        });

        let available_layers = vec![None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None];

        let bind_group = Self::create_bind_group(
            &device,
            &mut init_encoder,
            &bind_group_layout,
            &Self::create_texture(&device, width, height),
            &app_state.screen,
            app_state.zoom,
            &available_layers,
        );

        let render_pipeline = Self::create_render_pipeline(&device, &bind_group_layout, &vs_module, &fs_module);

        let swap_chain_descriptor = wgpu::SwapChainDescriptor {
            usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8Unorm,
            width: size.width.round() as u32,
            height: size.height.round() as u32,
        };

        let multisampled_framebuffer = Self::create_multisampled_framebuffer(&device, &swap_chain_descriptor, 8);

        let swap_chain = device.create_swap_chain(
            &surface,
            &swap_chain_descriptor,
        );

        let init_command_buf = init_encoder.finish();
        device.get_queue().submit(&[init_command_buf]);

        Self {
            window,
            device,
            surface,
            swap_chain_descriptor,
            swap_chain,
            render_pipeline,
            multisampled_framebuffer,
            loaded_tiles: BTreeMap::new(),
            available_layers,
            bind_group_layout,
            bind_group,
            vertex_shader,
            fragment_shader,
            _watcher: watcher,
            rx,
        }
    }

    fn create_render_pipeline(
        device: &Device,
        bind_group_layout: &BindGroupLayout,
        vs_module: &ShaderModule,
        fs_module: &ShaderModule
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
                color_blend: wgpu::BlendDescriptor::REPLACE,
                alpha_blend: wgpu::BlendDescriptor::REPLACE,
                write_mask: wgpu::ColorWrite::ALL,
            }],
            depth_stencil_state: None,
            index_format: wgpu::IndexFormat::Uint32,
            vertex_buffers: &[wgpu::VertexBufferDescriptor {
                stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                step_mode: wgpu::InputStepMode::Vertex,
                attributes: &[
                    wgpu::VertexAttributeDescriptor {
                        format: wgpu::VertexFormat::Float2,
                        offset: 0,
                        shader_location: 0,
                    },
                    wgpu::VertexAttributeDescriptor {
                        format: wgpu::VertexFormat::Float2,
                        offset: 8,
                        shader_location: 1,
                    },
                    wgpu::VertexAttributeDescriptor {
                        format: wgpu::VertexFormat::Uint,
                        offset: 16,
                        shader_location: 2,
                    },
                ],
            }],
            sample_count: 8,
        })
    }

    /// Creates a new bind group containing all the relevant uniform buffers.
    fn create_uniform_buffers(device: &Device, screen: &Screen, z: f32, drawable_layers: &Vec<Option<DrawableLayer>>) -> Vec<(Buffer, usize)> {
        let canvas_size_len = 4 * 4;
        let canvas_size_buffer = device
            .create_buffer_mapped(
                canvas_size_len / 4,
                wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::TRANSFER_SRC,
            )
            .fill_from_slice(&[screen.width as f32, screen.height as f32, 0.0, 0.0]);
        let transform_len = 4 * 4 * 4;
        let transform_buffer = device
            .create_buffer_mapped(
                transform_len / 4,
                wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::TRANSFER_SRC,
            )
            .fill_from_slice(screen.global_to_screen(z).as_slice());

        let layer_data_len = drawable_layers.len() * 12 * 4;
        let layer_data_buffer = device
            .create_buffer_mapped(
                layer_data_len / 12 / 4,
                wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::TRANSFER_SRC,
            )
            .fill_from_slice(&drawable_layers.iter().map(|dl| if let Some(dl) = dl { dl.layer_data } else { Default::default() }).collect::<Vec<_>>().as_slice());

        vec![
            (canvas_size_buffer, canvas_size_len),
            (transform_buffer, transform_len),
            (layer_data_buffer, layer_data_len)
        ]
    }

    fn copy_uniform_buffers(device: &Device, encoder: &mut CommandEncoder, source: &Vec<(Buffer, usize)>) -> Buffer {
        let final_buffer = device
            .create_buffer_mapped::<u8>(
                Self::uniform_buffer_size() as usize,
                wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::TRANSFER_DST,
            )
            .fill_from_slice(&[0; Self::uniform_buffer_size() as usize]);

        let mut total_bytes = 0;
        for (buffer, len) in source {
            encoder.copy_buffer_to_buffer(
                &buffer,
                0,
                &final_buffer,
                total_bytes,
                *len as u64
            );
            total_bytes += *len as u64;
        }

        final_buffer
    }

    const fn uniform_buffer_size() -> u64 {
        4 * 4
      + 4 * 4 * 4
      + 12 * 4 * 30
    }

    pub fn create_bind_group(
        device: &Device,
        encoder: &mut CommandEncoder,
        bind_group_layout: &BindGroupLayout,
        texture_view: &TextureView,
        screen: &Screen,
        z: f32,
        layers: &Vec<Option<DrawableLayer>>
    ) -> BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: bind_group_layout,
            bindings: &[
                wgpu::Binding {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer {
                        buffer: &Self::copy_uniform_buffers(
                            &device,
                            encoder,
                            &Self::create_uniform_buffers(
                                &device,
                                &screen,
                                z,
                                layers
                            )
                        ),
                        range: 0 .. Self::uniform_buffer_size(),
                    },
                },
                wgpu::Binding {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(texture_view),
                },
            ],
        })
    }

    pub fn create_texture(device: &Device, width: u32, height: u32) -> TextureView {
        let texture_extent = wgpu::Extent3d {
            width,
            height,
            depth: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            size: texture_extent,
            array_layer_count: 1,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsage::SAMPLED | wgpu::TextureUsage::TRANSFER_DST,
        });
        texture.create_default_view()
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
                    self.render_pipeline = Self::create_render_pipeline(&self.device, &self.bind_group_layout, &vs_module, &fs_module);
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
            for tile in self.loaded_tiles.values_mut() {
                for drawable_layer in tile.layers.iter_mut() {
                    drawable_layer.load_style(zoom, css_cache);
                }
            }
        }
    }

    pub fn get_hidpi_factor(&self) -> f64 {
        self.window.get_hidpi_factor()
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.swap_chain_descriptor.width = width;
        self.swap_chain_descriptor.height = height;
        self.swap_chain = self.device.create_swap_chain(&self.surface, &self.swap_chain_descriptor);
        self.multisampled_framebuffer = Self::create_multisampled_framebuffer(&self.device, &self.swap_chain_descriptor, 8);
    }

    fn update_uniforms(&mut self, encoder: &mut CommandEncoder, app_state: &AppState) {
        self.bind_group = Self::create_bind_group(
            &self.device,
            encoder,
            &self.bind_group_layout,
            &Self::create_texture(&self.device, app_state.screen.width, app_state.screen.height),
            &app_state.screen,
            app_state.zoom,
            &self.available_layers
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
            usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
        };

        device.create_texture(multisampled_frame_descriptor).create_default_view()
    }

    fn load_tiles(&mut self, app_state: &mut AppState, encoder: &mut CommandEncoder) {
        let tile_field = app_state.screen.get_tile_boundaries_for_zoom_level(app_state.zoom);
        let mut new_loaded_tiles = BTreeMap::new();

        app_state.tile_cache.fetch_tiles();

        for tile_id in tile_field.iter() {
            if !self.loaded_tiles.contains_key(&tile_id) {
                
                app_state.tile_cache.request_tile(&tile_id);
                
                let tile_cache = &mut app_state.tile_cache;
                if let Some(tile) = tile_cache.try_get_tile(&tile_id) {
                    let drawable_tile = DrawableTile::load_from_tile_id(
                        &self.device,
                        encoder,
                        tile_id,
                        &tile,
                        app_state.zoom,
                        &app_state.screen,
                        &mut app_state.css_cache
                    );
                    for layer in &drawable_tile.layers {
                        self.available_layers[layer.layer.id as usize] = Some(layer.clone());
                    }
                    new_loaded_tiles.insert(
                        tile_id.clone(),
                        drawable_tile
                    );
                } else {
                    log::trace!("Could not read tile {} from cache.", tile_id);
                }
            } else {
                if let Some(v) = self.loaded_tiles.remove(&tile_id) {
                    new_loaded_tiles.insert(tile_id, v);
                }
            }
        }

        self.update_uniforms(encoder, &app_state);

        self.loaded_tiles = new_loaded_tiles;
    }

    pub fn paint(&mut self, app_state: &mut AppState) {
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { todo: 0 });
        self.load_tiles(app_state, &mut encoder);
        // let t = std::time::Instant::now();
        self.update_uniforms(&mut encoder, &app_state);
        // dbg!(t.elapsed().as_micros());
        let frame = self.swap_chain.get_next_texture();
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                    attachment: &self.multisampled_framebuffer,
                    resolve_target: Some(&frame.view),
                    load_op: wgpu::LoadOp::Clear,
                    store_op: wgpu::StoreOp::Store,
                    clear_color: wgpu::Color::GREEN,
                }],
                depth_stencil_attachment: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.bind_group, &[]);

            for drawable_tile in self.loaded_tiles.values_mut() {
                for layer in &self.available_layers {
                    if let Some(layer) = layer {
                        drawable_tile.paint(&mut render_pass, layer);
                    }
                }
            }
        }

        self.device.get_queue().submit(&[encoder.finish()]);
    }
}