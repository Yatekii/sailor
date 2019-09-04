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
    ShaderModule,
    Device,
    BindGroupLayout,
    BindGroup,
    RenderPipeline,
    Texture,
    Sampler,
};

use crate::drawing::helpers::{
    ShaderStage,
    load_glsl,
};

use crate::config::CONFIG;

pub struct Temperature {
    pipeline: RenderPipeline,
    bind_group_layout: BindGroupLayout,
    bind_group: BindGroup,
    rx: crossbeam_channel::Receiver<std::result::Result<notify::event::Event, notify::Error>>,
    _watcher: RecommendedWatcher,
    texture: Texture,
    sampler: Sampler,
}

impl Temperature {
    pub fn init(device: &mut wgpu::Device) -> Self {

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
            &device, &CONFIG.renderer.temperature.vertex_shader,
            &CONFIG.renderer.temperature.fragment_shader
        ).expect("Fatal Error. Unable to load shaders.");
        
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            bindings: &[
                wgpu::BindGroupLayoutBinding {
                    binding: 0,
                    visibility: wgpu::ShaderStage::FRAGMENT,
                    ty: wgpu::BindingType::SampledTexture {
                        multisampled: false,
                        dimension: wgpu::TextureViewDimension::D2,
                    },
                },
                wgpu::BindGroupLayoutBinding {
                    binding: 1,
                    visibility: wgpu::ShaderStage::FRAGMENT,
                    ty: wgpu::BindingType::Sampler,
                },
            ]
        });

        let pipeline = Self::create_render_pipeline(
            &device,
            &bind_group_layout,
            &layer_vs_module,
            &layer_fs_module
        );

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            lod_min_clamp: -100.0,
            lod_max_clamp: 100.0,
            compare_function: wgpu::CompareFunction::Always,
        });

        let width = 64 * 8;
        let height = 64 * 8;

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            size: wgpu::Extent3d { width, height, depth: 1 },
            array_layer_count: 1,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Float,
            usage: wgpu::TextureUsage::SAMPLED
                 | wgpu::TextureUsage::OUTPUT_ATTACHMENT
                 | wgpu::TextureUsage::COPY_DST,
        });

        let bind_group = Self::create_bind_group(
            &device,
            &bind_group_layout,
            &texture,
            &sampler,
        );

        let init_command_buf = init_encoder.finish();
        device.get_queue().submit(&[init_command_buf]);

        Self {
            bind_group_layout,
            bind_group,
            _watcher: watcher,
            rx,
            pipeline,
            sampler,
            texture,
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
            depth_stencil_state: None,
            index_format: wgpu::IndexFormat::Uint32,
            vertex_buffers: &[],
            sample_count: CONFIG.renderer.msaa_samples,
            sample_mask: !0,
            alpha_to_coverage_enabled: false,
        })
    }

    pub fn create_bind_group(
        device: &Device,
        bind_group_layout: &BindGroupLayout,
        texture: &Texture,
        sampler: &Sampler,
    ) -> BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: bind_group_layout,
            bindings: &[
                wgpu::Binding {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture.create_default_view()),
                },
                wgpu::Binding {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }

    pub fn generate_texture(
        &mut self,
        device: &mut wgpu::Device,
        width: u32,
        height: u32
    ) {
        // Generate data.
        let mut data = Vec::with_capacity((width * height) as usize);
        for y in 0..width {
            for x in 0..height {
                data.push(f32::sin(x as f32 * 0.5) * f32::cos(y as f32 * 0.5));
            }
        }

        // Place in wgpu buffer
        let buffer = device.create_buffer_mapped(
            (width * height) as usize,
            wgpu::BufferUsage::COPY_SRC,
        )
        .fill_from_slice(data.as_slice());

        // Upload immediately
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            todo: 0,
        });

        encoder.copy_buffer_to_texture(
            wgpu::BufferCopyView {
                buffer: &buffer,
                offset: 0,
                row_pitch: width * 4,
                image_height: height,
            },
            wgpu::TextureCopyView {
                texture: &self.texture,
                mip_level: 0,
                array_layer: 0,
                origin: wgpu::Origin3d {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth: 1,
            },
        );

        device
            .get_queue()
            .submit(&[encoder.finish()]);
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
    pub fn update_shader(&mut self, device: &Device) -> bool {
        match self.rx.try_recv() {
            Ok(Ok(notify::event::Event {
                kind: EventKind::Modify(ModifyKind::Data(_)),
                ..
            })) => {
                if let Ok((vs_module, fs_module)) = Self::load_shader(
                    device,
                    &CONFIG.renderer.temperature.vertex_shader,
                    &CONFIG.renderer.temperature.fragment_shader
                ) {
                    self.pipeline = Self::create_render_pipeline(
                        device,
                        &self.bind_group_layout,
                        &vs_module,
                        &fs_module
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

    pub fn paint(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                attachment: view,
                resolve_target: None,
                load_op: wgpu::LoadOp::Load,
                store_op: wgpu::StoreOp::Store,
                clear_color: wgpu::Color {
                    r: 0.1,
                    g: 0.2,
                    b: 0.3,
                    a: 1.0,
                },
            }],
            depth_stencil_attachment: None,
        });
        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &self.bind_group, &[]);
        rpass.draw(0 .. 6, 0 .. 1);
    }
}