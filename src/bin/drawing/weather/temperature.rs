use crossbeam_channel::{unbounded, TryRecvError};
use notify::{event::ModifyKind, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use wgpu::{
    util::DeviceExt, BindGroup, BindGroupLayout, Device, RenderPipeline, Sampler, ShaderModule,
    Texture,
};

use crate::drawing::helpers::{load_glsl, ShaderStage};

use crate::config::CONFIG;
use osm::as_byte_slice;

pub struct Temperature {
    pipeline: RenderPipeline,
    bind_group_layout: BindGroupLayout,
    _bind_group: BindGroup,
    rx: crossbeam_channel::Receiver<std::result::Result<notify::event::Event, notify::Error>>,
    _watcher: RecommendedWatcher,
    texture: Texture,
}

impl Temperature {
    pub fn init(device: &mut wgpu::Device, queue: &mut wgpu::Queue) -> Self {
        let init_encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        let (tx, rx) = unbounded();

        let mut watcher: RecommendedWatcher =
            match Watcher::new_immediate(move |res| tx.send(res).unwrap()) {
                Ok(watcher) => watcher,
                Err(err) => {
                    log::info!("Failed to create a watcher for the vertex shader:");
                    log::info!("{}", err);
                    panic!("Unable to load a vertex shader.");
                }
            };

        match watcher.watch(&CONFIG.renderer.vertex_shader, RecursiveMode::Recursive) {
            Ok(_) => {}
            Err(err) => {
                log::info!(
                    "Failed to start watching {}:",
                    &CONFIG.renderer.vertex_shader
                );
                log::info!("{}", err);
            }
        };

        match watcher.watch(&CONFIG.renderer.fragment_shader, RecursiveMode::Recursive) {
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
            &CONFIG.renderer.temperature.vertex_shader,
            &CONFIG.renderer.temperature.fragment_shader,
        )
        .expect("Fatal Error. Unable to load shaders.");

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStage::FRAGMENT,
                    ty: wgpu::BindingType::SampledTexture {
                        multisampled: false,
                        dimension: wgpu::TextureViewDimension::D2,
                        component_type: wgpu::TextureComponentType::Float,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStage::FRAGMENT,
                    ty: wgpu::BindingType::Sampler { comparison: true },
                    count: None,
                },
            ],
        });

        let pipeline = Self::create_render_pipeline(
            &device,
            &bind_group_layout,
            &layer_vs_module,
            &layer_fs_module,
        );

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: None,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            lod_min_clamp: -100.0,
            lod_max_clamp: 100.0,
            compare: Some(wgpu::CompareFunction::Always),
            anisotropy_clamp: None,
        });

        let width = 64 * 8;
        let height = 64 * 8;

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width,
                height,
                depth: 1,
            },
            // array_layer_count: 1,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Float,
            usage: wgpu::TextureUsage::SAMPLED
                | wgpu::TextureUsage::OUTPUT_ATTACHMENT
                | wgpu::TextureUsage::COPY_DST,
        });

        let bind_group = Self::create_bind_group(&device, &bind_group_layout, &texture, &sampler);

        let init_command_buf = init_encoder.finish();
        queue.submit(vec![init_command_buf]);

        Self {
            bind_group_layout,
            _bind_group: bind_group,
            _watcher: watcher,
            rx,
            pipeline,
            texture,
        }
    }

    fn create_render_pipeline(
        device: &Device,
        bind_group_layout: &BindGroupLayout,
        vs_module: &ShaderModule,
        fs_module: &ShaderModule,
    ) -> RenderPipeline {
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
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
                clamp_depth: false,
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
            vertex_state: wgpu::VertexStateDescriptor {
                index_format: wgpu::IndexFormat::Uint32,
                vertex_buffers: &[],
            },
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
            label: None,
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(
                        &texture.create_view(&wgpu::TextureViewDescriptor::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }

    pub fn generate_texture(
        &mut self,
        device: &mut wgpu::Device,
        queue: &mut wgpu::Queue,
        width: u32,
        height: u32,
    ) {
        // Generate data.
        let mut data = Vec::with_capacity((width * height) as usize);
        for y in 0..width {
            for x in 0..height {
                data.push(f32::sin(x as f32 * 0.5) * f32::cos(y as f32 * 0.5));
            }
        }

        // Place in wgpu buffer
        let buffer = &device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: as_byte_slice(data.as_slice()),
            usage: wgpu::BufferUsage::COPY_SRC,
        });

        // Upload immediately
        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        encoder.copy_buffer_to_texture(
            wgpu::BufferCopyView {
                layout: wgpu::TextureDataLayout {
                    offset: 0,
                    bytes_per_row: width * 4,
                    rows_per_image: height,
                },
                buffer,
            },
            wgpu::TextureCopyView {
                texture: &self.texture,
                mip_level: 0,
                // array_layer: 0,
                origin: wgpu::Origin3d { x: 0, y: 0, z: 0 },
            },
            wgpu::Extent3d {
                width,
                height,
                depth: 1,
            },
        );

        queue.submit(vec![encoder.finish()]);
    }

    /// Loads a shader module from a GLSL vertex and fragment shader each.
    fn load_shader(
        device: &Device,
        vertex_shader: &str,
        fragment_shader: &str,
    ) -> Result<(ShaderModule, ShaderModule), std::io::Error> {
        let vertex_shader = std::fs::read_to_string(vertex_shader)?;
        let vs_bytes = load_glsl(&vertex_shader, ShaderStage::Vertex);
        let vs_module = device.create_shader_module(vs_bytes);

        let fragment_shader = std::fs::read_to_string(fragment_shader)?;
        let fs_bytes = load_glsl(&fragment_shader, ShaderStage::Fragment);
        let fs_module = device.create_shader_module(fs_bytes);

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
                    &CONFIG.renderer.temperature.fragment_shader,
                ) {
                    self.pipeline = Self::create_render_pipeline(
                        device,
                        &self.bind_group_layout,
                        &vs_module,
                        &fs_module,
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

    pub fn _paint(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                attachment: view,
                resolve_target: None,
                ops: wgpu::Operations::<wgpu::Color> {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.1,
                        g: 0.2,
                        b: 0.3,
                        a: 1.0,
                    }),
                    store: true,
                },
            }],
            depth_stencil_attachment: None,
        });
        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &self._bind_group, &[]);
        rpass.draw(0..6, 0..1);
    }
}
