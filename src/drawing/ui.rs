use crate::app_state::AppState;
use imgui::*;

pub struct HUD {
    platform: imgui_winit_support::WinitPlatform,
    imgui: imgui::Context,
    renderer: imgui_wgpu::Renderer,
}

impl HUD {
    pub fn new(
        window: &wgpu::winit::Window,
        device: &mut wgpu::Device,
    ) -> Self {
        let hidpi_factor = window.get_hidpi_factor();
        let mut imgui = imgui::Context::create();
        let mut platform = imgui_winit_support::WinitPlatform::init(&mut imgui);
        platform.attach_window(imgui.io_mut(), &window, imgui_winit_support::HiDpiMode::Default);
        imgui.set_ini_filename(None);

        let font_size = (13.0 * hidpi_factor) as f32;
        imgui.io_mut().font_global_scale = (1.0 / hidpi_factor) as f32;

        imgui.fonts().add_font(&[
            imgui::FontSource::DefaultFontData {
                config: Some(imgui::FontConfig {
                    oversample_h: 1,
                    pixel_snap_h: true,
                    size_pixels: font_size,
                    ..Default::default()
                })
            }
        ]);

        let renderer = imgui_wgpu::Renderer::new(
            &mut imgui,
            device,
            wgpu::TextureFormat::Bgra8Unorm,
            None,
        )
        .expect("Failed to initialize renderer");

        Self {
            platform,
            imgui,
            renderer,
        }
    }

    pub fn paint(
        &mut self,
        app_state: &mut AppState,
        window: &wgpu::winit::Window,
        width: f64,
        height: f64,
        hidpi_factor: f64,
        device: &mut wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
    ) {
        self.platform.prepare_frame(self.imgui.io_mut(), &window) // step 4
            .expect("Failed to prepare frame");
        let ui = self.imgui.frame();

        {
            let window = imgui::Window::new(im_str!("Hello world"));
            window
                .size([300.0, 100.0], imgui::Condition::FirstUseEver)
                .build(&ui, || {
                    ui.text(im_str!("{:#?}", app_state.hovered_objects.iter().map(|o| o.tags.clone()).collect::<Vec<_>>()));
                    ui.separator();
                    let mouse_pos = ui.io().mouse_pos;
                    ui.text(im_str!(
                        "Mouse Position: ({:.1},{:.1})",
                        mouse_pos[0],
                        mouse_pos[1]
                    ));
                });

            let window = imgui::Window::new(im_str!("Hello too"));
            window
                .size([400.0, 200.0], Condition::FirstUseEver)
                .position([400.0, 200.0], Condition::FirstUseEver)
                .build(&ui, || {
                    let mut item = 0;
                    // for i in 0..app_state.selected_objects.len() {
                    //     if app_state.selected_objects[i].selected {
                    //         item = i;
                    //     }
                    // }
                    let items = app_state.selected_objects.iter().map(|o| im_str!("{:?}", o.object)).collect::<Vec<_>>();
                    let mut item_refs = vec![];
                    for item in &items {
                        item_refs.push(item);
                    }
                    ui.text(im_str!("Hello world!"));
                    ui.list_box(
                        im_str!("hello top"),
                        &mut item,
                        &item_refs,
                        5
                    );
                });

            ui.show_demo_window(&mut true);
        }

        self.platform.prepare_render(&ui, &window);
        self.renderer
            .render(ui, width, height, hidpi_factor, device, encoder, &view)
            .expect("Rendering failed");
    }

    pub fn interact(&mut self, window: &wgpu::winit::Window, event: &wgpu::winit::Event) -> bool {
        self.platform.handle_event(self.imgui.io_mut(), &window, &event);
        self.imgui.io().want_capture_mouse
    }
}