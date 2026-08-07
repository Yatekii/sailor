#[cfg(target_arch = "wasm32")]
mod egui_shim;
pub mod state;
pub mod views;
pub mod widgets;

use crate::app_state::AppState;
use egui::Color32;
use egui::Layout;
use egui::Style;
use egui::Visuals;
use egui::vec2;
#[cfg(target_arch = "wasm32")]
use egui_shim::EguiState;
use egui_wgpu::Renderer;
use egui_wgpu::RendererOptions;
use egui_wgpu::ScreenDescriptor;
#[cfg(not(target_arch = "wasm32"))]
use egui_winit::State as EguiState;
use views::fps::view_fps;
use views::location_finder::LocationFinderWindow;
use views::panel_bottom::panel_bottom;
use views::panel_left::PanelLeft;
use views::panel_right::panel_right;
use wgpu::SurfaceConfiguration;

pub struct Hud {
    ctx: egui::Context,
    state: EguiState,
    renderer: Renderer,
    ui: HudUi,
}

impl Hud {
    pub fn new(
        window: &winit::window::Window,
        device: &wgpu::Device,
        surface_config: &SurfaceConfiguration,
    ) -> Self {
        let ctx = egui::Context::default();

        ctx.all_styles_mut(|style| {
            *style = Style {
                visuals: Visuals {
                    override_text_color: Some(Color32::WHITE),
                    ..Visuals::dark()
                },
                ..Style::default()
            };
            style.visuals.text_cursor.blink = false;
        });

        let state = EguiState::new(
            ctx.clone(),
            egui::ViewportId::ROOT,
            window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );

        let renderer = Renderer::new(device, surface_config.format, RendererOptions::default());

        let ui = HudUi {
            location_finder_window: LocationFinderWindow::new(false),
            side_panel: PanelLeft::new(),
        };

        Self {
            ctx,
            state,
            renderer,
            ui,
        }
    }

    pub fn paint(
        &mut self,
        app_state: &mut AppState,
        window: &winit::window::Window,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        frame: &wgpu::SurfaceTexture,
    ) {
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let raw_input = self.state.take_egui_input(window);
        let ctx = self.ctx.clone();
        let hud_ui = &mut self.ui;
        let mut full_output = ctx.run_ui(raw_input, |ui| {
            hud_ui.ui(ui, app_state);
        });
        self.state
            .handle_platform_output(window, full_output.platform_output);

        let paint_jobs = self
            .ctx
            .tessellate(full_output.shapes, full_output.pixels_per_point);

        let size = window.inner_size();
        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [size.width, size.height],
            pixels_per_point: window.scale_factor() as f32,
        };

        for (id, image_deltas) in &full_output.textures_delta.set {
            for image_delta in image_deltas {
                self.renderer
                    .update_texture(device, queue, *id, image_delta);
            }
        }
        self.renderer
            .update_buffers(device, queue, encoder, &paint_jobs, &screen_descriptor);

        {
            let mut render_pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("egui"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        depth_slice: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                })
                .forget_lifetime();
            self.renderer
                .render(&mut render_pass, &paint_jobs, &screen_descriptor);
        }

        for id in &full_output.textures_delta.free {
            self.renderer.free_texture(id);
        }
        // epaint's TexturesDelta panics on drop if not consumed.
        full_output.textures_delta.clear();
    }

    pub fn interact(
        &mut self,
        window: &winit::window::Window,
        event: &winit::event::WindowEvent,
    ) -> bool {
        let response = self.state.on_window_event(window, event);
        response.consumed || self.ctx.is_pointer_over_egui()
    }
}

struct HudUi {
    location_finder_window: LocationFinderWindow,
    side_panel: PanelLeft,
}

impl HudUi {
    pub fn ui(&mut self, ui: &mut egui::Ui, app_state: &mut AppState) {
        {
            let pointer_position = ui
                .ctx()
                .input(|i| i.pointer.hover_pos())
                .unwrap_or_default();

            // This should have precedence for grabbing keystrokes when it's open.
            self.location_finder_window.ui(ui.ctx(), app_state);

            // Draw menubar.
            egui::Panel::top("Main Menu Bar").show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    // Take full width and fixed height:
                    let height = ui.spacing().interact_size.y;
                    ui.style_mut().spacing.button_padding = vec2(2.0, 0.0);
                    ui.set_min_size(vec2(ui.available_width(), height));
                    ui.menu_button("File", |ui| {
                        if ui.button("Quit").clicked() {
                            ui.close();
                        }
                    });
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(format!(
                            "Frametime {:.2?} at zoom {:.2}",
                            app_state.stats.get_average(),
                            app_state.screen.zoom
                        ));

                        ui.label(format!(
                            "Mouse Position: ({:.1},{:.1})",
                            pointer_position[0], pointer_position[1]
                        ));

                        view_fps(ui, app_state);
                    })
                })
            });

            panel_bottom(ui, app_state);

            panel_right(ui, app_state);

            self.side_panel.ui(ui, app_state);
        }
    }
}
