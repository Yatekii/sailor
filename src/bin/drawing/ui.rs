pub mod state;
pub mod views;
pub mod widgets;

use crate::app_state::AppState;
use egui::vec2;
use egui::Color32;
use egui::FontDefinitions;
use egui::Layout;
use egui::Style;
use egui::Visuals;
use egui_wgpu_backend::RenderPass;
use egui_wgpu_backend::ScreenDescriptor;
use egui_winit_platform::PlatformDescriptor;
use views::fps::view_fps;
use views::location_finder::LocationFinderWindow;
use views::panel_bottom::panel_bottom;
use views::panel_left::PanelLeft;
use views::panel_right::panel_right;
use wgpu::SurfaceConfiguration;

pub struct Hud {
    pub(crate) platform: egui_winit_platform::Platform,
    rpass: RenderPass,
    ui: HudUi,
}

impl Hud {
    pub fn new(
        window: &winit::window::Window,
        device: &mut wgpu::Device,
        surface_config: &SurfaceConfiguration,
    ) -> Self {
        // We use the egui_winit_platform crate as the platform.
        let size = window.inner_size();
        let platform = egui_winit_platform::Platform::new(PlatformDescriptor {
            physical_width: size.width,
            physical_height: size.height,
            scale_factor: window.scale_factor(),
            font_definitions: FontDefinitions::default(),
            style: Default::default(),
        });

        let style = Style {
            visuals: Visuals {
                override_text_color: Some(Color32::WHITE),
                ..Visuals::dark()
            },
            ..Style::default()
        };
        platform.context().set_style(style);

        // We use the egui_wgpu_backend crate as the render backend.
        let rpass = RenderPass::new(device, surface_config.format, 1);

        let ui = HudUi {
            location_finder_window: LocationFinderWindow::new(false),
            side_panel: PanelLeft::new(),
        };

        Self {
            platform,
            rpass,
            ui,
        }
    }

    pub fn paint(
        &mut self,
        app_state: &mut AppState,
        window: &winit::window::Window,
        device: &mut wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        frame: &wgpu::SurfaceTexture,
    ) {
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        self.platform.context().set_pixels_per_point(4.0);
        // Begin to draw the UI frame.
        self.platform.begin_pass();

        // Draw the demo application.
        self.ui.ui(&self.platform.context(), app_state);

        // End the UI frame. We could now handle the output and draw the UI with the backend.
        let full_output = self.platform.end_pass(Some(window));
        let paint_jobs = self.platform.context().tessellate(
            full_output.shapes,
            self.platform.context().pixels_per_point(),
        );

        // Upload all resources for the GPU.
        let size = window.inner_size();
        let screen_descriptor = ScreenDescriptor {
            physical_width: size.width,
            physical_height: size.height,
            scale_factor: window.scale_factor() as f32,
        };
        let tdelta: egui::TexturesDelta = full_output.textures_delta;
        self.rpass
            .add_textures(device, queue, &tdelta)
            .expect("add texture ok");
        self.rpass
            .update_buffers(device, queue, &paint_jobs, &screen_descriptor);

        // Record all render passes.
        self.rpass
            .execute(encoder, &view, &paint_jobs, &screen_descriptor, None)
            .unwrap();
        self.rpass
            .remove_textures(tdelta)
            .expect("remove texture ok");
    }

    pub fn interact(&mut self, event: &winit::event::WindowEvent) -> bool {
        self.platform.handle_event(event);
        self.platform.captures_event(event) || self.platform.context().is_pointer_over_area()
    }
}

struct HudUi {
    location_finder_window: LocationFinderWindow,
    side_panel: PanelLeft,
}

impl HudUi {
    pub fn ui(&mut self, ctx: &egui::Context, app_state: &mut AppState) {
        {
            let pointer_position = ctx.input(|i| i.pointer.hover_pos()).unwrap_or_default();

            // This should have precedence for grabbing keystrokes when it's open.
            self.location_finder_window.ui(ctx, app_state);

            // Draw menubar.
            egui::TopBottomPanel::top("Main Menu Bar").show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    // Take full width and fixed height:
                    let height = ui.spacing().interact_size.y;
                    ui.style_mut().spacing.button_padding = vec2(2.0, 0.0);
                    ui.set_min_size(vec2(ui.available_width(), height));
                    ui.menu_button("File", |ui| {
                        if ui.button("Quit").clicked() {
                            ui.close_menu();
                        }
                    });
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(format!(
                            "Frametime {:.2?} at zoom {:.2}",
                            app_state.stats.get_average(),
                            app_state.zoom
                        ));

                        ui.label(format!(
                            "Mouse Position: ({:.1},{:.1})",
                            pointer_position[0], pointer_position[1]
                        ));

                        view_fps(ui, app_state);
                    })
                })
            });

            panel_bottom(ctx, app_state);

            panel_right(ctx, app_state);

            self.side_panel.ui(ctx, app_state);
        }
    }
}
