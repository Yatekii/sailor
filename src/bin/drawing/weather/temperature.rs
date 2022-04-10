use std::num::NonZeroU32;

use crossbeam_channel::{unbounded, TryRecvError};
use notify::{event::ModifyKind, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use wgpu::{
    util::DeviceExt, BindGroup, BindGroupLayout, Device, RenderPipeline, Sampler, ShaderModule,
    Texture,
};
use wgpu::{
    BlendState, ColorTargetState, ColorWrites, FragmentState, FrontFace, MultisampleState,
    PolygonMode, PrimitiveState, PrimitiveTopology, SamplerBindingType, ShaderModuleDescriptor,
    TextureFormat, VertexState,
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
            device,
            &CONFIG.renderer.temperature.vertex_shader,
            &CONFIG.renderer.temperature.fragment_shader,
        )
        .expect("Fatal Error. Unable to load shaders.");

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        });

        let pipeline = Self::create_render_pipeline(
            device,
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
            border_color: None,
        });

        let width = 64 * 8;
        let height = 64 * 8;

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            // array_layer_count: 1,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST,
        });

        let bind_group = Self::create_bind_group(device, &bind_group_layout, &texture, &sampler);

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
            bind_group_layouts: &[bind_group_layout],
            push_constant_ranges: &[],
        });

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Temperature"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: vs_module,
                entry_point: "main",
                buffers: &[],
            },
            fragment: Some(FragmentState {
                module: fs_module,
                entry_point: "main",
                targets: &[ColorTargetState {
                    format: TextureFormat::Bgra8Unorm,
                    blend: Some(BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: ColorWrites::ALL,
                }],
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
            depth_stencil: None,
            multisample: MultisampleState {
                count: CONFIG.renderer.msaa_samples,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
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
            usage: wgpu::BufferUsages::COPY_SRC,
        });

        // Upload immediately
        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        encoder.copy_buffer_to_texture(
            wgpu::ImageCopyBuffer {
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: NonZeroU32::new(width * 4),
                    rows_per_image: NonZeroU32::new(height),
                },
                buffer,
            },
            wgpu::ImageCopyTexture {
                texture: &self.texture,
                mip_level: 0,
                aspect: wgpu::TextureAspect::All,
                origin: wgpu::Origin3d { x: 0, y: 0, z: 0 },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
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
        let vs_module = device.create_shader_module(&ShaderModuleDescriptor {
            label: Some("VertexShader"),
            source: vs_bytes,
        });

        let fragment_shader = std::fs::read_to_string(fragment_shader)?;
        let fs_bytes = load_glsl(&fragment_shader, ShaderStage::Fragment);
        let fs_module = device.create_shader_module(&ShaderModuleDescriptor {
            label: Some("FragmentShader"),
            source: fs_bytes,
        });

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

    pub fn paint(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Temperature"),
            color_attachments: &[wgpu::RenderPassColorAttachment {
                view,
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
