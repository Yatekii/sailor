mod app_state;
mod config;
mod drawing;
mod stats;

use std::sync::Arc;

use crate::config::CONFIG;
use clap::Parser;
use lyon::geom::euclid::{self};
use nalgebra_glm::{vec2, vec4};
use osm::math::{TileId, deg2num, tile_to_world_space};
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalSize, PhysicalPosition},
    event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    keyboard::{Key, ModifiersState, NamedKey},
    window::{Window, WindowAttributes, WindowId},
};

/// Event injected into the loop once the (async) renderer initialization finished.
/// Only the web path constructs this; natively initialization is synchronous.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
enum UserEvent {
    Initialized(Box<Application>),
}

fn main() {
    init_logging();

    let event_loop = EventLoop::<UserEvent>::with_user_event().build().unwrap();
    let mut app = App {
        proxy: event_loop.create_proxy(),
        application: None,
        initializing: false,
        args: parse_args(),
    };

    #[cfg(not(target_arch = "wasm32"))]
    event_loop.run_app(&mut app).unwrap();
    #[cfg(target_arch = "wasm32")]
    {
        use winit::platform::web::EventLoopExtWebSys;
        event_loop.spawn_app(app);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn init_logging() {
    log::set_max_level(CONFIG.general.log.level.to_level_filter());
    pretty_env_logger::init();
}

#[cfg(target_arch = "wasm32")]
fn init_logging() {
    console_error_panic_hook::set_once();
    let _ = console_log::init_with_level(log::Level::Info);
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_args() -> Args {
    Args::parse()
}

#[cfg(target_arch = "wasm32")]
fn parse_args() -> Args {
    Args::default()
}

fn window_attributes() -> WindowAttributes {
    let attributes = WindowAttributes::default().with_title("Sailor");
    let attributes = match CONFIG.window.size {
        config::WindowSize::Windowed { width, height } => {
            attributes.with_inner_size(LogicalSize { width, height })
        }
        config::WindowSize::Fullscreen => attributes
            .with_fullscreen(Some(winit::window::Fullscreen::Borderless(None)))
            .with_decorations(false),
    };
    #[cfg(target_arch = "wasm32")]
    {
        use winit::platform::web::WindowAttributesExtWebSys;
        // Let winit create a canvas and append it to the document body.
        attributes.with_append(true)
    }
    #[cfg(not(target_arch = "wasm32"))]
    attributes
}

/// The winit handler. Owns the (possibly not-yet-initialized) application and
/// creates the window plus the renderer when the event loop first resumes.
struct App {
    // Only used on the web to deliver the async-initialized application.
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    proxy: EventLoopProxy<UserEvent>,
    application: Option<Application>,
    initializing: bool,
    args: Args,
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.application.is_some() || self.initializing {
            return;
        }
        self.initializing = true;

        let window = Arc::new(event_loop.create_window(window_attributes()).unwrap());
        let args = self.args.clone();

        #[cfg(not(target_arch = "wasm32"))]
        {
            self.application = Some(pollster::block_on(Application::new(window, args)));
        }
        #[cfg(target_arch = "wasm32")]
        {
            let proxy = self.proxy.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let application = Application::new(window, args).await;
                let _ = proxy.send_event(UserEvent::Initialized(Box::new(application)));
            });
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: UserEvent) {
        let UserEvent::Initialized(application) = event;
        self.application = Some(*application);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        if let Some(application) = self.application.as_mut() {
            application.window_event(event_loop, event);
        }
    }
}

pub struct Application {
    hud: drawing::ui::Hud,
    painter: drawing::Painter,
    app_state: app_state::AppState,
    modifiers_state: ModifiersState,
    mouse_down: bool,
    last_pos: PhysicalPosition<f64>,
    args: Args,
}

impl Application {
    async fn new(window: Arc<Window>, args: Args) -> Self {
        let tile_coordinate = deg2num(
            CONFIG.map.initial.center.latitude,
            CONFIG.map.initial.center.longitude,
            CONFIG.map.initial.zoom as u32,
        );
        let initial_center = tile_to_world_space(&tile_coordinate);
        let size = window.inner_size();

        let app_state = app_state::AppState::new(
            CONFIG.renderer.css.clone(),
            initial_center,
            size,
            CONFIG.map.initial.zoom,
            2.0,
        );

        let painter = drawing::Painter::init(window, size, &app_state).await;
        let hud = drawing::ui::Hud::new(&painter.window, &painter.device, &painter.surface_config);

        Self {
            hud,
            painter,
            app_state,
            modifiers_state: ModifiersState::default(),
            mouse_down: false,
            last_pos: PhysicalPosition::new(0.0, 0.0),
            args,
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, event: WindowEvent) {
        self.app_state.css_cache.update();
        let ui_event = self.hud.interact(&self.painter.window, &event);
        match &event {
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
                self.app_state.scale_factor_updated(*scale_factor as f32)
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        state: ElementState::Pressed,
                        logical_key: keycode,
                        ..
                    },
                ..
            } => match keycode {
                Key::Character(character) => {
                    if character == "Q" && self.modifiers_state.super_key() {
                        event_loop.exit()
                    }
                }
                Key::Named(NamedKey::Escape) => {
                    if !ui_event {
                        event_loop.exit()
                    }
                }
                Key::Named(NamedKey::Tab) => self.app_state.advance_selected_object(),
                _ => {}
            },
            WindowEvent::ModifiersChanged(state) => {
                self.modifiers_state = state.state();
            }
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::MouseInput { state, button, .. } => {
                if !ui_event && let MouseButton::Left = button {
                    match state {
                        ElementState::Pressed => {
                            self.mouse_down = true;
                        }
                        ElementState::Released => {
                            self.mouse_down = false;
                            self.app_state.update_selected_from_hover_objects();
                        }
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if !ui_event {
                    match delta {
                        MouseScrollDelta::LineDelta(_, y) => self.app_state.zoom += 0.1 * y,
                        MouseScrollDelta::PixelDelta(PhysicalPosition { y, .. }) => {
                            self.app_state.zoom += 0.001 * *y as f32
                        }
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let logical_position = position.to_logical(self.painter.get_hidpi_factor());

                let screen_to_global = self.app_state.screen.pixel_to_world(self.app_state.zoom);
                let new_pos =
                    screen_to_global * vec4(position.x as f32, position.y as f32, 0.0, 0.0);
                let old_pos = screen_to_global
                    * vec4(self.last_pos.x as f32, self.last_pos.y as f32, 0.0, 0.0);
                let delta_new = new_pos - old_pos;

                self.last_pos = *position;

                self.app_state
                    .set_cursor(vec2(logical_position.x, logical_position.y));

                if !ui_event {
                    if self.mouse_down {
                        self.app_state.screen.center -= euclid::vec2(delta_new.x, delta_new.y);
                    }

                    self.app_state
                        .update_hovered_objects((position.x as f32, position.y as f32))
                }
            }
            WindowEvent::RedrawRequested if !event_loop.exiting() => {
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
            _ => (),
        }
        self.painter.window.request_redraw();
    }
}

/// Render a map beautifully and ultra fast with CSS styling
#[derive(Parser, Debug, Default, Clone)]
#[command(version, about, long_about = None)]
struct Args {
    /// Tile to load (this argument can be given multiple times to load multiple tiles)
    ///
    /// If no tiles were given the full map is loaded.
    #[arg(short, long)]
    tile: Vec<String>,
}
