mod app_state;
mod config;
mod drawing;
mod stats;

use std::sync::Arc;

use crate::config::CONFIG;
use clap::Parser;
use lyon::{
    geom::euclid::{self, point2},
    math::vector,
};
use nalgebra_glm::{vec2, vec3, vec4, Mat4};
use osm::math::{deg2num, tile_to_world_space, TileId};
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize},
    event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::{Key, ModifiersState, NamedKey},
    window::{WindowAttributes, WindowId},
};

fn main() {
    log::set_max_level(CONFIG.general.log.level.to_level_filter());
    pretty_env_logger::init();

    let args = Args::parse();

    let tile_coordinate = deg2num(
        CONFIG.map.initial.center.latitude,
        CONFIG.map.initial.center.longitude,
        CONFIG.map.initial.zoom as u32,
    );
    let initial_center = tile_to_world_space(&tile_coordinate);

    let event_loop = winit::event_loop::EventLoop::new().unwrap();

    let attributes = WindowAttributes::default();
    let window_attributes = match CONFIG.window.size {
        config::WindowSize::Windowed { width, height } => {
            attributes.with_inner_size(LogicalSize { width, height })
        }
        config::WindowSize::Fullscreen => attributes
            .with_fullscreen(Some(winit::window::Fullscreen::Borderless(None)))
            .with_decorations(false),
    };

    #[allow(deprecated)]
    let window = Arc::new(event_loop.create_window(window_attributes).unwrap());
    let size = window.inner_size();

    let app_state = app_state::AppState::new(
        CONFIG.renderer.css.clone(),
        initial_center,
        size,
        CONFIG.map.initial.zoom,
        2.0,
    );

    let mut painter = drawing::Painter::init(window, size, &app_state);
    let hud = drawing::ui::Hud::new(
        &painter.window,
        &mut painter.device,
        &painter.surface_config,
    );

    let mouse_down = false;
    let last_pos = winit::dpi::LogicalPosition::new(0.0, 0.0);

    let modifiers_state = ModifiersState::default();

    let screen = app_state.screen.clone();
    let mut application = Application {
        hud,
        painter,
        app_state,
        modifiers_state,
        mouse_down,
        last_pos,
        args,
    };

    // Hack to make the UI scale correctly.
    application
        .hud
        .platform
        .handle_event(&winit::event::WindowEvent::Resized(PhysicalSize::new(
            screen.width as u32,
            screen.height as u32,
        )));

    event_loop.run_app(&mut application).unwrap();
}

pub struct Application {
    hud: drawing::ui::Hud,
    painter: drawing::Painter,
    app_state: app_state::AppState,
    modifiers_state: ModifiersState,
    mouse_down: bool,
    last_pos: LogicalPosition<f64>,
    args: Args,
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
                self.app_state.screen.width = physical_size.width.min(8192) as f32;
                self.app_state.screen.height = physical_size.height.min(8192) as f32;
                self.painter.resize(
                    self.app_state.screen.width as u32,
                    self.app_state.screen.height as u32,
                );
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.app_state.scale_factor_updated(scale_factor as f32)
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
                let logical_position = position.to_logical(self.painter.get_hidpi_factor() / 2.0);

                let screen_to_global = self.app_state.screen.screen_to_world(self.app_state.zoom);
                let new_pos = screen_to_global
                    * vec4(
                        logical_position.x as f32,
                        logical_position.y as f32,
                        0.0,
                        0.0,
                    );
                let old_pos = screen_to_global
                    * vec4(self.last_pos.x as f32, self.last_pos.y as f32, 0.0, 0.0);
                let delta_new = new_pos - old_pos;

                self.last_pos = logical_position;

                if !ui_event {
                    if self.mouse_down {
                        self.app_state.screen.center -= euclid::vec2(delta_new.x, delta_new.y);
                    }

                    self.app_state.update_hovered_objects((
                        logical_position.x as f32,
                        logical_position.y as f32,
                    ))
                }
            }
            WindowEvent::RedrawRequested => {
                self.hud
                    .platform
                    .handle_event(&winit::event::WindowEvent::Resized(PhysicalSize::new(
                        self.app_state.screen.width as u32,
                        self.app_state.screen.height as u32,
                    )));
                if !event_loop.exiting() {
                    self.painter.update_shader();
                    // self.app_state.load_tile(TileId::new(13, 4290, 2868));

                    if self.args.tile.is_empty() {
                        self.app_state.load_tiles();
                    } else {
                        for tile in &self.args.tile {
                            let coords: Vec<u32> =
                                tile.split("/").filter_map(|v| v.parse().ok()).collect();
                            if coords.len() != 3 {
                                continue;
                            }

                            self.app_state
                                .load_tile(TileId::new(coords[0], coords[1], coords[2]));
                        }
                    }

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

/// Render a map beautifully and ultra fast with CSS styling
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Tile to load (this argument can be given multiple times to load multiple tiles)
    ///
    /// If no tiles were given the full map is loaded.
    #[arg(short, long)]
    tile: Vec<String>,
}
