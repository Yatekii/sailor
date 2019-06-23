use wgpu::{
    winit::{
        EventsLoop,
    },
    ShaderModule,
    SwapChain,
    Device,
    BindGroup,
    RenderPipeline,
    Buffer,
    BufferUsage,
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

pub struct Painter {
    events_loop: EventsLoop,
    device: Device,
    swap_chain: SwapChain,
    bind_group: BindGroup,
    render_pipeline: RenderPipeline,
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    index_count: u32,
}

impl Painter {
    pub fn init() -> Self {
        let events_loop = EventsLoop::new();

        #[cfg(not(feature = "gl"))]
        let (_window, instance, size, surface) = {
            use wgpu::winit::Window;

            let instance = wgpu::Instance::new();

            let window = Window::new(&events_loop).unwrap();
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

        let device = adapter.request_device(&wgpu::DeviceDescriptor {
            extensions: wgpu::Extensions {
                anisotropic_filtering: false,
            },
            limits: wgpu::Limits::default(),
        });

        let (vs_module, fs_module) = Self::load_shader(&device).expect("Fatal Error. Unable to load shaders.");

        let bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { bindings: &[] });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            bindings: &[],
        });
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

        Self {
            events_loop,
            vertex_buffer: device.create_buffer_mapped(0, BufferUsage::VERTEX).fill_from_slice(&[] as &[Vertex]),
            index_buffer: device.create_buffer_mapped(0, BufferUsage::INDEX).fill_from_slice(&[] as &[u16]),
            device,
            swap_chain,
            bind_group,
            render_pipeline,
            index_count: 0,
        }
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

    pub fn update_view(&mut self) {
        let frame = self.swap_chain.get_next_texture();
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { todo: 0 });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                    attachment: &frame.view,
                    resolve_target: None,
                    load_op: wgpu::LoadOp::Clear,
                    store_op: wgpu::StoreOp::Store,
                    clear_color: wgpu::Color::GREEN,
                }],
                depth_stencil_attachment: None,
            });
            rpass.set_pipeline(&self.render_pipeline);
            rpass.set_bind_group(0, &self.bind_group, &[]);
            rpass.set_index_buffer(&self.index_buffer, 0);
            rpass.set_vertex_buffers(&[(&self.vertex_buffer, 0)]);
            rpass.draw_indexed(0 .. self.index_count, 0, 0 .. 1);
        }

        self.device.get_queue().submit(&[encoder.finish()]);
    }
}