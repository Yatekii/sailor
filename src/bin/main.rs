mod app_state;
mod config;
mod drawing;
mod stats;

use crate::config::CONFIG;
use lyon::math::vector;
use osm::math::{deg2num, tile_to_world_space};
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalPosition, PhysicalPosition},
    event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::{Key, ModifiersState, NamedKey},
    window::WindowId,
};

fn main() {
    log::set_max_level(CONFIG.general.log.level.to_level_filter());
    pretty_env_logger::init();

    let tile_coordinate = deg2num(
        CONFIG.map.initial.center.latitude,
        CONFIG.map.initial.center.longitude,
        CONFIG.map.initial.zoom as u32,
    );
    let initial_center = tile_to_world_space(&tile_coordinate);

    let width = 1200;
    let height = 800;

    let event_loop = winit::event_loop::EventLoop::new().unwrap();

    let app_state = app_state::AppState::new(
        CONFIG.renderer.css.clone(),
        initial_center,
        width,
        height,
        CONFIG.map.initial.zoom,
        2.0,
    );

    let mut painter = drawing::Painter::init(&event_loop, width, height, &app_state);
    let hud = drawing::ui::Hud::new(
        &painter.window,
        &mut painter.device,
        &painter.surface_config,
    );

    let mouse_down = false;
    let last_pos = winit::dpi::LogicalPosition::new(0.0, 0.0);

    let modifiers_state = ModifiersState::default();

    let mut application = Application {
        hud,
        painter,
        app_state,
        modifiers_state,
        mouse_down,
        last_pos,
    };

    event_loop.run_app(&mut application).unwrap();
}

pub struct Application {
    hud: drawing::ui::Hud,
    painter: drawing::Painter,
    app_state: app_state::AppState,
    modifiers_state: ModifiersState,
    mouse_down: bool,
    last_pos: LogicalPosition<f64>,
}

impl ApplicationHandler for Application {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let _ = event_loop;
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let ui_event = self.hud.interact(&event);
        match event {
            WindowEvent::Destroyed => event_loop.exit(),
            WindowEvent::Resized(physical_size) => {
                self.app_state.screen.width = physical_size.width.min(8192);
                self.app_state.screen.height = physical_size.height.min(8192);
                self.painter
                    .resize(self.app_state.screen.width, self.app_state.screen.height);
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.app_state.scale_factor_updated(scale_factor)
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: keycode,
                        ..
                    },
                ..
            } => {
                if !ui_event {
                    match keycode {
                        Key::Character(character) => {
                            if character == "Q" && self.modifiers_state.super_key() {
                                event_loop.exit()
                            }
                        }
                        Key::Named(NamedKey::Escape) => event_loop.exit(),
                        Key::Named(NamedKey::Tab) => self.app_state.advance_selected_object(),
                        _ => {}
                    }
                }
            }
            WindowEvent::ModifiersChanged(state) => {
                self.modifiers_state = state.state();
            }
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::MouseInput { state, button, .. } => {
                if !ui_event {
                    if let MouseButton::Left = button {
                        match state {
                            ElementState::Pressed => {
                                self.mouse_down = true;
                            }
                            ElementState::Released => {
                                self.mouse_down = false;
                                self.app_state.update_selected_hover_objects();
                            }
                        }
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if !ui_event {
                    match delta {
                        MouseScrollDelta::LineDelta(_, y) => self.app_state.zoom += 0.1 * y,
                        MouseScrollDelta::PixelDelta(PhysicalPosition { y, .. }) => {
                            self.app_state.zoom += 0.001 * y as f32
                        }
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let logical_position = position.to_logical(self.painter.get_hidpi_factor());
                let size = self.app_state.screen.tile_size() as f32;
                let mut delta = vector(
                    (logical_position.x - self.last_pos.x) as f32,
                    (logical_position.y - self.last_pos.y) as f32,
                );
                let zoom_x = (self.app_state.screen.width as f32)
                    / size
                    / 2f32.powf(self.app_state.zoom)
                    / size
                    / 1.5;
                let zoom_y = (self.app_state.screen.height as f32)
                    / size
                    / 2f32.powf(self.app_state.zoom)
                    / size
                    / 1.5;
                delta.x *= zoom_x;
                delta.y *= zoom_y;

                self.last_pos = logical_position;

                if !ui_event {
                    if self.mouse_down {
                        self.app_state.screen.center -= delta;
                    }

                    self.app_state.update_hovered_objects((
                        logical_position.x as f32,
                        logical_position.y as f32,
                    ))
                }
            }
            WindowEvent::RedrawRequested => {
                if !event_loop.exiting() {
                    self.painter.update_shader();
                    self.app_state.load_tiles();
                    self.painter.paint(&mut self.hud, &mut self.app_state);

                    self.app_state.stats.capture_frame();
                    if CONFIG.general.display_framerate {
                        println!(
                            "Frametime {:.2?} at zoom {:.2}",
                            self.app_state.stats.get_average(),
                            self.app_state.zoom
                        );
                    }
                }
            }
            _ => (),
        }
        self.painter.window.request_redraw();
    }
}
