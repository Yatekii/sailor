use lyon::math::Point;
use crate::vector_tile::math::TileId;
use crate::drawing::{
    drawable_tile::DrawableTile,
    drawable_layer::DrawableLayer,
};
use std::collections::HashMap;

use wgpu::{
    winit::{
        EventsLoop,
        dpi::{
            LogicalSize,
        },
    },
    ShaderModule,
    SwapChain,
    Device,
    BindGroupLayout,
    BindGroup,
    RenderPipeline,
    Buffer,
    BufferUsage,
};

use crate::vector_tile::{
    math,
    cache::{
        Tile,
        TileCache,
    },
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
    device: Device,
    swap_chain: SwapChain,
    render_pipeline: RenderPipeline,
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    index_count: u32,
    loaded_tiles: HashMap<TileId, DrawableTile>,
    bind_group_layout: BindGroupLayout,
    uniform_buffer: Buffer,
}

impl Painter {
    /// Initializes the entire draw machinery.
    pub fn init(events_loop: &EventsLoop) -> Self {
        #[cfg(not(feature = "gl"))]
        let (_window, instance, size, surface) = {
            use wgpu::winit::Window;

            let instance = wgpu::Instance::new();

            let window = Window::new(&events_loop).unwrap();
            //window.set_inner_size(LogicalSize { width: 600.0, height: 600.0 });
            let size = window
                .get_inner_size()
                .unwrap()
                .to_physical(window.get_hidpi_factor());

            let surface = instance.create_surface(&window);

            (window, instance, size, surface)
        };

        #[cfg(feature = "gl")]
        let (instance, size, surface) = {
            let wb = wgpu::winit::WindowBuilder::new();
                //.with_dimensions(LogicalSize { width: 600.0, height: 600.0 });
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

        let mut init_encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { todo: 0 });

        let (vs_module, fs_module) = Self::load_shader(&device).expect("Fatal Error. Unable to load shaders.");

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            bindings: &[
                wgpu::BindGroupLayoutBinding {
                    binding: 0,
                    visibility: wgpu::ShaderStage::VERTEX,
                    ty: wgpu::BindingType::UniformBuffer,
                },
            ]
        });

        let uniform_buffer = Self::create_uniform_buffer(&device, &Point::new(1.0, 0.0));

        let bind_group = Self::create_bind_group(&device, &bind_group_layout, &uniform_buffer);

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            bind_group_layouts: &[&bind_group_layout],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
            index_format: wgpu::IndexFormat::Uint16,
            vertex_buffers: &[wgpu::VertexBufferDescriptor {
                stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                step_mode: wgpu::InputStepMode::Vertex,
                attributes: &[
                    wgpu::VertexAttributeDescriptor {
                        format: wgpu::VertexFormat::Float2,
                        offset: 0,
                        shader_location: 0,
                    },
                ],
            }],
            sample_count: 1,
        });

        let swap_chain = device.create_swap_chain(
            &surface,
            &wgpu::SwapChainDescriptor {
                usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
                format: wgpu::TextureFormat::Bgra8Unorm,
                width: size.width.round() as u32,
                height: size.height.round() as u32,
            },
        );

        let vertex_buffer = device
            .create_buffer_mapped(0, BufferUsage::VERTEX)
            .fill_from_slice(&[] as &[Vertex]);

        let index_buffer = device
            .create_buffer_mapped(0, BufferUsage::INDEX)
            .fill_from_slice(&[] as &[u16]);

        let init_command_buf = init_encoder.finish();
        device.get_queue().submit(&[init_command_buf]);

        Self {
            vertex_buffer,
            index_buffer,
            device,
            swap_chain,
            render_pipeline,
            index_count: 0,
            loaded_tiles: HashMap::new(),
            bind_group_layout,
            uniform_buffer,
        }
    }

    /// Creates a new bind group containing all the relevant uniform buffers.
    fn create_uniform_buffer(device: &Device, pan: &Point) -> Buffer {
        device
            .create_buffer_mapped(
                2,
                wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::TRANSFER_DST,
            )
            .fill_from_slice(&[pan.x, pan.y])
    }

    fn create_bind_group(device: &Device, bind_group_layout: &BindGroupLayout, uniform_buffer: &Buffer) -> BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: bind_group_layout,
            bindings: &[
                wgpu::Binding {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer {
                        buffer: &uniform_buffer,
                        range: 0 .. 8,
                    },
                },
            ],
        })
    }

    /// Loads a shader module from a GLSL vertex and fragment shader each.
    fn load_shader(device: &Device) -> Result<(ShaderModule, ShaderModule), std::io::Error> {
        let vs_bytes = load_glsl(&std::fs::read_to_string("src/drawing/shader.vert")?, ShaderStage::Vertex);
        let vs_module = device.create_shader_module(vs_bytes.as_slice());
        let fs_bytes = load_glsl(&std::fs::read_to_string("src/drawing/shader.frag")?, ShaderStage::Fragment);
        let fs_module = device.create_shader_module(fs_bytes.as_slice());
        Ok((vs_module, fs_module))
    }

    /// Reloads the shader if the file watcher has detected any change to the shader files.
    pub fn update_shader(&mut self) {

    }

    /// Sets the buffers to be drawn.
    pub fn set_buffers(&mut self, vertices: &Vec<Vertex>, indices: &Vec<u16>) {
        self.vertex_buffer = self.device
            .create_buffer_mapped(vertices.len(), wgpu::BufferUsage::VERTEX)
            .fill_from_slice(&vertices);

        self.index_buffer = self.device
            .create_buffer_mapped(indices.len(), wgpu::BufferUsage::INDEX)
            .fill_from_slice(&indices);

        self.index_count = indices.len() as u32;
    }

    pub fn load_tiles(&mut self, app_state: &mut AppState) {
        let tile_field = app_state.screen.get_tile_boundaries_for_zoom_level(app_state.zoom);

        for tile_id in tile_field.iter() {
            if !self.loaded_tiles.contains_key(&tile_id) {
                app_state.tile_cache.fetch_tile(&tile_id);
                let css_cache = &app_state.css_cache;
                if let Some(tile) = app_state.tile_cache.try_get_tile(&tile_id) {
                    let mut vertex_count = 0;
                    self.loaded_tiles.insert(tile_id, DrawableTile {
                        vertex_buffer: self.device
                            .create_buffer_mapped(tile.layers[0].mesh.vertices.len(), wgpu::BufferUsage::VERTEX)
                            .fill_from_slice(&tile.layers[0].mesh.vertices),
                        index_buffer: self.device
                            .create_buffer_mapped(tile.layers[0].mesh.indices.len(), wgpu::BufferUsage::INDEX)
                            .fill_from_slice(&tile.layers[0].mesh.indices),
                        index_count: tile.layers[0].mesh.indices.len() as u32,
                        layers: tile.layers
                            .iter()
                            .map(|l| {
                                let vc = l.mesh.indices.len() as u32;
                                vertex_count += vc;
                                DrawableLayer::from_layer(vertex_count - vc, vertex_count, l, css_cache)
                            })
                            .collect(),
                    });
                } else {
                    log::error!("Could not read tile from cache. This is a bug. Please report it!");
                }
            }
        }
    }

    pub fn paint(&mut self, app_state: &AppState) {
        let frame = self.swap_chain.get_next_texture();
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { todo: 0 });
        encoder.copy_buffer_to_buffer(
            &Self::create_uniform_buffer(
                &self.device,
                &app_state.screen.center
            ),
            0,
            &self.uniform_buffer,
            0,
            8
        );
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                    attachment: &frame.view,
                    resolve_target: None,
                    load_op: wgpu::LoadOp::Clear,
                    store_op: wgpu::StoreOp::Store,
                    clear_color: wgpu::Color::GREEN,
                }],
                depth_stencil_attachment: None,
            });

            let bind_group = Self::create_bind_group(
                &self.device,
                &self.bind_group_layout,
                &Self::create_uniform_buffer(&self.device, &app_state.screen.center)
            );

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &bind_group, &[]);

            for drawable_tile in self.loaded_tiles.values_mut() {
                drawable_tile.paint(&mut render_pass);
            }
        }

        self.device.get_queue().submit(&[encoder.finish()]);
    }
}