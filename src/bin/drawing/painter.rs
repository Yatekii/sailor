use std::sync::Arc;
use std::time::Duration;

use util::StagingBelt;
use wgpu::*;
use winit::dpi::PhysicalSize;
use winit::window::Window;

use super::layer::map::MapLayer;
use super::layer::{FramePass, GpuTiming, LayerCtx, LayerStack, Spans};
use crate::app_state::AppState;
use crate::config::CONFIG;

pub struct Painter {
    pub window: Arc<Window>,
    hidpi_factor: f64,
    pub device: Device,
    pub queue: Queue,
    surface: Surface<'static>,
    staging_belt: StagingBelt,
    pub surface_config: SurfaceConfiguration,
    multisampled_framebuffer: TextureView,
    stencil: TextureView,
    gpu_timing: Option<GpuTiming>,
    stack: LayerStack,
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

        // Opt into GPU timestamps when the adapter supports them; otherwise the
        // per-pass histograms simply won't appear.
        let timestamps_supported = adapter.features().contains(Features::TIMESTAMP_QUERY);
        let mut required_features = Features::DEPTH32FLOAT_STENCIL8;
        if timestamps_supported {
            required_features |= Features::TIMESTAMP_QUERY;
        }

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Main Device"),
                required_features,
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

        let staging_belt = wgpu::util::StagingBelt::new(device.clone(), 1024);

        let gpu_timing =
            timestamps_supported.then(|| GpuTiming::new(&device, queue.get_timestamp_period()));

        let mut stack = LayerStack::new();
        stack.push(Box::new(MapLayer::new(&device, &queue, app_state)));

        Self {
            window,
            hidpi_factor: factor,
            device,
            queue,
            surface,
            staging_belt,
            surface_config,
            multisampled_framebuffer,
            stencil,
            gpu_timing,
            stack,
        }
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
            usage: TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[TextureFormat::Depth32FloatStencil8],
        };

        device
            .create_texture(frame_descriptor)
            .create_view(&wgpu::TextureViewDescriptor::default())
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

    pub fn paint(&mut self, hud: &mut super::ui::Hud, app_state: &mut AppState) {
        // Read back last frame's GPU pass timings (non-blocking) and record them.
        if let Some(gt) = self.gpu_timing.as_mut() {
            if let Some(raw) = gt.take(&self.device) {
                let p = gt.period;
                let poly = (raw[1].saturating_sub(raw[0])) as f32 * p;
                let text = (raw[3].saturating_sub(raw[2])) as f32 * p;
                app_state
                    .stats
                    .record("gpu.polygon_pass", Duration::from_nanos(poly as u64));
                app_state
                    .stats
                    .record("gpu.text_pass", Duration::from_nanos(text as u64));
            }
        }
        // Only instrument the GPU this frame if last frame's readback is done,
        // so we never copy into a still-mapped buffer.
        let record_gpu = self.gpu_timing.as_ref().is_some_and(|g| !g.pending);

        let mut spans: Spans = Vec::new();
        macro_rules! span {
            ($name:expr, $body:expr) => {{
                let __t = web_time::Instant::now();
                let __r = $body;
                spans.push(($name, __t.elapsed()));
                __r
            }};
        }

        // Nothing to draw until at least one tile's features have loaded; hold the
        // frame (and the HUD) until then, matching the pre-layer behaviour.
        let has_features = !app_state
            .feature_collection()
            .read()
            .unwrap()
            .features()
            .is_empty();

        if has_features
            && let wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) = self.surface.get_current_texture()
        {
            let mut encoder = self
                .device
                .create_command_encoder(&CommandEncoderDescriptor {
                    label: Some("tile polygon encoder"),
                });

            let mut ctx = LayerCtx {
                device: &self.device,
                queue: &self.queue,
                encoder: &mut encoder,
                app_state: &mut *app_state,
                resolution: (self.surface_config.width, self.surface_config.height),
                spans: &mut spans,
                time: Default::default(),
            };
            self.stack.update_all(&mut ctx);

            let view = frame
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            let msaa = (CONFIG.renderer.msaa_samples > 1).then_some(&self.multisampled_framebuffer);

            let mut frame_pass = FramePass {
                encoder: &mut encoder,
                view: &view,
                msaa,
                depth_stencil: &self.stencil,
                app_state: &*app_state,
                gpu_timing: self.gpu_timing.as_ref(),
                record_gpu,
                spans: &mut spans,
            };
            self.stack.paint_all(&mut frame_pass);

            span!("cpu.hud", {
                hud.paint(
                    app_state,
                    &self.window,
                    &self.device,
                    &self.queue,
                    &mut encoder,
                    &frame,
                );
            });

            if record_gpu {
                if let Some(gt) = self.gpu_timing.as_ref() {
                    gt.resolve(&mut encoder);
                }
            }

            span!("cpu.submit", {
                self.staging_belt.finish();
                self.queue.submit([encoder.finish()]);
                self.queue.present(frame);
                if record_gpu {
                    if let Some(gt) = self.gpu_timing.as_mut() {
                        gt.map();
                    }
                }
            });
        }

        for (name, dur) in spans {
            app_state.stats.record(name, dur);
        }
    }
}
