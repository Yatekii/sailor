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
use winit::{
    window::Window,
    event_loop::EventLoop,
    dpi::{
        LogicalSize,
    },
};
use wgpu::{
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
use wgpu_glyph::{
    Section,
    GlyphBrushBuilder,
    GlyphBrush,
};
use osm::*;

use crate::drawing::helpers::{
    ShaderStage,
    load_glsl,
};
use crate::app_state::AppState;

use crate::config::CONFIG;

pub struct Painter {
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
    bind_group_layout: BindGroupLayout,
    bind_group: BindGroup,
    rx: crossbeam_channel::Receiver<std::result::Result<notify::event::Event, notify::Error>>,
    _watcher: RecommendedWatcher,
    glyph_brush: GlyphBrush<'static, ()>,
    temperature: crate::drawing::weather::Temperature,
}

impl Painter {
    /// Initializes the entire draw machinery.
    pub fn init(event_loop: &EventLoop<()>, width: u32, height: u32, app_state: &AppState) -> Self {
        let (window, instance, size, surface, factor) = {
            use raw_window_handle::HasRawWindowHandle as _;

            let instance = wgpu::Instance::new();

            let window = Window::new(&event_loop).unwrap();
            window.set_inner_size(LogicalSize { width: width as f64, height: height as f64 });
            let factor = window.hidpi_factor();
            let size = window
                .inner_size()
                .to_physical(factor);

            let surface = instance.create_surface(window.raw_window_handle());

            (window, instance, size, surface, factor)
        };

        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
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
                    ty: wgpu::BindingType::UniformBuffer {
                        dynamic: false,
                    },
                },
                wgpu::BindGroupLayoutBinding {
                    binding: 1,
                    visibility: wgpu::ShaderStage::VERTEX,
                    ty: wgpu::BindingType::UniformBuffer {
                        dynamic: false,
                    },
                },
            ]
        });

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
            std::iter::empty::<&VisibleTile>()
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

        let font: &[u8] = include_bytes!("../../../config/Ruda-Bold.ttf");
        let mut glyph_brush = GlyphBrushBuilder::using_font_bytes(font)
            .build(&mut device, wgpu::TextureFormat::Bgra8Unorm);

        let mut temperature = crate::drawing::weather::Temperature::init(&mut device);

        let init_command_buf = init_encoder.finish();
        device.get_queue().submit(&[init_command_buf]);

        let width = 64 * 8;
        let height = 64 * 8;

        temperature.generate_texture(&mut device, width, height);

        Self {
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
            bind_group_layout,
            bind_group,
            _watcher: watcher,
            rx,
            glyph_brush,
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
            vertex_stage: wgpu::ProgrammableStageDescriptor {
                module: &vs_module,
                entry_point: "main",
            },
            fragment_stage: Some(wgpu::ProgrammableStageDescriptor {
                module: &fs_module,
                entry_point: "main",
            }),
            rasterization_state: Some(wgpu::RasterizationStateDescriptor {
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: wgpu::CullMode::None,
                depth_bias: 0,
                depth_bias_slope_scale: 0.0,
                depth_bias_clamp: 0.0,
            }),
            primitive_topology: wgpu::PrimitiveTopology::TriangleList,
            color_states: &[wgpu::ColorStateDescriptor {
                format: wgpu::TextureFormat::Bgra8Unorm,
                color_blend,
                alpha_blend,
                write_mask: wgpu::ColorWrite::ALL,
            }],
            depth_stencil_state: Some(DepthStencilStateDescriptor {
                format: wgpu::TextureFormat::Depth24PlusStencil8,
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
            sample_mask: !0,
            alpha_to_coverage_enabled: false,
        })
    }

    /// Creates a new bind group containing all the relevant uniform buffers.
    fn create_uniform_buffers(device: &Device, screen: &Screen, feature_collection: &FeatureCollection) -> Vec<(Buffer, usize)> {
        let canvas_size_len = 4 * 4;
        let canvas_size_buffer = device
            .create_buffer_mapped(
                canvas_size_len / 4,
                wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_SRC,
            )
            .fill_from_slice(&[screen.width as f32, screen.height as f32, 0.0, 0.0]);

        let buffer = feature_collection.assemble_style_buffer();
        let layer_data_len = buffer.len() * 12 * 4;
        let layer_data_buffer = device
            .create_buffer_mapped(
                layer_data_len / 12 / 4,
                wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_SRC,
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
                wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
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
        visible_tiles: impl Iterator<Item=&'a VisibleTile>
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
            device
            .create_buffer_mapped::<f32>(
                tile_data_buffer_byte_size,
                wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
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
            app_state.visible_tiles().values()
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
            format: wgpu::TextureFormat::Depth24PlusStencil8,
            usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT | wgpu::TextureUsage::SAMPLED,
        };

        device.create_texture(frame_descriptor).create_default_view()
    }

    pub fn paint(&mut self, hud: &mut super::ui::HUD, app_state: &mut AppState) {
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { todo: 0 });

        let feature_collection = app_state.feature_collection().read().unwrap().clone();
        self.update_uniforms(&mut encoder, &app_state, &feature_collection);
        self.bind_group = Self::create_blend_bind_group(
            &self.device,
            &self.bind_group_layout,
            &self.uniform_buffer,
            &self.tile_transform_buffer
        );
        let num_tiles = app_state.visible_tiles().len();
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
                for (i, vt) in app_state.visible_tiles().values().enumerate() {
                    if !vt.is_loaded_to_gpu() {
                        vt.load_to_gpu(&self.device);
                    }
                    let tile_id = vt.tile_id();
                    let matrix = app_state.screen.tile_to_global_space(
                        app_state.zoom,
                        &tile_id
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
                        &(tile_id + TileId::new(tile_id.z, 1, 1))
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

                    vt.paint(&mut render_pass, &self.blend_pipeline, &feature_collection, i as u32);
                }
            }

            for (i, vt) in app_state.visible_tiles().values().enumerate() {
                vt.queue_text(
                    &mut self.glyph_brush,
                    &app_state.screen,
                    app_state.zoom
                );
            }

            let _ = self.glyph_brush.draw_queued(
                &mut self.device,
                &mut encoder,
                &frame.view,
                app_state.screen.width,
                app_state.screen.height,
            );

            // self.temperature.paint(&mut encoder, &frame.view);

            hud.paint(
                app_state,
                &self.window,
                &mut self.device,
                &mut encoder,
                &frame.view,
            );
            self.device.get_queue().submit(&[encoder.finish()]);
        }
    }
}