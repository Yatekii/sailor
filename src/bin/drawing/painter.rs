use std::sync::Arc;
use std::time::Duration;

use util::StagingBelt;
use wgpu::*;
use winit::dpi::PhysicalSize;
use winit::window::Window;

use osm::math::Camera;

use super::layer::{FramePass, GpuTiming, Layer, LayerCtx, LayerStack, Selection, Spans, StatSink};
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
}

impl Painter {
    /// Initializes the entire draw machinery.
    pub async fn init(window: Arc<Window>, size: PhysicalSize<u32>) -> Self {
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

    /// Renders the map plus overlay layers into a fresh surface frame and hands it
    /// back so the app can composite UI on top, then call `present`. Returns `None`
    /// when the surface is unavailable.
    ///
    /// UI-agnostic: no app or map-data types here — the app owns the layers and the
    /// HUD, and passes only the camera, selection, and a stat sink.
    #[allow(clippy::too_many_arguments)]
    pub fn paint(
        &mut self,
        map: &mut dyn Layer,
        overlays: &mut LayerStack,
        camera: &Camera,
        selection: Option<Selection>,
        hover: Option<super::layer::hover::HoverInfo>,
        wind: super::layer::WindControls,
        stats: &mut dyn StatSink,
    ) -> Option<Frame> {
        // Read back last frame's GPU pass timings (non-blocking) and record them.
        if let Some(gt) = self.gpu_timing.as_mut()
            && let Some(raw) = gt.take(&self.device)
        {
            let p = gt.period;
            let poly = (raw[1].saturating_sub(raw[0])) as f32 * p;
            let text = (raw[3].saturating_sub(raw[2])) as f32 * p;
            stats.record("gpu.polygon_pass", Duration::from_nanos(poly as u64));
            stats.record("gpu.text_pass", Duration::from_nanos(text as u64));
        }
        // Only instrument the GPU this frame if last frame's readback is done,
        // so we never copy into a still-mapped buffer.
        let record_gpu = self.gpu_timing.as_ref().is_some_and(|g| !g.pending);

        let (wgpu::CurrentSurfaceTexture::Success(surface)
        | wgpu::CurrentSurfaceTexture::Suboptimal(surface)) = self.surface.get_current_texture()
        else {
            return None;
        };

        let mut spans: Spans = Vec::new();
        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("tile polygon encoder"),
            });

        {
            let mut ctx = LayerCtx {
                device: &self.device,
                queue: &self.queue,
                encoder: &mut encoder,
                screen: camera,
                selection,
                hover,
                resolution: (self.surface_config.width, self.surface_config.height),
                spans: &mut spans,
                time: Default::default(),
                wind,
            };
            map.update(&mut ctx);
            overlays.update_all(&mut ctx);
        }

        {
            let view = surface
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            let msaa = (CONFIG.renderer.msaa_samples > 1).then_some(&self.multisampled_framebuffer);

            let mut frame_pass = FramePass {
                encoder: &mut encoder,
                view: &view,
                msaa,
                depth_stencil: &self.stencil,
                screen: camera,
                gpu_timing: self.gpu_timing.as_ref(),
                record_gpu,
                spans: &mut spans,
            };
            map.paint(&mut frame_pass);
            overlays.paint_all(&mut frame_pass);
        }

        Some(Frame {
            surface,
            encoder,
            record_gpu,
            spans,
        })
    }

    /// Finishes a frame: resolves GPU timings, submits, presents, and records the
    /// remaining CPU spans. Call after the app has drawn its UI into the frame.
    pub fn present(&mut self, mut frame: Frame, stats: &mut dyn StatSink) {
        if frame.record_gpu
            && let Some(gt) = self.gpu_timing.as_ref()
        {
            gt.resolve(&mut frame.encoder);
        }

        let submit = web_time::Instant::now();
        self.staging_belt.finish();
        self.queue.submit([frame.encoder.finish()]);
        self.queue.present(frame.surface);
        if frame.record_gpu
            && let Some(gt) = self.gpu_timing.as_mut()
        {
            gt.map();
        }
        frame.spans.push(("cpu.submit", submit.elapsed()));

        for (name, dur) in frame.spans {
            stats.record(name, dur);
        }
    }
}

/// An in-flight surface frame: the layer stack has been recorded into `encoder`;
/// the app draws UI into it, then hands it to `Painter::present`.
pub struct Frame {
    pub surface: SurfaceTexture,
    pub encoder: CommandEncoder,
    record_gpu: bool,
    spans: Spans,
}

impl Frame {
    /// Records a named CPU span (e.g. the app's UI pass) for this frame.
    pub fn push_span(&mut self, name: &'static str, dur: Duration) {
        self.spans.push((name, dur));
    }
}
