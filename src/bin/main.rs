mod app_state;
mod config;
mod drawing;
mod hover;
mod stats;

use std::sync::Arc;

use crate::config::CONFIG;
use clap::Parser;
use nalgebra_glm::vec2;
use osm::math::{Coord, Geo, Pixel, TileId, deg2num, tile_to_world_space};
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
    osm::platform::init_logging(CONFIG.general.log.level);

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
        config::WindowSize::Windowed { width, height } => attributes
            .with_inner_size(LogicalSize { width, height })
            .with_maximized(true),
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
    map: drawing::layer::map::MapLayer,
    overlays: drawing::layer::LayerStack,
    app_state: app_state::AppState,
    modifiers_state: ModifiersState,
    mouse_down: bool,
    drag_moved: bool,
    last_position: Coord<Pixel>,
    /// Cursor position when the left button went down, to tell a click from a drag.
    press_position: Coord<Pixel>,
    args: Args,
}

impl Application {
    async fn new(window: Arc<Window>, args: Args) -> Self {
        let tile_coordinate = deg2num(
            Coord::<Geo>::new(
                CONFIG.map.initial.center.longitude,
                CONFIG.map.initial.center.latitude,
            ),
            CONFIG.map.initial.zoom as u32,
        );
        let initial_center = tile_to_world_space(&tile_coordinate);

        // On the web winit reports the freshly-appended canvas as 0x0; size the
        // surface from the browser viewport instead (and tell winit about it).
        let size = match osm::platform::viewport_size() {
            Some((width, height)) => {
                let factor = window.scale_factor();
                let size = winit::dpi::PhysicalSize::new(
                    (width as f64 * factor) as u32,
                    (height as f64 * factor) as u32,
                );
                let _ = window.request_inner_size(size);
                size
            }
            None => window.inner_size(),
        };

        // The map owns its tile/feature data; the app shares the feature-collection
        // handle so the UI (layer toggles) can reach it too.
        let feature_collection = std::sync::Arc::new(std::sync::RwLock::new(
            osm::feature::collection::FeatureCollection::new(),
        ));

        let app_state = app_state::AppState::new(
            CONFIG.renderer.css.clone(),
            initial_center,
            size,
            CONFIG.map.initial.zoom,
            2.0,
            feature_collection.clone(),
        );

        let mut painter = drawing::Painter::init(window, size).await;
        let hud = drawing::ui::Hud::new(&painter.window, &painter.device, &painter.surface_config);

        let map = drawing::layer::map::MapLayer::new(
            &painter.device,
            &app_state.screen,
            feature_collection,
            &mut painter.text,
        );
        let mut overlays = drawing::layer::LayerStack::new();
        overlays.push(Box::new(drawing::layer::wind::WindLayer::new(
            &painter.device,
        )));
        overlays.push(Box::new(
            drawing::layer::temperature::TemperatureLayer::default(),
        ));
        overlays.push(Box::new(drawing::layer::hover::HoverLayer::new(
            &painter.device,
            painter.surface_config.format,
        )));
        overlays.push(Box::new(drawing::layer::legend::LegendLayer::new(
            &painter.device,
            &mut painter.text,
        )));

        Self {
            hud,
            painter,
            map,
            overlays,
            app_state,
            modifiers_state: ModifiersState::default(),
            mouse_down: false,
            drag_moved: false,
            last_position: Coord::<Pixel>::new(0.0, 0.0),
            press_position: Coord::<Pixel>::new(0.0, 0.0),
            args,
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, event: WindowEvent) {
        if self.app_state.css_cache.update() {
            // Restyle already-loaded features against the new sheet; without this
            // only tiles loaded after the edit would pick up the change.
            let zoom = self.app_state.screen.zoom;
            let fc = self.app_state.feature_collection();
            fc.write()
                .unwrap()
                .load_styles(zoom, &mut self.app_state.css_cache);
        }
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
                            self.drag_moved = false;
                            self.press_position = self.last_position;
                        }
                        ElementState::Released => {
                            self.mouse_down = false;
                            // a drag is a pan, not a selection
                            if !self.drag_moved {
                                self.app_state.update_selected_from_hover_objects();
                            }
                        }
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if !ui_event {
                    let cursor = self.last_position;
                    let to = match delta {
                        MouseScrollDelta::LineDelta(_, y) => self.app_state.screen.zoom + 0.1 * y,
                        MouseScrollDelta::PixelDelta(PhysicalPosition { y, .. }) => {
                            self.app_state.screen.zoom + 0.001 * *y as f32
                        }
                    };
                    self.app_state.screen.zoom_to_cursor(cursor, to);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let logical_position = position.to_logical(self.painter.get_hidpi_factor());

                let from = self.last_position;
                let to = Coord::<Pixel>::new(position.x as f32, position.y as f32);

                self.last_position = to;

                self.app_state
                    .set_cursor(vec2(logical_position.x, logical_position.y));

                if !ui_event {
                    if self.mouse_down {
                        // Only a real drag past a small threshold pans and blocks the
                        // release-selection; sub-pixel jitter during a click must not.
                        const DRAG_THRESHOLD_PX: f32 = 6.0;
                        let dx = to.x() - self.press_position.x();
                        let dy = to.y() - self.press_position.y();
                        if (dx * dx + dy * dy).sqrt() > DRAG_THRESHOLD_PX {
                            self.drag_moved = true;
                        }
                        if self.drag_moved {
                            self.app_state.screen.pan(from, to);
                        }
                    }

                    self.map.update_hovered_objects(
                        &self.app_state.screen,
                        (position.x as f32, position.y as f32),
                        self.app_state.hovered_objects.clone(),
                    )
                }
            }
            WindowEvent::RedrawRequested if !event_loop.exiting() => {
                if self.args.tile.is_empty() {
                    self.map
                        .load_visible(&self.app_state.screen, &mut self.app_state.css_cache);
                } else {
                    for tile in &self.args.tile {
                        let coords: Vec<u32> =
                            tile.split("/").filter_map(|v| v.parse().ok()).collect();
                        if coords.len() != 3 {
                            continue;
                        }

                        self.map.load_tile(
                            TileId::new(coords[0], coords[1], coords[2]),
                            &self.app_state.screen,
                            &mut self.app_state.css_cache,
                        );
                    }
                }

                self.app_state.tile_stats = self.map.tile_stats();

                let selection =
                    self.app_state
                        .selected_object()
                        .map(|s| drawing::layer::Selection {
                            tile_id: s.tile_id,
                            feature_id: s.object.feature_id,
                            feature_slot: s.object.feature_slot,
                        });

                let cursor = {
                    let c = self.app_state.cursor();
                    (c.x, c.y)
                };
                let pixels_per_point = self.painter.window.scale_factor() as f32;
                let hovered = self.app_state.hovered_objects.clone();
                let hovered = hovered.lock().unwrap();
                let zoom = self.app_state.screen.zoom;
                let painted = hover::with_hover_info(
                    &hovered,
                    &self.app_state.css_cache,
                    zoom,
                    cursor,
                    pixels_per_point,
                    |hover| {
                        self.painter.paint(
                            &mut self.map,
                            &mut self.overlays,
                            &self.app_state.screen,
                            selection,
                            hover,
                            self.app_state.ui.wind,
                            &mut self.app_state.stats,
                        )
                    },
                );
                drop(hovered);

                if let Some(mut frame) = painted {
                    let hud_start = web_time::Instant::now();
                    self.hud.paint(
                        &mut self.app_state,
                        &self.painter.window,
                        &self.painter.device,
                        &self.painter.queue,
                        &mut frame.encoder,
                        &frame.surface,
                    );
                    frame.push_span("cpu.hud", hud_start.elapsed());
                    self.painter.present(frame, &mut self.app_state.stats);
                }

                self.app_state.stats.capture_frame();
                if CONFIG.general.display_framerate {
                    println!(
                        "Frametime {:.2?} at zoom {:.2}",
                        self.app_state.stats.get_average(),
                        self.app_state.screen.zoom
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
