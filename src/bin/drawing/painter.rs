use winit::{
    window::Window,
    event_loop::EventLoop,
    dpi::{
        LogicalSize,
    },
};
use wgpu::*;

use crate::app_state::AppState;

use crate::config::CONFIG;

pub struct Painter {
    pub window: Window,
    hidpi_factor: f64,
    pub device: Device,
    surface: Surface,
    swap_chain_descriptor: SwapChainDescriptor,
    swap_chain: SwapChain,
    multisampled_framebuffer: TextureView,
    stencil: TextureView,
    map: crate::drawing::map::Map,
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

        let swap_chain = device.create_swap_chain(
            &surface,
            &swap_chain_descriptor,
        );

        let map = crate::drawing::map::Map::init(&window, &mut device, &app_state);
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
            multisampled_framebuffer,
            stencil,
            map,
            temperature,
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

    pub fn update_shader(&mut self) {
        self.map.update_shader(&self.device);
        self.temperature.update_shader(&self.device);
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

        let frame = self.swap_chain.get_next_texture();

        let view = if CONFIG.renderer.msaa_samples > 1 { &self.multisampled_framebuffer } else { &frame.view };
        let resolve_target = if CONFIG.renderer.msaa_samples > 1 { Some(&frame.view) } else { None };
        
        // Paint map.
        self.map.paint(
            app_state,
            &mut self.device,
            &mut encoder,
            view,
            resolve_target,
            &self.stencil
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